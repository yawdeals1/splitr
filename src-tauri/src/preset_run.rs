//! Runs a preset: its strategy, then its `fallback` when the first finds nothing (FR-30 to FR-32,
//! #8). Every list of parts it returns has passed `ranges::validate` (FR-35).

use serde::{Deserialize, Serialize};

use crate::error::{Result, SplitError};
use crate::presets::{Preset, StrategyKind};
use crate::ranges::{self, Range};
use crate::strategy::{BlankPageStrategy, DocumentText, FixedStrategy, OutlineStrategy, PatternStrategy, Strategy};

/// What the user set for this split.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunOptions {
    /// A page with at most this many visible characters counts as blank (FR-32, from settings).
    pub blank_threshold: u32,
    /// Leave blank separator pages out of the parts (FR-32).
    pub drop_separators: bool,
}

/// What running a preset gave.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PresetRun {
    /// Parts found by rules, already validated.
    #[serde(rename_all = "camelCase")]
    Parts {
        ranges: Vec<Range>,
        /// The strategy that found them: the preset's own, or its fallback.
        strategy: StrategyKind,
        /// True when the parts may leave pages out (dropped blank separators), for the review screen.
        allow_gaps: bool,
        /// Why the preset's own strategy found nothing, when its fallback found the parts.
        fallback_reason: Option<String>,
    },
    /// The preset splits with the AI, or falls back to it. `reason` says why the rules found
    /// nothing when the AI is the fallback. Smart mode runs the AI step (#64).
    #[serde(rename_all = "camelCase")]
    NeedsAi { reason: Option<String> },
}

/// Runs `preset` on the extracted `doc`. When its strategy finds no bookmarks or no split points,
/// the preset's `fallback` runs instead (FR-30).
pub fn run(preset: &Preset, doc: &DocumentText, options: &RunOptions) -> Result<PresetRun> {
    match attempt(preset, preset.strategy, doc, options, None) {
        Err(first @ (SplitError::NoOutline | SplitError::NothingFound)) => match preset.fallback {
            Some(fallback) if fallback != preset.strategy => {
                attempt(preset, fallback, doc, options, Some(first.to_string()))
            }
            _ => Err(first),
        },
        result => result,
    }
}

fn attempt(
    preset: &Preset,
    kind: StrategyKind,
    doc: &DocumentText,
    options: &RunOptions,
    fallback_reason: Option<String>,
) -> Result<PresetRun> {
    let mut allow_gaps = false;
    let ranges = match kind {
        StrategyKind::Ai => return Ok(PresetRun::NeedsAi { reason: fallback_reason }),
        StrategyKind::Outline => OutlineStrategy.propose(doc)?,
        StrategyKind::Pattern => {
            needs_text(doc)?;
            let pattern = preset.pattern.as_deref().filter(|p| !p.trim().is_empty()).ok_or_else(|| {
                SplitError::Presets(format!("The preset \"{}\" has no pattern to search for", preset.name))
            })?;
            PatternStrategy::new(pattern)?.propose(doc)?
        }
        StrategyKind::Blank => {
            needs_text(doc)?;
            allow_gaps = options.drop_separators;
            let strategy = BlankPageStrategy {
                // Capped here too, not only in saved settings, since callers send the value (FR-32).
                max_chars: options.blank_threshold.min(crate::settings::MAX_BLANK_THRESHOLD) as usize,
                drop_separators: options.drop_separators,
            };
            strategy.propose(doc)?
        }
        StrategyKind::Fixed => {
            let every = preset.every.ok_or_else(|| {
                SplitError::Presets(format!("The preset \"{}\" doesn't say how many pages go in each part", preset.name))
            })?;
            FixedStrategy::EveryN(every).propose(doc)?
        }
    };
    if ranges.is_empty() {
        return Err(SplitError::InvalidRanges("Every page is blank, so there are no parts to save".into()));
    }
    ranges::validate(&ranges, doc.page_count, allow_gaps)?;
    Ok(PresetRun::Parts { ranges, strategy: kind, allow_gaps, fallback_reason })
}

/// True if `preset`, or its fallback, splits by the page text: the strategies that OCR reads
/// scanned pages for (FR-80). Bookmarks and fixed lengths don't need the text.
pub fn reads_text(preset: &Preset) -> bool {
    let reads = |kind: StrategyKind| matches!(kind, StrategyKind::Pattern | StrategyKind::Blank | StrategyKind::Ai);
    reads(preset.strategy) || preset.fallback.is_some_and(reads)
}

