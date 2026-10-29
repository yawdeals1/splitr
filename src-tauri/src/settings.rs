//! User settings stored as JSON in the app config folder (SRS 6.4).
//!
//! API keys never go here: they live only in the OS credential store (NFR-05).

use std::fs;
use std::io::ErrorKind;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

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
    /// Whether the user accepted the AI notice (FR-61).
    pub ai_consent: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            last_output_folder: None,
            name_template: "{original} - {index} - {title}".into(),
            blank_threshold: 0,
            ai_consent: false,
        }
    }
}

impl Settings {
    /// Replaces values the app can't use with their defaults.
    fn normalized(mut self) -> Self {
        if self.name_template.trim().is_empty() {
            self.name_template = Self::default().name_template;
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
        ai_consent: field(&fields, "aiConsent", defaults.ai_consent),
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
            ai_consent: true,
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
                "aiConsent": true, "blankThreshold": -1}"#,
        )
        .unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.theme, Theme::System);
        assert_eq!(loaded.blank_threshold, 0);
        assert_eq!(loaded.name_template, "my {index}");
        assert_eq!(loaded.last_output_folder.as_deref(), Some("D:/x"));
        assert!(loaded.ai_consent);
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
        fs::write(&path, r#"{"theme": "light", "nameTemplate": "  "}"#).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.theme, Theme::Light);
        assert_eq!(loaded.name_template, Settings::default().name_template);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn has_no_key_fields() {
        let json = serde_json::to_string(&Settings::default()).unwrap().to_lowercase();
        assert!(!json.contains("key") && !json.contains("token"));
    }
}
