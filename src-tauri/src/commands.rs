//! Tauri commands called from the React interface.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager};

use crate::ai::{self, response, AiCalls, AiRequest, Provider};
use crate::browser;
use crate::chatgpt;
use crate::error::{Result, SplitError};
use crate::export::{self, ExportRequest, SavedParts};
use crate::key_detect::{self, Detection};
use crate::naming;
use crate::keystore::{self, KeyStatus, OsStore};
use crate::pdf::PdfInfo;
use crate::preset_run::{self, PresetRun, RunOptions};
use crate::presets::{self, Preset};
use crate::ranges::{self, Range};
use crate::settings::{self, Settings};
use crate::strategy::{AiRun, AiStrategy, DocumentText, FixedStrategy, OutlineEntry, Strategy, cancelled_is_ok};
use crate::user_presets::{self, PresetInput, PresetList};
use crate::viewer::Viewer;

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

/// Every platform with a saved key, and each key's last 4 characters (#27, NFR-05).
#[tauri::command]
pub async fn saved_api_keys() -> Result<Vec<KeyStatus>> {
    key_store(|| keystore::saved(&OsStore)).await
}

/// Checks the saved key for `platform` with one call to that platform only (#27, NFR-05).
#[tauri::command]
pub async fn verify_api_key(app: AppHandle, platform: String) -> Result<ai::verify::KeyCheck> {
    let settings = get_settings(app).await?;
    let key = {
        let platform = platform.clone();
        key_store(move || keystore::key_for(&OsStore, &platform)).await?
    };
    ai::verify::verify(&platform, &settings.ai_custom_url, &key).await
}

/// Sends one chunk of page excerpts to the chosen AI platform and returns its reply text. `ai_split`
/// parses and validates replies (FR-33 to FR-36).
#[tauri::command]
pub async fn ask_ai(app: AppHandle, request: AiRequest) -> Result<String> {
    let provider = ai_provider(&app).await?;
    app.state::<AiCalls>().run(provider.complete(&request)).await
}

/// Sends every chunk of page excerpts and returns one validated range list for a `page_count`-page
/// PDF (FR-33 to FR-35). A reply that can't be used is asked for again once; if it still fails,
/// the error says what was wrong so the user can set the parts by hand (FR-36).
#[tauri::command]
pub async fn ai_split(app: AppHandle, chunks: Vec<AiRequest>, page_count: u32, allow_gaps: bool) -> Result<Vec<Range>> {
    let provider = ai_provider(&app).await?;
    app.state::<AiCalls>().run(response::split(&provider, &chunks, page_count, allow_gaps)).await
}

/// The AI provider the user chose. Only when AI is on and, for a service off this computer, the
/// user accepted the notice (FR-61, FR-62). A local model needs no notice: nothing leaves the
/// computer (FR-81). The key is read from the credential store for this call only (NFR-05).
async fn ai_provider(app: &AppHandle) -> Result<ai::Chosen> {
    let settings = get_settings(app.clone()).await?;
    if !settings.ai_enabled {
        return Err(SplitError::Ai("AI is turned off. Turn it on in Settings to use Smart mode with AI".into()));
    }
    if settings.ai_platform == ai::local::LOCAL {
        let model = settings
            .model_for(ai::local::LOCAL)
            .ok_or_else(|| SplitError::Ai("Choose the local model to use under AI in Settings".into()))?;
        return Ok(ai::Chosen::Compatible(ai::local::client(&settings.ai_local_url, model)?));
    }
    if !settings.ai_consent {
        return Err(SplitError::Ai("Accept the AI notice before Splitr sends page excerpts to an AI service".into()));
    }
    let platform = settings.ai_platform.as_str();
    if !ai::is_supported(platform) {
        return Err(ai::unsupported(platform));
    }
    let model = settings
        .model_for(platform)
        .ok_or_else(|| SplitError::Ai("Enter a model name under AI in Settings".into()))?;
    if platform == "chatgpt" {
        let token = chatgpt_token().await?;
        return Ok(ai::Chosen::ChatGpt(ai::chatgpt::ChatGpt::new(token, model)?));
    }
    let key = {
        let platform = platform.to_string();
        key_store(move || keystore::key_for(&OsStore, &platform)).await?
    };
    ai::provider(platform, &settings.ai_custom_url, key, model)
}

fn chatgpt_registration_path(app: &AppHandle) -> Result<PathBuf> {
    let dir = app.path().app_config_dir().map_err(|e| {
        SplitError::Ai(format!("Splitr could not find the folder for its ChatGPT registration ({e})"))
    })?;
    Ok(dir.join(chatgpt::FILE_NAME))
}

/// Carries the sign-in link to the interface.
const CHATGPT_SIGN_IN: &str = "chatgpt-sign-in";

/// Whether a ChatGPT account is connected (#31), with its name, email and plan. Never a token (NFR-05).
#[tauri::command]
pub async fn chatgpt_status() -> Result<chatgpt::Status> {
    key_store(|| Ok(chatgpt::Status::of(chatgpt::load_session(&OsStore)?.as_ref()))).await
}

