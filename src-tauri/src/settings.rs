//! User settings stored as JSON in the app config folder (SRS 6.4).
//!
//! API keys never go here: they live only in the OS credential store (NFR-05).

use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::ai;
use crate::error::{Result, SplitError};

pub const FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// Everything in SRS 6.4. Missing fields take their default, so older files keep loading.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub theme: Theme,
    pub last_output_folder: Option<String>,
    /// File name template (FR-51).
    pub name_template: String,
    /// A page with at most this many visible characters counts as blank (FR-32).
    pub blank_threshold: u32,
    /// Read pages without a text layer with OCR before the text strategies run (FR-80).
    pub ocr_enabled: bool,
    /// AI features are on (FR-62). Off hides Smart mode's AI options and blocks every AI call.
    pub ai_enabled: bool,
    /// Whether the user accepted the AI notice (FR-61).
    pub ai_consent: bool,
    /// The AI service Smart mode uses: "chatgpt" for the ChatGPT account, or a key platform.
    pub ai_platform: String,
    /// Models the user chose, by service. A service that isn't here uses its default.
    pub ai_models: BTreeMap<String, String>,
    /// The address of the custom OpenAI-compatible platform (#30), such as
    /// "https://llm.example.com/v1". Checked when it is used.
    pub ai_custom_url: String,
    /// The address of the model server on this computer (#25). Blank means Ollama's.
    pub ai_local_url: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            last_output_folder: None,
            name_template: "{original} - {index} - {title}".into(),
            blank_threshold: 0,
            ocr_enabled: true,
            ai_enabled: true,
            ai_consent: false,
            ai_platform: ai::DEFAULT_PLATFORM.into(),
            ai_models: BTreeMap::new(),
            ai_custom_url: String::new(),
            ai_local_url: ai::local::DEFAULT_URL.into(),
        }
    }
}

impl Settings {
    /// The model to use with `platform`: the user's choice, else the default, else none.
    pub fn model_for(&self, platform: &str) -> Option<String> {
        self.ai_models
            .get(platform)
            .cloned()
            .or_else(|| ai::default_model(platform).map(String::from))
    }

    /// Replaces values the app can't use with their defaults.
    fn normalized(mut self) -> Self {
        if self.name_template.trim().is_empty() {
            self.name_template = Self::default().name_template;
        }
        if !ai::is_platform(&self.ai_platform) {
            self.ai_platform = Self::default().ai_platform;
        }
        // A blank model means the default, so it isn't kept.
        self.ai_models = std::mem::take(&mut self.ai_models)
            .into_iter()
            .map(|(platform, model)| (platform, model.trim().to_string()))
            .filter(|(platform, model)| ai::is_platform(platform) && !model.is_empty())
            .collect();
        self.ai_custom_url = self.ai_custom_url.trim().to_string();
        self.ai_local_url = self.ai_local_url.trim().to_string();
        if self.ai_local_url.is_empty() {
            self.ai_local_url = Self::default().ai_local_url;
        }
        if self.last_output_folder.as_deref().is_some_and(|f| f.trim().is_empty()) {
            self.last_output_folder = None;
        }
        self
    }
}

/// Reads settings. A missing file gives the defaults. Each field is read on its own, so one
/// unusable value (say a theme from a newer version) falls back to its default and the rest are
/// kept. A file that isn't JSON at all is copied to `settings.json.bak` before the defaults are
/// used, so the next save can't destroy it. A file that exists but can't be read is an error, so
/// the interface doesn't save defaults over it.
pub fn load(path: &Path) -> Result<Settings> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Settings::default()),
        Err(e) => return Err(SplitError::SettingsLoad(describe(path, &e))),
    };
    let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(&text) else {
        let _ = fs::copy(path, path.with_extension("json.bak"));
        return Ok(Settings::default());
    };
    let defaults = Settings::default();
    Ok(Settings {
        theme: field(&fields, "theme", defaults.theme),
        last_output_folder: field(&fields, "lastOutputFolder", defaults.last_output_folder),
        name_template: field(&fields, "nameTemplate", defaults.name_template),
        blank_threshold: field(&fields, "blankThreshold", defaults.blank_threshold),
        ocr_enabled: field(&fields, "ocrEnabled", defaults.ocr_enabled),
        ai_enabled: field(&fields, "aiEnabled", defaults.ai_enabled),
        ai_consent: field(&fields, "aiConsent", defaults.ai_consent),
        ai_platform: field(&fields, "aiPlatform", defaults.ai_platform),
        ai_models: field(&fields, "aiModels", defaults.ai_models),
        ai_custom_url: field(&fields, "aiCustomUrl", defaults.ai_custom_url),
        ai_local_url: field(&fields, "aiLocalUrl", defaults.ai_local_url),
    }
    .normalized())
}

/// The value of `key`, or `default` if it is missing or has the wrong type.
fn field<T: DeserializeOwned>(fields: &Map<String, Value>, key: &str, default: T) -> T {
    fields
        .get(key)
        .and_then(|v| T::deserialize(v).ok())
        .unwrap_or(default)
}

