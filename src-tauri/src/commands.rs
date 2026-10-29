//! Tauri commands called from the React interface.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager};

use crate::ai::claude::Claude;
use crate::ai::{self, response, AiCalls, AiRequest, Provider};
use crate::error::{Result, SplitError};
use crate::export::{self, ExportRequest, SavedParts};
use crate::naming;
use crate::keystore::{self, KeyStatus, OsStore};
use crate::pdf::PdfInfo;
use crate::presets::{self, Preset};
use crate::ranges::{self, Range};
use crate::settings::{self, Settings};
use crate::strategy::{DocumentText, FixedStrategy, OutlineEntry, Strategy};
use crate::user_presets::{self, PresetInput, PresetList};
use crate::viewer::Viewer;

/// The credential store entry for the Claude key (`keystore::PLATFORMS`).
const CLAUDE_PLATFORM: &str = "anthropic";

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

/// Runs a credential store call off the UI thread (NFR-02): the store may wait for the user to
/// unlock it.
async fn key_store<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| SplitError::ApiKey(format!("The credential store didn't respond ({e})")))?
}

/// Saves an API key in the OS credential store (FR-60, NFR-05). Only its last 4 characters come back.
#[tauri::command]
pub async fn save_api_key(platform: String, key: String) -> Result<KeyStatus> {
    key_store(move || keystore::save(&OsStore, &platform, &key)).await
}

/// Whether a key is saved for `platform`, and its last 4 characters. Never the key itself (NFR-05).
#[tauri::command]
pub async fn api_key_status(platform: String) -> Result<KeyStatus> {
    key_store(move || keystore::status(&OsStore, &platform)).await
}

#[tauri::command]
pub async fn delete_api_key(platform: String) -> Result<KeyStatus> {
    key_store(move || keystore::delete(&OsStore, &platform)).await
}

/// Sends one chunk of page excerpts to Claude and returns its reply text. `ai_split` parses and
/// validates replies (FR-33 to FR-36).
#[tauri::command]
pub async fn ask_ai(app: AppHandle, request: AiRequest) -> Result<String> {
    let claude = ai_provider(&app).await?;
    app.state::<AiCalls>().run(claude.complete(&request)).await
}

/// Sends every chunk of page excerpts and returns one validated range list for a `page_count`-page
/// PDF (FR-33 to FR-35). A reply that can't be used is asked for again once; if it still fails,
/// the error says what was wrong so the user can set the parts by hand (FR-36).
#[tauri::command]
pub async fn ai_split(app: AppHandle, chunks: Vec<AiRequest>, page_count: u32, allow_gaps: bool) -> Result<Vec<Range>> {
    let claude = ai_provider(&app).await?;
    app.state::<AiCalls>().run(response::split(&claude, &chunks, page_count, allow_gaps)).await
}

/// The AI provider the user chose. Only when AI is on and the user accepted the notice (FR-61,
/// FR-62). The key is read from the credential store for this call only (NFR-05).
async fn ai_provider(app: &AppHandle) -> Result<Claude> {
    let settings = get_settings(app.clone()).await?;
    if !settings.ai_enabled {
        return Err(SplitError::Ai("AI is turned off. Turn it on in Settings to use Smart mode with AI".into()));
    }
    if !settings.ai_consent {
        return Err(SplitError::Ai("Accept the AI notice before Splitr sends page excerpts to Claude".into()));
    }
    let platform = settings.ai_platform.as_str();
    if platform != CLAUDE_PLATFORM {
        // ChatGPT (#31) and the other platforms (#28 to #32) plug in here.
        return Err(SplitError::Ai(match platform {
            "chatgpt" => "Signing in with ChatGPT isn't available yet. Choose Claude under AI in Settings to use                           an API key for now"
                .into(),
            _ => "Smart mode can only use Claude for now. Choose Claude under AI in Settings".into(),
        }));
    }
    let model = settings
        .model_for(platform)
        .ok_or_else(|| SplitError::Ai("Enter a model name under AI in Settings".into()))?;
    let key = key_store(|| keystore::key_for(&OsStore, CLAUDE_PLATFORM)).await?;
    Claude::new(key, model)
}

/// The model each AI service uses when the user hasn't chosen one, for the settings screen.
#[tauri::command]
pub fn default_ai_models() -> BTreeMap<String, String> {
    ai::default_models()
}

