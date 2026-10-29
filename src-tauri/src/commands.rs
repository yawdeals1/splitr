//! Tauri commands called from the React interface.

use std::path::PathBuf;

use tauri::{AppHandle, Manager};

use crate::error::{Result, SplitError};
use crate::pdf::{self, PdfInfo};
use crate::presets::{self, Preset};
use crate::ranges::{self, Range};
use crate::strategy::{DocumentText, FixedStrategy, Strategy};
use crate::user_presets::{self, PresetInput, PresetList};

fn user_presets_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app.path().app_config_dir().map_err(|e| {
        SplitError::Presets(format!("Splitr could not find the folder where it keeps your presets ({e})."))
    })?;
    Ok(dir.join(user_presets::FILE_NAME))
}

/// Runs file work for a command off the UI thread (NFR-02). The config folder may be on a slow or
/// synced drive.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| SplitError::Presets(format!("Your presets could not be updated ({e}).")))?
}

/// Bundled presets followed by the user's own (FR-70, FR-71). If the user's presets can't be read,
/// the bundled ones are still listed and `problem` says why the others are missing.
#[tauri::command]
pub async fn list_presets(app: AppHandle) -> PresetList {
    let mut presets = presets::bundled();
    let user = match user_presets_path(&app) {
        Ok(path) => blocking(move || user_presets::load(&path)).await,
        Err(e) => Err(e),
    };
    let problem = match user {
        Ok(user) => {
            presets.extend(user);
            None
        }
        Err(e) => Some(e.to_string()),
    };
    PresetList { presets, problem }
}

/// Creates or edits a user preset (FR-71).
#[tauri::command]
pub async fn save_user_preset(app: AppHandle, preset: PresetInput) -> Result<Preset> {
    let path = user_presets_path(&app)?;
    blocking(move || user_presets::save(&path, preset)).await
}

#[tauri::command]
pub async fn delete_user_preset(app: AppHandle, id: String) -> Result<()> {
    let path = user_presets_path(&app)?;
    blocking(move || user_presets::delete(&path, &id)).await
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
