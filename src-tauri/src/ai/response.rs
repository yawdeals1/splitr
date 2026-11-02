//! Turns AI replies into one range list the app can trust (SRS 6.3, FR-33 to FR-36): strict JSON
//! parsing, a check of each chunk's answer with one retry, merging overlapping chunks, and a
//! final `ranges::validate`.

use serde::Deserialize;

use super::{AiRequest, Provider};
use crate::error::{Result, SplitError};
use crate::ranges::{self, Range};

/// One item of the SRS 6.3 reply. Every field is required.
#[derive(Deserialize)]
struct Part {
    start: u32,
    end: u32,
    title: String,
}

/// One chunk's checked answer, with the pages that chunk covered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChunkReply {
    pub first: u32,
    pub last: u32,
    pub ranges: Vec<Range>,
}

/// Parses a reply in the SRS 6.3 shape and checks it against the chunk's pages `first` to `last`
/// (FR-35). The only text allowed around the JSON array is a Markdown code fence, and before it a
/// `<think>` block, where open models on local servers and some platforms put their reasoning.
pub fn parse(reply: &str, first: u32, last: u32, allow_gaps: bool) -> Result<Vec<Range>> {
    let parts: Vec<Part> = serde_json::from_str(strip_fence(strip_thinking(reply))).map_err(|e| {
        SplitError::InvalidRanges(if e.is_data() {
            "each part needs a whole-number start and end and a title".into()
        } else {
            "the answer is not a JSON list of parts".into()
        })
    })?;
    let ranges: Vec<Range> = parts
        .into_iter()
        .map(|p| Range { start: p.start, end: p.end, title: p.title.trim().to_string() })
        .collect();
    ranges::validate_within(&ranges, first, last, allow_gaps)?;
    Ok(ranges)
}

/// The reply after a leading `<think>...</think>` block, or the whole reply if there is none.
fn strip_thinking(reply: &str) -> &str {
    let reply = reply.trim();
    reply.strip_prefix("<think>").and_then(|rest| rest.split_once("</think>")).map_or(reply, |(_, answer)| answer)
}

/// The text inside a ```json fence, or the whole reply if there is none.
fn strip_fence(reply: &str) -> &str {
    let reply = reply.trim();
    let Some(inner) = reply.strip_prefix("```").and_then(|r| r.strip_suffix("```")) else {
        return reply;
    };
    // Drop the language tag on the opening line.
    inner.split_once('\n').map_or(inner, |(_, body)| body).trim()
}

/// Joins chunk answers into one ascending, non-overlapping list (FR-34). Chunks are in page order
/// and may overlap. Each overlap is split at its middle: the earlier chunk decides where parts
/// start before that page, the later chunk from it on, since each saw more context there. A part
/// the later chunk carries over from before its share continues the earlier chunk's last part.
pub fn merge(chunks: &[ChunkReply]) -> Vec<Range> {
    let mut merged: Vec<Range> = Vec::new();
    for (i, chunk) in chunks.iter().enumerate() {
        let lo = if i == 0 { chunk.first } else { cut(&chunks[i - 1], chunk) };
        let hi = chunks.get(i + 1).map_or(chunk.last, |next| cut(chunk, next).saturating_sub(1)).min(chunk.last);
        for r in &chunk.ranges {
            let (start, end) = (r.start.max(lo), r.end.min(hi));
            if start > end {
                continue;
            }
            match merged.last_mut() {
                Some(prev) if r.start < lo && prev.end + 1 == start => prev.end = end,
                _ => merged.push(Range { start, end, title: r.title.clone() }),
            }
        }
    }
    merged
}

/// The first page `later` decides, in the middle of its overlap with `earlier`.
fn cut(earlier: &ChunkReply, later: &ChunkReply) -> u32 {
    if later.first > earlier.last {
        return later.first;
    }
    later.first + (earlier.last - later.first).div_ceil(2)
}