/// Text-based strategies can't read a PDF with no text at all, such as a scan without OCR
/// (FR-80). Without this check every page of a scan would count as blank.
fn needs_text(doc: &DocumentText) -> Result<()> {
    if doc.pages.iter().any(|p| p.chars().any(|c| !c.is_whitespace())) {
        Ok(())
    } else {
        Err(SplitError::NoTextLayer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::OutlineEntry;

    fn preset(strategy: StrategyKind, fallback: Option<StrategyKind>) -> Preset {
        Preset {
            id: "test".into(),
            category: "Tests".into(),
            name: "Test preset".into(),
            strategy,
            fallback,
            prompt: "Split it".into(),
            pattern: Some(r"Invoice #\d+".into()),
            every: Some(2),
            user: false,
        }
    }

    fn doc(pages: &[&str]) -> DocumentText {
        DocumentText {
            page_count: pages.len() as u32,
            pages: pages.iter().map(|p| p.to_string()).collect(),
            outline: Vec::new(),
        }
    }

    fn parts(run: PresetRun) -> (Vec<(u32, u32)>, StrategyKind, bool, Option<String>) {
        match run {
            PresetRun::Parts { ranges, strategy, allow_gaps, fallback_reason } => {
                (ranges.iter().map(|r| (r.start, r.end)).collect(), strategy, allow_gaps, fallback_reason)
            }
            other => panic!("expected parts, got {other:?}"),
        }
    }

    const OPTIONS: RunOptions = RunOptions { blank_threshold: 0, drop_separators: false };

    #[test]
    fn runs_the_presets_own_strategy_when_it_finds_parts() {
        let mut d = doc(&["a", "b", "c", "d"]);
        d.outline = vec![OutlineEntry { title: "One".into(), page: 1 }, OutlineEntry { title: "Two".into(), page: 3 }];
        let run = run(&preset(StrategyKind::Outline, Some(StrategyKind::Ai)), &d, &OPTIONS).unwrap();
        assert_eq!(parts(run), (vec![(1, 2), (3, 4)], StrategyKind::Outline, false, None));
    }

    #[test]
    fn falls_back_to_the_ai_without_bookmarks_or_matches() {
        let d = doc(&["a", "b"]);
        let outline = run(&preset(StrategyKind::Outline, Some(StrategyKind::Ai)), &d, &OPTIONS).unwrap();
        assert_eq!(outline, PresetRun::NeedsAi { reason: Some("This PDF has no bookmarks".into()) });
        let pattern = run(&preset(StrategyKind::Pattern, Some(StrategyKind::Ai)), &d, &OPTIONS).unwrap();
        assert_eq!(pattern, PresetRun::NeedsAi { reason: Some("No split points were found".into()) });
    }

    #[test]
    fn falls_back_to_a_rule_strategy_and_says_why() {
        let d = doc(&["a", "", "b", "c"]);
        let run = run(&preset(StrategyKind::Outline, Some(StrategyKind::Blank)), &d, &OPTIONS).unwrap();
        assert_eq!(
            parts(run),
            (vec![(1, 1), (2, 2), (3, 4)], StrategyKind::Blank, false, Some("This PDF has no bookmarks".into()))
        );
    }

    #[test]
    fn without_a_fallback_the_first_error_is_returned() {
        let d = doc(&["a", "b"]);
        let err = run(&preset(StrategyKind::Outline, None), &d, &OPTIONS).unwrap_err();
        assert!(matches!(err, SplitError::NoOutline));
        // A fallback that finds nothing either reports its own error.
        let err = run(&preset(StrategyKind::Outline, Some(StrategyKind::Pattern)), &d, &OPTIONS).unwrap_err();
        assert!(matches!(err, SplitError::NothingFound));
    }

    #[test]
    fn ai_presets_need_the_ai() {
        let run = run(&preset(StrategyKind::Ai, None), &doc(&["a"]), &OPTIONS).unwrap();
        assert_eq!(run, PresetRun::NeedsAi { reason: None });
    }

    #[test]
    fn other_errors_do_not_fall_back() {
        let mut p = preset(StrategyKind::Pattern, Some(StrategyKind::Ai));
        p.pattern = Some("(".into());
        assert!(matches!(run(&p, &doc(&["a"]), &OPTIONS), Err(SplitError::InvalidPattern(_))));
        p.pattern = None;
        assert!(matches!(run(&p, &doc(&["a"]), &OPTIONS), Err(SplitError::Presets(_))));
    }

    #[test]
    fn blank_pages_use_the_threshold_and_can_be_dropped() {
        // "p. 2" has 3 visible characters: blank with a threshold of 3, not with 0.
        let d = doc(&["Chapter one", "p. 2", "Chapter two", "more"]);
        let blank = preset(StrategyKind::Blank, None);
        let keep = RunOptions { blank_threshold: 3, drop_separators: false };
        assert_eq!(parts(run(&blank, &d, &keep).unwrap()).0, vec![(1, 1), (2, 2), (3, 4)]);
        let drop = RunOptions { blank_threshold: 3, drop_separators: true };
        assert_eq!(parts(run(&blank, &d, &drop).unwrap()), (vec![(1, 1), (3, 4)], StrategyKind::Blank, true, None));
        assert!(matches!(run(&blank, &d, &OPTIONS), Err(SplitError::NothingFound)));
    }

    #[test]
    fn the_blank_threshold_is_capped() {
        // A page with more text than the cap is never blank, however high the threshold sent.
        let long = "x".repeat(crate::settings::MAX_BLANK_THRESHOLD as usize + 1);
        let d = doc(&[&long, "p. 2", &long, &long]);
        let huge = RunOptions { blank_threshold: 100_000, drop_separators: false };
        assert_eq!(parts(run(&preset(StrategyKind::Blank, None), &d, &huge).unwrap()).0, vec![(1, 1), (2, 2), (3, 4)]);
    }

    #[test]
    fn a_pdf_without_text_is_not_split_by_its_text() {
        let scan = doc(&["", " ", "\n"]);
        let blank = preset(StrategyKind::Blank, Some(StrategyKind::Ai));
        assert!(matches!(run(&blank, &scan, &OPTIONS), Err(SplitError::NoTextLayer)));
        let pattern = preset(StrategyKind::Pattern, Some(StrategyKind::Ai));
        assert!(matches!(run(&pattern, &scan, &OPTIONS), Err(SplitError::NoTextLayer)));
    }

    #[test]
    fn only_text_strategies_need_ocr() {
        assert!(reads_text(&preset(StrategyKind::Pattern, None)));
        assert!(reads_text(&preset(StrategyKind::Blank, None)));
        assert!(reads_text(&preset(StrategyKind::Ai, None)));
        assert!(reads_text(&preset(StrategyKind::Outline, Some(StrategyKind::Ai))));
        assert!(!reads_text(&preset(StrategyKind::Outline, None)));
        assert!(!reads_text(&preset(StrategyKind::Fixed, Some(StrategyKind::Outline))));
    }

    #[test]
    fn dropping_every_page_is_an_error() {
        let d = doc(&["a", "", ""]);
        let drop = RunOptions { blank_threshold: 1, drop_separators: true };
        let err = run(&preset(StrategyKind::Blank, None), &d, &drop).unwrap_err();
        assert_eq!(err.to_string(), "Every page is blank, so there are no parts to save");
    }

    /// A bundled preset on a real PDF, read by PDFium: the path `run_preset` takes.
    #[test]
    fn bundled_presets_split_real_pdfs() {
        use crate::pdf::{self, tests as pdfs};
        let extract = |doc| {
            pdfs::start_engine();
            let dir = tempfile::tempdir().unwrap();
            let (_, doc) = pdf::open_document(&pdfs::save(&dir, "a.pdf", doc), None).unwrap();
            pdf::extract_text(&doc, |_, _| std::ops::ControlFlow::Continue(())).unwrap()
        };
        let bundled = crate::presets::bundled();
        let book = bundled.iter().find(|p| p.id == "books-by-chapter").unwrap();

        let run = run(book, &extract(pdfs::outlined()), &OPTIONS).unwrap();
        let PresetRun::Parts { ranges, .. } = run else { panic!("expected parts, got {run:?}") };
        let ranges: Vec<_> = ranges.iter().map(|r| (r.start, r.end, r.title.as_str())).collect();
        assert_eq!(ranges, [(1, 2, "Chapter 1"), (3, 4, "Chapter 2")]);

        // Without bookmarks the preset falls back to the AI.
        let run = super::run(book, &extract(pdfs::document(3)), &OPTIONS).unwrap();
        assert_eq!(run, PresetRun::NeedsAi { reason: Some("This PDF has no bookmarks".into()) });
    }

    #[test]
    fn fixed_presets_split_every_n_pages() {
        let run = run(&preset(StrategyKind::Fixed, None), &doc(&["a", "b", "c"]), &OPTIONS).unwrap();
        assert_eq!(parts(run).0, vec![(1, 2), (3, 3)]);
        let mut p = preset(StrategyKind::Fixed, None);
        p.every = None;
        assert!(matches!(super::run(&p, &doc(&["a"]), &OPTIONS), Err(SplitError::Presets(_))));
    }
}
