//! Tauri commands called from the React interface.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::error::{Result, SplitError};
use crate::pdf::{self, PdfInfo};
use crate::presets::{self, Preset};
use crate::ranges::{self, Range};
use crate::settings::{self, Settings};
use crate::strategy::{DocumentText, FixedStrategy, Strategy};

fn settings_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app.path().app_config_dir().map_err(|e| SplitError::Settings(e.to_string()))?;
    Ok(dir.join(settings::FILE_NAME))
}

/// SRS 6.4. Falls back to defaults if the file is missing or corrupt.
#[tauri::command]
pub fn get_settings(app: AppHandle) -> Settings {
    settings_path(&app).map(|p| settings::load(&p)).unwrap_or_default()
}

/// Saves and returns the settings as stored, after replacing unusable values with defaults.
#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings> {
    settings::save(&settings_path(&app)?, &settings)
}

#[tauri::command]
pub fn list_presets() -> Vec<Preset> {
    presets::bundled()
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