/// Connects a ChatGPT account (#31). OpenAI's sign-in page opens in the system browser, never in
/// Splitr, and the tokens go to the OS credential store. `cancel_ai` stops the wait. The link also
/// goes to the interface, because no system can promise that a browser appeared. It holds no token.
#[tauri::command]
pub async fn chatgpt_connect(app: AppHandle) -> Result<chatgpt::Status> {
    let path = chatgpt_registration_path(&app)?;
    let mut registration = {
        let path = path.clone();
        blocking(move || chatgpt::Registration::load(&path)).await?
    };
    // The host id is saved before the first sign-in, so every sign-in from here on uses it.
    {
        let (registration, path) = (registration.clone(), path.clone());
        blocking(move || registration.save(&path)).await?;
    }
    let open = |url: &str| {
        let _ = app.emit(CHATGPT_SIGN_IN, url);
        browser::open(&app, url);
        Ok(())
    };
    let result = app
        .state::<AiCalls>()
        .run(chatgpt::connect(&chatgpt::Endpoints::default(), &mut registration, open))
        .await;
    // The client id OpenAI issued is kept even if a later step failed, so the next try reuses it.
    blocking(move || registration.save(&path)).await?;
    let session = result?;
    key_store(move || {
        chatgpt::save_session(&OsStore, &session)?;
        Ok(chatgpt::Status::of(Some(&session)))
    })
    .await
}

/// Ends the ChatGPT connection (#31): revokes the refresh token at OpenAI, then forgets the tokens
/// on this computer. They are forgotten even if OpenAI can't be reached, and the error says so.
#[tauri::command]
pub async fn chatgpt_disconnect() -> Result<chatgpt::Status> {
    let session = key_store(|| chatgpt::load_session(&OsStore)).await?;
    let revoked = match session.as_ref().and_then(|s| Some((s.client_id.clone(), s.refresh_token.clone()?))) {
        Some((client_id, token)) => {
            let endpoints = chatgpt::Endpoints::default();
            chatgpt::oauth::revoke(&chatgpt::oauth::client()?, &endpoints.revoke, &client_id, &token).await
        }
        None => Ok(()),
    };
    key_store(|| chatgpt::delete_session(&OsStore)).await?;
    revoked.map(|()| chatgpt::Status::default()).map_err(|e| {
        SplitError::Ai(format!(
            "ChatGPT is disconnected on this computer, but Splitr couldn't end the connection at OpenAI ({e}). Remove \
             Splitr in ChatGPT under Settings to end it there too"
        ))
    })
}

/// A current access token for the connected ChatGPT account, refreshed and saved again if it was
/// about to expire. Refresh tokens rotate, so the new one must replace the old one.
async fn chatgpt_token() -> Result<String> {
    let session = key_store(|| chatgpt::load_session(&OsStore))
        .await?
        .ok_or_else(|| SplitError::Ai("Connect your ChatGPT account under AI in Settings first".into()))?;
    if !session.plan_usage() {
        return Err(SplitError::Ai(
            "Splitr may not use your ChatGPT plan yet. Disconnect and connect ChatGPT again and allow plan usage, \
             or use an API key"
                .into(),
        ));
    }
    let (session, changed) = match chatgpt::fresh(&chatgpt::Endpoints::default(), session).await {
        Ok(fresh) => fresh,
        Err(e) => {
            // A refresh token that no longer works means the connection has ended.
            if e.to_string() == chatgpt::oauth::SIGNED_OUT {
                key_store(|| chatgpt::delete_session(&OsStore)).await?;
            }
            return Err(e);
        }
    };
    if changed {
        let saved = session.clone();
        key_store(move || chatgpt::save_session(&OsStore, &saved)).await?;
    }
    Ok(session.access_token)
}