/// Writes settings to a temp file and renames it, so a failed write never leaves a half file.
/// Each save uses its own temp name, so saves that overlap can't write into the same file.
pub fn save(path: &Path, settings: &Settings) -> Result<Settings> {
    static NEXT_TMP: AtomicU64 = AtomicU64::new(0);
    let settings = settings.clone().normalized();
    let fail = |e: std::io::Error| SplitError::SettingsSave(describe(path, &e));
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(fail)?;
    }
    let json = serde_json::to_string_pretty(&settings)
        .map_err(|e| SplitError::SettingsSave(format!("they could not be converted to JSON ({e})")))?;
    let n = NEXT_TMP.fetch_add(1, Ordering::Relaxed);
    let tmp = path.with_extension(format!("json.{}-{n}.tmp", std::process::id()));
    fs::write(&tmp, json).map_err(fail)?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        fail(e)
    })?;
    Ok(settings)
}

/// A plain description of a file error that names the file and what the user can do.
fn describe(path: &Path, e: &std::io::Error) -> String {
    match e.kind() {
        ErrorKind::PermissionDenied => format!(
            "Splitr doesn't have permission to use {}. Check that the folder isn't read-only",
            path.display()
        ),
        _ => format!("{} could not be used ({e})", path.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A fresh folder per test under the system temp dir.
    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("splitr-settings-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = temp_dir("missing");
        assert_eq!(load(&dir.join(FILE_NAME)).unwrap(), Settings::default());
    }

    #[test]
    fn round_trips_through_disk() {
        let dir = temp_dir("roundtrip");
        let path = dir.join(FILE_NAME);
        let settings = Settings {
            theme: Theme::Dark,
            last_output_folder: Some("out".into()),
            name_template: "{index} - {title}".into(),
            blank_threshold: 5,
            ocr_enabled: false,
            ai_enabled: false,
            ai_consent: true,
            ai_platform: "anthropic".into(),
            ai_models: BTreeMap::from([("anthropic".into(), "claude-haiku-5-5".into())]),
            ai_custom_url: "https://llm.example.com/v1".into(),
            ai_local_url: "http://127.0.0.1:1234/v1".into(),
        };
        save(&path, &settings).unwrap();
        assert_eq!(load(&path).unwrap(), settings);
        let leftovers = fs::read_dir(&dir)
            .unwrap()
            .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".tmp"));
        assert_eq!(leftovers.count(), 0);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupt_file_gives_defaults_and_is_backed_up() {
        let dir = temp_dir("corrupt");
        let path = dir.join(FILE_NAME);
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(&path).unwrap(), Settings::default());
        assert_eq!(fs::read_to_string(path.with_extension("json.bak")).unwrap(), "{ not json");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn one_bad_field_keeps_the_others() {
        let dir = temp_dir("badfield");
        let path = dir.join(FILE_NAME);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            &path,
            r#"{"theme": "sepia", "nameTemplate": "my {index}", "lastOutputFolder": "D:/x",
                "aiConsent": true, "aiEnabled": "no", "blankThreshold": -1}"#,
        )
        .unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.theme, Theme::System);
        assert_eq!(loaded.blank_threshold, 0);
        assert_eq!(loaded.name_template, "my {index}");
        assert_eq!(loaded.last_output_folder.as_deref(), Some("D:/x"));
        assert!(loaded.ai_consent);
        assert!(loaded.ai_enabled);
        assert!(loaded.ocr_enabled, "OCR is on unless turned off");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unreadable_file_is_an_error() {
        // A folder where the file should be can't be read as text.
        let dir = temp_dir("unreadable");
        let path = dir.join(FILE_NAME);
        fs::create_dir_all(&path).unwrap();
        let err = load(&path).unwrap_err().to_string();
        assert!(err.contains("Could not load settings"), "{err}");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_fields_take_defaults() {
        let dir = temp_dir("partial");
        let path = dir.join(FILE_NAME);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            &path,
            r#"{"theme": "light", "nameTemplate": "  ", "aiPlatform": "nope", "aiLocalUrl": " ",
                "aiModels": {"anthropic": " ", "openai": " gpt-x ", "nope": "m"}}"#,
        )
        .unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.theme, Theme::Light);
        assert_eq!(loaded.name_template, Settings::default().name_template);
        assert_eq!(loaded.ai_platform, "chatgpt");
        assert_eq!(loaded.ai_models, BTreeMap::from([("openai".into(), "gpt-x".into())]));
        assert_eq!(loaded.model_for("anthropic").as_deref(), Some("claude-opus-5-5"));
        assert_eq!(loaded.model_for("openai").as_deref(), Some("gpt-x"));
        assert_eq!(loaded.model_for("custom"), None);
        assert_eq!(loaded.ai_local_url, "http://localhost:11434/v1");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn has_no_key_fields() {
        let json = serde_json::to_string(&Settings::default()).unwrap().to_lowercase();
        assert!(!json.contains("key") && !json.contains("token"));
    }
}
