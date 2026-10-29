//! Tauri commands called from the React interface.

use crate::error::{Result, SplitError};
use crate::pdf::{self, PdfInfo};
use crate::presets::{self, Preset};
use crate::ranges::{self, Range};
use crate::strategy::{DocumentText, FixedStrategy, Strategy};

#[tauri::command]
pub fn list_presets() -> Vec<Preset> {
    presets::bundled()
}

/// Opens a PDF off the UI thread (NFR-02). `password` is only needed for encrypted files (FR-03).
#[tauri::command]
pub async fn open_pdf(path: String, password: Option<String>) -> Result<PdfInfo> {
    tauri::async_runtime::spawn_blocking(move || pdf::open(&path, password.as_deref()))
        .await
        .map_err(|e| SplitError::OpenFailed("The PDF".into(), e.to_string()))?
}

#[tauri::command]
pub fn parse_custom_ranges(input: String, page_count: u32) -> Result<Vec<Range>> {
    ranges::parse_custom(&input, page_count)
}

#[tauri::command]
pub fn fixed_ranges(page_count: u32, every: Option<u32>, parts: Option<u32>) -> Result<Vec<Range>> {
    let strategy = match (every, parts) {
        (_, Some(n)) => FixedStrategy::Parts(n),
        (Some(n), None) => FixedStrategy::EveryN(n),
        (None, None) => FixedStrategy::EveryN(0),
    };
    strategy.propose(&DocumentText { page_count, ..Default::default() })
}

#[tauri::command]
pub fn validate_ranges(ranges: Vec<Range>, page_count: u32, allow_gaps: bool) -> Result<()> {
    ranges::validate(&ranges, page_count, allow_gaps)
}
