//! Tauri commands called from the React interface.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::error::{Result, SplitError};
use crate::pdf::{self, PdfInfo};
use crate::presets::{self, Preset};
use crate::ranges::{self, Range};
use crate::strategy::{DocumentText, FixedStrategy, Strategy};
use crate::user_presets::{self, PresetInput};

fn user_presets_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app.path().app_config_dir().map_err(|e| SplitError::Presets(e.to_string()))?;
    Ok(dir.join(user_presets::FILE_NAME))
}

/// Bundled presets followed by the user's own (FR-70, FR-71).
#[tauri::command]
pub fn list_presets(app: AppHandle) -> Vec<Preset> {
    let mut all = presets::bundled();
    if let Ok(path) = user_presets_path(&app) {
        all.extend(user_presets::load(&path));
    }
    all
}

/// Creates or edits a user preset (FR-71).
#[tauri::command]
pub fn save_user_preset(app: AppHandle, preset: PresetInput) -> Result<Preset> {
    user_presets::save(&user_presets_path(&app)?, preset)
}

#[tauri::command]
pub fn delete_user_preset(app: AppHandle, id: String) -> Result<()> {
    user_presets::delete(&user_presets_path(&app)?, &id)
}

#[tauri::command]
pub fn open_pdf(path: String) -> Result<PdfInfo> {
    pdf::open(&path)
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