/// Sends every chunk, retrying a chunk once if its answer can't be used (FR-36), then merges the
/// answers and validates the result for a `page_count`-page PDF (FR-34, FR-35). Errors from the
/// provider itself, such as a bad key, are returned at once. An answer that still fails becomes
/// `SplitError::AiReply`, so the user can set the parts by hand. `progress(done, total)` gets the
/// chunks answered so far: once before the first request and after each chunk, for a progress bar
/// (NFR-02).
pub async fn split_with_progress<P: Provider>(
    provider: &P,
    chunks: &[AiRequest],
    page_count: u32,
    allow_gaps: bool,
    mut progress: impl FnMut(u32, u32),
) -> Result<Vec<Range>> {
    if chunks.is_empty() {
        return Err(SplitError::Ai("There are no pages to send to the AI".into()));
    }
    let total = u32::try_from(chunks.len()).unwrap_or(u32::MAX);
    progress(0, total);
    let mut replies = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let first = chunk.pages.iter().map(|p| p.page).min();
        let last = chunk.pages.iter().map(|p| p.page).max();
        let (Some(first), Some(last)) = (first, last) else {
            return Err(SplitError::Ai("There are no pages to send to the AI".into()));
        };
        let ranges = ask(provider, chunk, first, last, allow_gaps).await?;
        replies.push(ChunkReply { first, last, ranges });
        progress(u32::try_from(replies.len()).unwrap_or(u32::MAX), total);
    }
    replies.sort_by_key(|c| c.first);
    let ranges = merge(&replies);
    if ranges.is_empty() {
        return Err(SplitError::AiReply(1, page_count, "it has no parts".into()));
    }
    ranges::validate(&ranges, page_count, allow_gaps)
        .map_err(|e| SplitError::AiReply(1, page_count, lower_first(&e.to_string())))?;
    Ok(ranges)
}

/// One chunk's ranges, asking a second time if the first answer fails (FR-36).
async fn ask<P: Provider>(provider: &P, chunk: &AiRequest, first: u32, last: u32, allow_gaps: bool) -> Result<Vec<Range>> {
    let mut problem = String::new();
    for _ in 0..2 {
        let reply = provider.complete(chunk).await?;
        match parse(&reply, first, last, allow_gaps) {
            Ok(ranges) => return Ok(ranges),
            Err(e) => problem = e.to_string(),
        }
    }
    Err(SplitError::AiReply(first, last, lower_first(&problem)))
}