/// Stops the AI calls in progress. They end with `SplitError::AiCancelled`.
#[tauri::command]
pub fn cancel_ai(app: AppHandle) {
    app.state::<AiCalls>().cancel();
}

#[tauri::command]
pub async fn delete_user_preset(app: AppHandle, id: String) -> Result<()> {
    let path = user_presets_path(&app)?;
    blocking(move || user_presets::delete(&path, &id)).await
}

/// Opens a PDF off the UI thread (NFR-02). `password` is only needed for encrypted files (FR-03).
/// The opened PDF becomes the one whose pages `render_page` shows.
#[tauri::command]
pub async fn open_pdf(app: AppHandle, path: String, password: Option<String>) -> Result<PdfInfo> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Viewer>().open(&path, password.as_deref()))
        .await
        .map_err(|e| SplitError::OpenFailed("The PDF".into(), e.to_string()))?
}

/// Page `page` (1-based) of the open PDF as JPEG bytes `width` pixels wide, rendered off the UI
/// thread and cached for the session (FR-10, FR-11, NFR-02).
#[tauri::command]
pub async fn render_page(app: AppHandle, path: String, page: u32, width: u32) -> Result<Response> {
    tauri::async_runtime::spawn_blocking(move || app.state::<Viewer>().render(&path, page, width))
        .await
        .map_err(|e| SplitError::RenderFailed(page, e.to_string()))?
        .map(Response::new)
}

/// The event `extract_text` sends while it reads pages: `{ done, total }`.
pub const EXTRACT_PROGRESS: &str = "extract-progress";

#[derive(Clone, Serialize)]
struct ExtractProgress {
    done: u32,
    total: u32,
}

/// What `extract_text` found. The page text stays in the backend for `run_preset` (#8).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TextSummary {
    page_count: u32,
    /// Pages with any visible text. Scanned pages have none.
    pages_with_text: u32,
    outline: Vec<OutlineEntry>,
}

/// Reads the page text and top-level bookmarks of the open PDF off the UI thread, sending
/// `extract-progress` events, so the rule strategies can run on it (FR-30 to FR-32, NFR-02).
/// `cancel_extract` stops it.
#[tauri::command]
pub async fn extract_text(app: AppHandle, path: String) -> Result<TextSummary> {
    tauri::async_runtime::spawn_blocking(move || {
        // Sending about 100 events is enough for a progress bar, however long the file.
        let text = app.state::<Viewer>().extract(&path, |done, total| {
            if done == total || done % total.div_ceil(100) == 0 {
                let _ = app.emit(EXTRACT_PROGRESS, ExtractProgress { done, total });
            }
        })?;
        let pages_with_text = text.pages.iter().filter(|p| p.chars().any(|c| !c.is_whitespace())).count();
        Ok(TextSummary {
            page_count: text.page_count,
            pages_with_text: u32::try_from(pages_with_text).unwrap_or(u32::MAX),
            outline: text.outline.clone(),
        })
    })
    .await
    .map_err(|e| SplitError::ExtractFailed(e.to_string()))?
}

/// Stops the text extraction in progress. It ends with `SplitError::ExtractCancelled`.
#[tauri::command]
pub fn cancel_extract(app: AppHandle) {
    app.state::<Viewer>().cancel_extract();
}

/// Runs an export off the UI thread (NFR-02): writing large parts takes a while.
async fn export_off_ui<T: Send + 'static>(work: impl FnOnce() -> Result<T> + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| SplitError::ExportFailed(format!("the export stopped unexpectedly ({e})")))?
}

/// Saves each part as its own PDF in the chosen folder, named from `request.names` (FR-50 to FR-52).
#[tauri::command]
pub async fn export_parts(request: ExportRequest) -> Result<SavedParts> {
    export_off_ui(move || export::export(&request)).await
}

/// Like `export_parts`, but bundles the parts into `<zip_name>.zip` in the folder (FR-53).
/// `zip_name` comes from `export_names`.
#[tauri::command]
pub async fn export_zip(request: ExportRequest, zip_name: String) -> Result<String> {
    export_off_ui(move || export::export_zip(&request, &zip_name)).await
}

/// File names (without extensions) for parts with these titles, from the user's template, and for
/// the ZIP (FR-51 to FR-53). The interface shows them as a preview and passes them to the export
/// commands, so the two can't disagree.
#[tauri::command]
pub fn export_names(template: String, original: String, titles: Vec<String>) -> naming::ExportNames {
    naming::export_names(&template, &original, &titles)
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
