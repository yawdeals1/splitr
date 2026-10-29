//! User settings stored as JSON in the app config folder (SRS 6.4).
//!
//! API keys never go here: they live only in the OS credential store (NFR-05).

use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

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

/// Reads settings. A missing or corrupt file gives the defaults instead of an error.
pub fn load(path: &Path) -> Settings {
    fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Settings>(&text).ok())
        .unwrap_or_default()
        .normalized()
}

/// Writes settings to a temp file and renames it, so a failed write never leaves a half file.
pub fn save(path: &Path, settings: &Settings) -> Result<Settings> {
    let settings = settings.clone().normalized();
    let fail = |e: std::io::Error| SplitError::Settings(e.to_string());
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(fail)?;
    }
    let json = serde_json::to_string_pretty(&settings).map_err(|e| SplitError::Settings(e.to_string()))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(fail)?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        fail(e)
    })?;
    Ok(settings)
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
        assert_eq!(load(&dir.join(FILE_NAME)), Settings::default());
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
        assert_eq!(load(&path), settings);
        assert!(!path.with_extension("json.tmp").exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupt_file_gives_defaults() {
        let dir = temp_dir("corrupt");
        let path = dir.join(FILE_NAME);
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(load(&path), Settings::default());
        fs::write(&path, r#"{"theme": "purple"}"#).unwrap();
        assert_eq!(load(&path), Settings::default());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn missing_fields_take_defaults() {
        let dir = temp_dir("partial");
        let path = dir.join(FILE_NAME);
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, r#"{"theme": "light", "nameTemplate": "  "}"#).unwrap();
        let loaded = load(&path);
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