/// The models an OpenAI-compatible platform (#30), OpenCode Go (#32) or the local model server (#25)
/// offers, for the settings screen. Runs only when the user asks, and sends the platform's saved key
/// to that platform only (NFR-05). The local server gets no key.
#[tauri::command]
pub async fn list_ai_models(app: AppHandle, platform: String) -> Result<Vec<String>> {
    let local = platform == ai::local::LOCAL;
    if !ai::compatible::is_compatible(&platform) && platform != "opencode-go" && !local {
        return Err(SplitError::Ai("Splitr can't list the models of this AI service. Type the model name".into()));
    }
    let settings = get_settings(app.clone()).await?;
    if local {
        return app.state::<AiCalls>().run(ai::local::models(&settings.ai_local_url)).await;
    }
    let key = {
        let platform = platform.clone();
        key_store(move || keystore::key_for(&OsStore, &platform)).await?
    };
    if platform == "opencode-go" {
        return app.state::<AiCalls>().run(ai::opencode_go::models(key)).await;
    }
    let client = ai::compatible::ChatCompletions::new(&platform, &settings.ai_custom_url, key, String::new())?;
    app.state::<AiCalls>().run(client.models()).await
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
    /// True while OCR reads the pages that have no text (FR-80); `total` is then those pages.
    ocr: bool,
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
        let text = extract_with_progress(&app, &path, false)?;
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

/// The open PDF's text, read once and then kept (#7). With `ocr`, the pages without text are then
/// read by OCR (FR-80). Sends `extract-progress` events while it reads.
fn extract_with_progress(app: &AppHandle, path: &str, ocr: bool) -> Result<Arc<DocumentText>> {
    // Sending about 100 events is enough for a progress bar, however long the file.
    let progress = |done: u32, total: u32| {
        if done == total || done.is_multiple_of(total.div_ceil(100)) {
            let _ = app.emit(EXTRACT_PROGRESS, ExtractProgress { done, total, ocr: false });
        }
    };
    let viewer = app.state::<Viewer>();
    if !ocr {
        return viewer.extract(path, progress);
    }
    // OCR takes a second or more a page, so every page is reported.
    viewer.extract_with_ocr(path, progress, |done, total| {
        let _ = app.emit(EXTRACT_PROGRESS, ExtractProgress { done, total, ocr: true });
    })
}

/// When OCR is off and the open PDF has pages without text, the message that says so (FR-80).
/// The interface shows it after a split by the page text. Uses the text already read.
#[tauri::command]
pub async fn text_note(app: AppHandle, path: String) -> Result<Option<String>> {
    if get_settings(app.clone()).await?.ocr_enabled {
        return Ok(None);
    }
    tauri::async_runtime::spawn_blocking(move || {
        let text = app.state::<Viewer>().extract(&path, |_, _| ())?;
        Ok(crate::ocr::missing_note(&text))
    })
    .await
    .map_err(|e| SplitError::ExtractFailed(e.to_string()))?
}

/// Runs the preset `preset_id` on the open PDF at `path`: its strategy, then its fallback when the
/// first finds nothing (FR-30 to FR-32). Reads the text first if `extract_text` hasn't, off the UI
/// thread and with `extract-progress` events (NFR-02). The parts have passed `ranges::validate`
/// (FR-35); an AI preset, or an AI fallback, comes back as `NeedsAi`.
#[tauri::command]
pub async fn run_preset(app: AppHandle, path: String, preset_id: String, options: RunOptions) -> Result<PresetRun> {
    let preset = list_presets(app.clone())
        .await
        .presets
        .into_iter()
        .find(|p| p.id == preset_id)
        .ok_or_else(|| SplitError::Presets("This preset is no longer available. Choose another one".into()))?;
    let ocr = get_settings(app.clone()).await?.ocr_enabled && preset_run::reads_text(&preset);
    tauri::async_runtime::spawn_blocking(move || {
        let text = extract_with_progress(&app, &path, ocr)?;
        match preset_run::run(&preset, &text, &options) {
            Err(SplitError::NoTextLayer) if ocr => Err(SplitError::NoTextEvenWithOcr),
            result => result,
        }
    })
    .await
    .map_err(|e| SplitError::ExtractFailed(e.to_string()))?
}

/// The event `run_ai` sends as chunks are answered: `{ done, total }`.
pub const AI_PROGRESS: &str = "ai-progress";

/// Splits the open PDF at `path` with the AI (FR-30, FR-33 to FR-36): with the prompt of the
/// preset `preset_id`, or with the user's custom `prompt`. Nothing is read or sent unless AI is on
/// and the user accepted the notice (FR-61, FR-62). Sends `extract-progress` events while it reads
/// the text, then `ai-progress` events (NFR-02). `cancel_ai` and `cancel_extract` end it with
/// `AiRun::Cancelled`.
#[tauri::command]
pub async fn run_ai(app: AppHandle, path: String, preset_id: Option<String>, prompt: Option<String>) -> Result<AiRun> {
    let instruction = match preset_id {
        Some(id) => list_presets(app.clone())
            .await
            .presets
            .into_iter()
            .find(|p| p.id == id)
            .ok_or_else(|| SplitError::Presets("This preset is no longer available. Choose another one".into()))?
            .prompt,
        None => prompt.unwrap_or_default(),
    };
    let provider = ai_provider(&app).await?;
    let ocr = get_settings(app.clone()).await?.ocr_enabled;
    let result = async {
        let text = {
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || extract_with_progress(&app, &path, ocr))
                .await
                .map_err(|e| SplitError::ExtractFailed(e.to_string()))??
        };
        let strategy = AiStrategy { instruction };
        let progress = |done, total| {
            let _ = app.emit(AI_PROGRESS, ExtractProgress { done, total, ocr: false });
        };
        app.state::<AiCalls>().run(strategy.propose(&provider, &text, progress)).await
    }
    .await;
    cancelled_is_ok(result)
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

/// Says which platform a pasted key belongs to, from its prefix only (#27). No network call.
#[tauri::command]
pub fn detect_api_key(key: String) -> Detection {
    key_detect::detect(&key)
}
