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
    let dir = app.path().app_config_dir().map_err(|e| {
        SplitError::SettingsLoad(format!("Splitr could not find its settings folder ({e})"))
    })?;
    Ok(dir.join(settings::FILE_NAME))
}

/// SRS 6.4. Runs off the UI thread (NFR-02): the config folder may be on a slow or synced drive.
#[tauri::command]
pub async fn get_settings(app: AppHandle) -> Result<Settings> {
    let path = settings_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || settings::load(&path))
        .await
        .map_err(|e| SplitError::SettingsLoad(e.to_string()))?
}

/// Saves and returns the settings as stored, after replacing unusable values with defaults.
#[tauri::command]
pub async fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings> {
    let path = settings_path(&app)?;
    tauri::async_runtime::spawn_blocking(move || settings::save(&path, &settings))
        .await
        .map_err(|e| SplitError::SettingsSave(e.to_string()))?
}

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
