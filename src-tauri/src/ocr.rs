//! OCR for pages without a text layer, such as scans (FR-80, #24), so the Pattern, Blank page and
//! AI strategies can read them. Uses the ocrs engine, which runs on this computer: no page leaves
//! it (NFR-06).
//!
//! The models ship as a Tauri resource in `ocr/`, fetched by `scripts/fetch-ocr-models.mjs`. They
//! load the first time a page is read, so starting the app stays fast.

use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use image::RgbImage;
use ocrs::{ImageSource, OcrEngine, OcrEngineParams};

use crate::error::{Result, SplitError};
use crate::strategy::DocumentText;

const DETECTION_MODEL: &str = "text-detection.onnx";
const RECOGNITION_MODEL: &str = "text-recognition.onnx";

/// The model folder, set once at startup.
static MODELS: OnceLock<PathBuf> = OnceLock::new();
/// The engine, loaded on first use, or why it failed to load.
static ENGINE: OnceLock<std::result::Result<OcrEngine, String>> = OnceLock::new();

/// Remembers where the models are (the `ocr` resource folder). Call once at startup.
pub fn init(dir: &Path) {
    let _ = MODELS.set(dir.to_path_buf());
}

fn engine() -> Result<&'static OcrEngine> {
    ENGINE
        .get_or_init(|| {
            let dir = MODELS.get().ok_or("the OCR models were not found")?;
            load(dir)
        })
        .as_ref()
        .map_err(|e| SplitError::Ocr(e.clone()))
}

fn load(dir: &Path) -> std::result::Result<OcrEngine, String> {
    let model = |name: &str| {
        let path = dir.join(name);
        if !path.is_file() {
            return Err(format!(
                "the OCR model {} is missing. Reinstall Splitr; developers run `npm run fetch-ocr`",
                path.display()
            ));
        }
        rten::Model::load_file(&path).map_err(|e| format!("the OCR model {} could not be loaded ({e})", path.display()))
    };
    OcrEngine::new(OcrEngineParams {
        detection_model: Some(model(DETECTION_MODEL)?),
        recognition_model: Some(model(RECOGNITION_MODEL)?),
        ..Default::default()
    })
    .map_err(|e| format!("the OCR engine could not start ({e})"))
}

/// The text in `image`, one line of text per line. Single characters on their own are left out:
/// they are nearly always specks on the scan.
pub fn recognize(image: &RgbImage) -> Result<String> {
    let engine = engine()?;
    let failed = |e: String| SplitError::Ocr(format!("a page could not be read ({e})"));
    let source = ImageSource::from_bytes(image.as_raw(), image.dimensions()).map_err(|e| failed(e.to_string()))?;
    let input = engine.prepare_input(source).map_err(|e| failed(e.to_string()))?;
    let text = engine.get_text(&input).map_err(|e| failed(e.to_string()))?;
    Ok(text.lines().map(str::trim).filter(|l| l.chars().count() > 1).collect::<Vec<_>>().join("\n"))
}

/// True if `page` has any visible text.
pub fn has_text(page: &str) -> bool {
    page.chars().any(|c| !c.is_whitespace())
}

/// The page numbers of `doc` with no visible text.
pub fn pages_without_text(doc: &DocumentText) -> Vec<u32> {
    (1..).zip(&doc.pages).filter(|(_, text)| !has_text(text)).map(|(n, _)| n).collect()
}

/// A copy of `doc` with the pages that have no text read by `read_page` (called with the 1-based
/// page number). `progress` gets the pages read and how many need reading; returning
/// `ControlFlow::Break` stops with `SplitError::ExtractCancelled` (NFR-02).
pub fn fill_missing(
    doc: &DocumentText,
    mut read_page: impl FnMut(u32) -> Result<String>,
    mut progress: impl FnMut(u32, u32) -> ControlFlow<()>,
) -> Result<DocumentText> {
    let missing = pages_without_text(doc);
    let total = u32::try_from(missing.len()).unwrap_or(u32::MAX);
    let mut pages = doc.pages.clone();
    for (done, page) in (1..).zip(missing) {
        pages[page as usize - 1] = read_page(page)?;
        if progress(done, total).is_break() {
            return Err(SplitError::ExtractCancelled);
        }
    }
    Ok(DocumentText { page_count: doc.page_count, pages, outline: doc.outline.clone() })
}

