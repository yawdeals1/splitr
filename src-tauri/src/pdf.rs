//! PDF engine (SRS 3): open, render thumbnails, extract text and outline, write parts.
//!
//! Phase 0 wires this to pdfium-render (render, text, outline) and lopdf (writing parts).
//! See the commented dependencies in Cargo.toml. The original file is never modified (FR-04).

use serde::Serialize;

use crate::error::{Result, SplitError};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfInfo {
    pub path: String,
    pub file_name: String,
    pub page_count: u32,
    pub size_bytes: u64,
}

pub fn open(_path: &str) -> Result<PdfInfo> {
    Err(SplitError::NotImplemented("Opening PDFs"))
}
