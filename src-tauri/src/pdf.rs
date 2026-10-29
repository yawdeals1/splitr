//! PDF engine (SRS 3): open, render thumbnails, extract text and outline, write parts.
//!
//! PDFium (via pdfium-render) renders, extracts text and reads the outline; lopdf writes parts.
//! The PDFium library ships as a Tauri resource in `pdfium/` (SRS 2.5, 5.3, NFR-12), fetched by
//! `scripts/fetch-pdfium.mjs`. The original file is never modified (FR-04).

use std::path::Path;
use std::sync::OnceLock;

use pdfium_render::prelude::Pdfium;
use serde::Serialize;

use crate::error::{Result, SplitError};

/// The PDFium instance loaded at startup, or why it failed to load.
static ENGINE: OnceLock<std::result::Result<Pdfium, String>> = OnceLock::new();

/// Loads the PDFium library from `dir` (the `pdfium` resource folder). Call once at startup.
/// A failure is kept, so every later PDF call reports it to the user.
pub fn init(dir: &Path) -> Result<()> {
    ENGINE
        .get_or_init(|| load(dir))
        .as_ref()
        .map(|_| ())
        .map_err(|e| SplitError::PdfEngine(e.clone()))
}

fn load(dir: &Path) -> std::result::Result<Pdfium, String> {
    let path = Pdfium::pdfium_platform_library_name_at_path(dir);
    if !path.is_file() {
        return Err(format!(
            "the PDFium library is missing ({}). Reinstall Splitr; developers run `npm run fetch-pdfium`",
            path.display()
        ));
    }
    let bindings = Pdfium::bind_to_library(&path)
        .map_err(|e| format!("the PDFium library at {} could not be loaded ({e})", path.display()))?;
    Ok(Pdfium::new(bindings))
}

/// The loaded PDFium instance, or a user-facing error if it failed to load.
pub fn engine() -> Result<&'static Pdfium> {
    match ENGINE.get() {
        Some(Ok(pdfium)) => Ok(pdfium),
        Some(Err(e)) => Err(SplitError::PdfEngine(e.clone())),
        None => Err(SplitError::PdfEngine("the PDFium library was not loaded".into())),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfInfo {
    pub path: String,
    pub file_name: String,
    pub page_count: u32,
    pub size_bytes: u64,
}

pub fn open(_path: &str) -> Result<PdfInfo> {
    engine()?;
    Err(SplitError::NotImplemented("Opening PDFs"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn missing_library_gives_a_clear_error() {
        let err = load(Path::new("does-not-exist")).unwrap_err();
        assert!(err.contains("PDFium library is missing"), "{err}");
        assert!(err.contains("npm run fetch-pdfium"), "{err}");
    }

    /// Needs `npm run fetch-pdfium` first; CI runs it before `cargo test`.
    #[test]
    fn loads_the_bundled_library() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("pdfium");
        init(&dir).unwrap();
        let doc = engine().unwrap().create_new_pdf().unwrap();
        assert_eq!(doc.pages().len(), 0);
    }
}