/// The message when OCR is off and pages of `doc` have no text (FR-80), or `None` if every page
/// has text.
pub fn missing_note(doc: &DocumentText) -> Option<String> {
    let missing = pages_without_text(doc);
    (!missing.is_empty()).then(|| {
        format!("No text found on {}, try turning on “Read scanned pages (OCR)”", page_list(&missing))
    })
}

/// "page 4", "pages 3 to 7" or "pages 1, 3 to 5 and 9" for sorted page numbers.
fn page_list(pages: &[u32]) -> String {
    let mut runs: Vec<(u32, u32)> = Vec::new();
    for &page in pages {
        match runs.last_mut() {
            Some((_, end)) if *end + 1 == page => *end = page,
            _ => runs.push((page, page)),
        }
    }
    let words: Vec<String> = runs
        .iter()
        .map(|&(start, end)| if start == end { start.to_string() } else { format!("{start} to {end}") })
        .collect();
    let noun = if pages.len() == 1 { "page" } else { "pages" };
    match words.split_last() {
        Some((last, [])) => format!("{noun} {last}"),
        Some((last, rest)) => format!("{noun} {} and {last}", rest.join(", ")),
        None => String::new(),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Points OCR at the models `npm run fetch-ocr` downloaded.
    pub(crate) fn start_ocr() {
        init(&Path::new(env!("CARGO_MANIFEST_DIR")).join("ocr"));
    }

    fn doc(pages: &[&str]) -> DocumentText {
        DocumentText { page_count: pages.len() as u32, pages: pages.iter().map(|p| p.to_string()).collect(), outline: Vec::new() }
    }

    #[test]
    fn lists_pages_in_runs() {
        assert_eq!(page_list(&[4]), "page 4");
        assert_eq!(page_list(&[3, 4, 5, 6, 7]), "pages 3 to 7");
        assert_eq!(page_list(&[1, 3, 4, 5, 9]), "pages 1, 3 to 5 and 9");
        assert_eq!(page_list(&[2, 8]), "pages 2 and 8");
    }

    #[test]
    fn the_note_names_the_pages_without_text() {
        assert_eq!(missing_note(&doc(&["a", "b"])), None);
        let note = missing_note(&doc(&["Invoice 1", " \n", "", "Invoice 2"])).unwrap();
        assert!(note.starts_with("No text found on pages 2 to 3, try turning on"), "{note}");
    }

    #[test]
    fn fills_only_the_pages_without_text_with_progress() {
        let mut read = Vec::new();
        let mut steps = Vec::new();
        let filled = fill_missing(
            &doc(&["Chapter 1", "", "text", "  "]),
            |page| {
                read.push(page);
                Ok(format!("scanned {page}"))
            },
            |done, total| {
                steps.push((done, total));
                ControlFlow::Continue(())
            },
        )
        .unwrap();
        assert_eq!(read, [2, 4]);
        assert_eq!(steps, [(1, 2), (2, 2)]);
        assert_eq!(filled.pages, ["Chapter 1", "scanned 2", "text", "scanned 4"]);
        assert_eq!(filled.page_count, 4);
    }

    #[test]
    fn stops_when_cancelled_or_a_page_fails() {
        let cancelled = fill_missing(&doc(&["", ""]), |_| Ok("x".into()), |_, _| ControlFlow::Break(()));
        assert!(matches!(cancelled, Err(SplitError::ExtractCancelled)));
        let failed = fill_missing(&doc(&[""]), |_| Err(SplitError::Ocr("broken".into())), |_, _| ControlFlow::Continue(()));
        assert!(failed.unwrap_err().to_string().contains("broken"));
    }

    #[test]
    fn missing_models_give_a_clear_error() {
        let err = load(Path::new("no-such-folder")).err().unwrap();
        assert!(err.contains("fetch-ocr"), "{err}");
    }
}