/// "Range 5-3 ends..." reads as "range 5-3 ends..." after "could not be used:".
fn lower_first(s: &str) -> String {
    let mut chars = s.chars();
    chars.next().map_or_else(String::new, |c| c.to_lowercase().chain(chars).collect())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;

    use super::*;
    use crate::ai::PageExcerpt;

    async fn split<P: Provider>(provider: &P, chunks: &[AiRequest], page_count: u32, allow_gaps: bool) -> Result<Vec<Range>> {
        split_with_progress(provider, chunks, page_count, allow_gaps, |_, _| {}).await
    }

    fn range(start: u32, end: u32, title: &str) -> Range {
        Range { start, end, title: title.into() }
    }

    fn chunk(first: u32, last: u32, ranges: Vec<Range>) -> ChunkReply {
        ChunkReply { first, last, ranges }
    }

    #[test]
    fn parses_the_srs_shape() {
        let reply = r#"[{"start":1,"end":1,"title":" Preface "},{"start":2,"end":54,"title":"Chapter 1"}]"#;
        assert_eq!(parse(reply, 1, 54, false).unwrap(), vec![range(1, 1, "Preface"), range(2, 54, "Chapter 1")]);
    }

    #[test]
    fn accepts_a_code_fence_and_nothing_else() {
        let fenced = "```json\n[{\"start\":1,\"end\":3,\"title\":\"A\"}]\n```";
        assert_eq!(parse(fenced, 1, 3, false).unwrap(), vec![range(1, 3, "A")]);
        let bare_fence = "```\n[{\"start\":1,\"end\":3,\"title\":\"A\"}]\n```";
        assert!(parse(bare_fence, 1, 3, false).is_ok());
        let chatty = "Here are the parts:\n[{\"start\":1,\"end\":3,\"title\":\"A\"}]";
        assert!(parse(chatty, 1, 3, false).is_err());
    }

    #[test]
    fn a_leading_think_block_is_not_part_of_the_answer() {
        let answer = "[{\"start\":1,\"end\":3,\"title\":\"A\"}]";
        assert!(parse(&format!("<think>\nPage 1 starts it [1]\n</think>\n\n{answer}"), 1, 3, false).is_ok());
        assert!(parse(&format!("<think>hmm</think>\n```json\n{answer}\n```"), 1, 3, false).is_ok());
        // Only before the answer, and only a finished one.
        assert!(parse(&format!("{answer}<think>hmm</think>"), 1, 3, false).is_err());
        assert!(parse(&format!("<think>{answer}"), 1, 3, false).is_err());
    }

    #[test]
    fn rejects_malformed_json() {
        let not_json = "the answer is not a JSON list of parts";
        let bad_item = "each part needs a whole-number start and end and a title";
        for (reply, message) in [
            ("", not_json),
            ("[{\"start\":1,\"end\":3,\"title\":\"A\"}", not_json),
            ("{\"start\":1,\"end\":3,\"title\":\"A\"}", bad_item),
            ("[{\"start\":1,\"end\":3}]", bad_item),
            ("[{\"start\":\"1\",\"end\":3,\"title\":\"A\"}]", bad_item),
            ("[{\"start\":1.5,\"end\":3,\"title\":\"A\"}]", bad_item),
            ("[{\"start\":-1,\"end\":3,\"title\":\"A\"}]", bad_item),
            ("[{\"start\":1,\"end\":3,\"title\":null}]", bad_item),
        ] {
            assert_eq!(parse(reply, 1, 3, true).unwrap_err().to_string(), message, "{reply}");
        }
    }

    #[test]
    fn rejects_overlaps_gaps_and_pages_outside_the_chunk() {
        let overlap = r#"[{"start":1,"end":5,"title":"A"},{"start":5,"end":10,"title":"B"}]"#;
        assert_eq!(parse(overlap, 1, 10, true).unwrap_err().to_string(), "Range 5-10 overlaps or is out of order");
        let gap = r#"[{"start":1,"end":4,"title":"A"},{"start":6,"end":10,"title":"B"}]"#;
        assert_eq!(parse(gap, 1, 10, false).unwrap_err().to_string(), "Pages 5 to 5 are not in any part");
        assert!(parse(gap, 1, 10, true).is_ok());
        let outside = r#"[{"start":11,"end":25,"title":"A"}]"#;
        assert_eq!(parse(outside, 11, 20, false).unwrap_err().to_string(), "Range 11-25 is outside pages 11 to 20");
        let before = r#"[{"start":0,"end":3,"title":"A"}]"#;
        assert!(parse(before, 1, 3, true).is_err());
        let backwards = r#"[{"start":3,"end":2,"title":"A"}]"#;
        assert!(parse(backwards, 1, 3, true).is_err());
        assert_eq!(parse("[]", 1, 3, false).unwrap_err().to_string(), "Pages 1 to 3 are not in any part");
        assert_eq!(parse("[]", 1, 3, true).unwrap(), vec![]);
    }

    #[test]
    fn merge_joins_a_part_that_crosses_chunks() {
        // Pages 1-12 and 9-20, overlap 9-12: chunk one decides before page 11, chunk two from it.
        let chunks = [
            chunk(1, 12, vec![range(1, 4, "Intro"), range(5, 12, "Chapter 1")]),
            chunk(9, 20, vec![range(9, 15, "Chapter 1 (cont)"), range(16, 20, "Chapter 2")]),
        ];
        assert_eq!(merge(&chunks), vec![range(1, 4, "Intro"), range(5, 15, "Chapter 1"), range(16, 20, "Chapter 2")]);
    }

    #[test]
    fn merge_lets_each_chunk_decide_its_half_of_the_overlap() {
        // Chunk one puts a start at page 10 (its half); chunk two's view of it is ignored.
        let early = [
            chunk(1, 12, vec![range(1, 9, "A"), range(10, 12, "B")]),
            chunk(9, 20, vec![range(9, 20, "A or B")]),
        ];
        assert_eq!(merge(&early), vec![range(1, 9, "A"), range(10, 20, "B")]);
        // A start at page 12 is chunk two's to decide, and chunk two doesn't see one.
        let late = [
            chunk(1, 12, vec![range(1, 11, "A"), range(12, 12, "B")]),
            chunk(9, 20, vec![range(9, 20, "A")]),
        ];
        assert_eq!(merge(&late), vec![range(1, 20, "A")]);
        // Chunk two starts a part exactly at its share.
        let at_cut = [
            chunk(1, 12, vec![range(1, 12, "A")]),
            chunk(9, 20, vec![range(9, 10, "A"), range(11, 20, "B")]),
        ];
        assert_eq!(merge(&at_cut), vec![range(1, 10, "A"), range(11, 20, "B")]);
    }

    #[test]
    fn merge_handles_three_chunks_touching_chunks_and_gaps() {
        let three = [
            chunk(1, 10, vec![range(1, 10, "A")]),
            chunk(9, 18, vec![range(9, 14, "A"), range(15, 18, "B")]),
            chunk(17, 25, vec![range(17, 25, "B")]),
        ];
        assert_eq!(merge(&three), vec![range(1, 14, "A"), range(15, 25, "B")]);
        let touching = [chunk(1, 5, vec![range(1, 5, "A")]), chunk(6, 9, vec![range(6, 9, "B")])];
        assert_eq!(merge(&touching), vec![range(1, 5, "A"), range(6, 9, "B")]);
        // Skipped pages stay skipped when the task allows it.
        let gaps = [
            chunk(1, 12, vec![range(2, 6, "A")]),
            chunk(9, 20, vec![range(13, 20, "B")]),
        ];
        assert_eq!(merge(&gaps), vec![range(2, 6, "A"), range(13, 20, "B")]);
        assert_eq!(merge(&[]), vec![]);
    }

    /// A provider that returns canned replies in order and counts the calls.
    struct Canned(RefCell<VecDeque<Result<String>>>);

    impl Canned {
        fn new(replies: Vec<Result<&str>>) -> Self {
            Self(RefCell::new(replies.into_iter().map(|r| r.map(String::from)).collect()))
        }
        fn left(&self) -> usize {
            self.0.borrow().len()
        }
    }

    impl Provider for Canned {
        async fn complete(&self, _request: &AiRequest) -> Result<String> {
            self.0.borrow_mut().pop_front().expect("no reply left")
        }
    }

    fn request(first: u32, last: u32) -> AiRequest {
        AiRequest {
            instruction: "Split by chapter".into(),
            pages: (first..=last).map(|page| PageExcerpt { page, head: String::new(), tail: String::new() }).collect(),
        }
    }

    fn run(provider: &Canned, chunks: &[AiRequest], page_count: u32, allow_gaps: bool) -> Result<Vec<Range>> {
        tauri::async_runtime::block_on(split(provider, chunks, page_count, allow_gaps))
    }

    #[test]
    fn split_merges_chunk_answers() {
        let provider = Canned::new(vec![
            Ok(r#"[{"start":1,"end":12,"title":"One"}]"#),
            Ok(r#"[{"start":9,"end":14,"title":"One"},{"start":15,"end":20,"title":"Two"}]"#),
        ]);
        let ranges = run(&provider, &[request(1, 12), request(9, 20)], 20, false).unwrap();
        assert_eq!(ranges, vec![range(1, 14, "One"), range(15, 20, "Two")]);
    }

    #[test]
    fn split_reports_each_answered_chunk() {
        let provider = Canned::new(vec![
            Ok(r#"[{"start":1,"end":12,"title":"One"}]"#),
            Ok(r#"[{"start":9,"end":20,"title":"One"}]"#),
        ]);
        let mut seen = Vec::new();
        let chunks = [request(1, 12), request(9, 20)];
        let split = split_with_progress(&provider, &chunks, 20, false, |done, total| seen.push((done, total)));
        tauri::async_runtime::block_on(split).unwrap();
        assert_eq!(seen, [(0, 2), (1, 2), (2, 2)]);
    }

    #[test]
    fn split_retries_a_bad_answer_once() {
        let provider = Canned::new(vec![Ok("Sure! Here you go."), Ok(r#"[{"start":1,"end":5,"title":"All"}]"#)]);
        assert_eq!(run(&provider, &[request(1, 5)], 5, false).unwrap(), vec![range(1, 5, "All")]);
        assert_eq!(provider.left(), 0);
    }

    #[test]
    fn split_gives_up_after_the_retry() {
        let provider = Canned::new(vec![
            Ok(r#"[{"start":1,"end":3,"title":"A"}]"#),
            Ok(r#"[{"start":1,"end":4,"title":"A"},{"start":4,"end":5,"title":"B"}]"#),
            Ok("never asked"),
        ]);
        let err = run(&provider, &[request(1, 5)], 5, false).unwrap_err();
        assert!(matches!(err, SplitError::AiReply(1, 5, _)));
        assert_eq!(
            err.to_string(),
            "The AI's answer for pages 1 to 5 could not be used: range 4-5 overlaps or is out of order. \
             Set the parts by hand instead"
        );
        assert_eq!(provider.left(), 1);
    }

    #[test]
    fn split_does_not_retry_provider_errors() {
        let provider = Canned::new(vec![Err(SplitError::Ai("Your key was rejected".into())), Ok("never asked")]);
        let err = run(&provider, &[request(1, 5)], 5, false).unwrap_err();
        assert_eq!(err.to_string(), "Your key was rejected");
        assert_eq!(provider.left(), 1);
    }

    #[test]
    fn split_checks_the_merged_list_against_the_whole_pdf() {
        // Each chunk is fine, but the chunks leave pages 6 to 8 out.
        let provider = Canned::new(vec![
            Ok(r#"[{"start":1,"end":5,"title":"A"}]"#),
            Ok(r#"[{"start":9,"end":10,"title":"B"}]"#),
        ]);
        let err = run(&provider, &[request(1, 5), request(9, 10)], 10, false).unwrap_err();
        assert_eq!(
            err.to_string(),
            "The AI's answer for pages 1 to 10 could not be used: pages 6 to 8 are not in any part. \
             Set the parts by hand instead"
        );
        let nothing = Canned::new(vec![Ok("[]")]);
        assert!(matches!(run(&nothing, &[request(1, 5)], 5, true), Err(SplitError::AiReply(1, 5, _))));
        assert!(run(&Canned::new(vec![]), &[], 5, true).is_err());
        assert!(run(&Canned::new(vec![]), &[request(1, 0)], 5, true).is_err());
    }
}
