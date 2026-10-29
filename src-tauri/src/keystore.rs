//! API keys in the OS credential store (FR-60, NFR-05): Windows Credential Manager, the macOS
//! Keychain, or the Secret Service on Linux. One entry per platform.
//!
//! A key never goes to the settings file, the logs or the interface. After saving, the interface
//! only learns whether a key is saved and its last 4 characters. Error messages never include it.

use serde::Serialize;

use crate::error::{Result, SplitError};

/// The service name every entry is stored under.
const SERVICE: &str = "Splitr";

/// Platforms a key can be saved for, with the name shown to the user. The ids match
/// `key_detect::Platform` (#27).
const PLATFORMS: &[(&str, &str)] = &[
    ("anthropic", "Claude"),
    ("openai", "OpenAI"),
    ("gemini", "Gemini"),
    ("openrouter", "OpenRouter"),
    ("nvidia", "NVIDIA"),
    ("groq", "Groq"),
    ("xai", "xAI"),
    ("deepseek", "DeepSeek"),
    ("mistral", "Mistral"),
    ("together", "Together AI"),
    ("opencode-go", "OpenCode Go"),
    ("custom", "the custom platform"),
];

/// Real keys are 20 to a few hundred characters. The limits catch a wrong paste, and the minimum
/// keeps the last 4 characters from giving away much of the key.
const MIN_LEN: usize = 8;
const MAX_LEN: usize = 1000;

/// What the interface may know about a saved key (NFR-05).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyStatus {
    pub platform: String,
    /// The last 4 characters, or `None` when no key is saved.
    pub last4: Option<String>,
}

/// Where keys are kept. The OS store in the app, an in-memory one in tests.
pub trait SecretStore {
    fn get(&self, platform: &str) -> Result<Option<String>>;
    fn set(&self, platform: &str, secret: &str) -> Result<()>;
    /// Removing a key that isn't there is not an error.
    fn delete(&self, platform: &str) -> Result<()>;
}

/// The operating system's credential store, through the `keyring` crate.
pub struct OsStore;

impl OsStore {
    fn entry(platform: &str) -> Result<keyring::Entry> {
        keyring::Entry::new(SERVICE, platform).map_err(store_error)
    }
}

impl SecretStore for OsStore {
    fn get(&self, platform: &str) -> Result<Option<String>> {
        match Self::entry(platform)?.get_password() {
            Ok(secret) => Ok(Some(secret)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(store_error(e)),
        }
    }

    fn set(&self, platform: &str, secret: &str) -> Result<()> {
        Self::entry(platform)?.set_password(secret).map_err(store_error)
    }

    fn delete(&self, platform: &str) -> Result<()> {
        match Self::entry(platform)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(store_error(e)),
        }
    }
}

/// A message for a credential store failure. Only the variants that describe the store itself
/// are shown in full: `BadEncoding` carries the stored bytes, so it must never be printed.
fn store_error(e: keyring::Error) -> SplitError {
    let detail = match &e {
        keyring::Error::PlatformFailure(inner) | keyring::Error::NoStorageAccess(inner) => {
            format!(" ({inner})")
        }
        _ => String::new(),
    };
    SplitError::ApiKey(format!(
        "Splitr couldn't use your system's credential store{detail}. On Linux, check that a \
         keyring such as GNOME Keyring or KWallet is running and unlocked"
    ))
}

/// The name shown to the user for a platform id, or an error for an unknown one.
fn label(platform: &str) -> Result<&'static str> {
    PLATFORMS
        .iter()
        .find(|(id, _)| *id == platform)
        .map(|(_, name)| *name)
        .ok_or_else(|| SplitError::ApiKey(format!("Splitr doesn't support the AI platform \"{platform}\"")))
}

fn last4(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    chars[chars.len().saturating_sub(4)..].iter().collect()
}

/// Checks a pasted key and trims the spaces and line breaks a copy often adds.
fn clean(platform: &str, key: &str) -> Result<String> {
    let name = label(platform)?;
    let key = key.trim();
    if key.is_empty() {
        return Err(SplitError::ApiKey(format!("Paste your {name} API key first")));
    }
    if key.chars().any(char::is_whitespace) {
        return Err(SplitError::ApiKey(
            "An API key can't contain spaces or line breaks. Copy the key again".into(),
        ));
    }
    let len = key.chars().count();
    if !(MIN_LEN..=MAX_LEN).contains(&len) {
        return Err(SplitError::ApiKey(format!(
            "This doesn't look like a {name} API key: it is {len} characters long. Copy the whole key again"
        )));
    }
    Ok(key.to_string())
}

/// Saves the key for `platform`, replacing any earlier one, and reads it back to confirm the
/// store kept it (FR-60).
pub fn save(store: &dyn SecretStore, platform: &str, key: &str) -> Result<KeyStatus> {
    let key = clean(platform, key)?;
    store.set(platform, &key)?;
    if store.get(platform)?.as_deref() != Some(key.as_str()) {
        return Err(SplitError::ApiKey(
            "Your system's credential store didn't keep the key. Try again, or check that it is unlocked".into(),
        ));
    }
    Ok(KeyStatus { platform: platform.into(), last4: Some(last4(&key)) })
}

/// Whether a key is saved for `platform`, and its last 4 characters.
pub fn status(store: &dyn SecretStore, platform: &str) -> Result<KeyStatus> {
    label(platform)?;
    let last4 = store.get(platform)?.map(|key| last4(&key));
    Ok(KeyStatus { platform: platform.into(), last4 })
}

/// Removes the key for `platform`.
pub fn delete(store: &dyn SecretStore, platform: &str) -> Result<KeyStatus> {
    label(platform)?;
    store.delete(platform)?;
    Ok(KeyStatus { platform: platform.into(), last4: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    struct MemoryStore(RefCell<HashMap<String, String>>);

    impl SecretStore for MemoryStore {
        fn get(&self, platform: &str) -> Result<Option<String>> {
            Ok(self.0.borrow().get(platform).cloned())
        }
        fn set(&self, platform: &str, secret: &str) -> Result<()> {
            self.0.borrow_mut().insert(platform.into(), secret.into());
            Ok(())
        }
        fn delete(&self, platform: &str) -> Result<()> {
            self.0.borrow_mut().remove(platform);
            Ok(())
        }
    }

    /// A store that accepts writes but never keeps them.
    struct ForgetfulStore;

    impl SecretStore for ForgetfulStore {
        fn get(&self, _: &str) -> Result<Option<String>> {
            Ok(None)
        }
        fn set(&self, _: &str, _: &str) -> Result<()> {
            Ok(())
        }
        fn delete(&self, _: &str) -> Result<()> {
            Ok(())
        }
    }

    const KEY: &str = "sk-ant-api03-abcdefghijklmnop-WXYZ";

    #[test]
    fn save_trims_and_shows_only_last_four() {
        let store = MemoryStore::default();
        let status = save(&store, "anthropic", &format!("  {KEY}\n")).unwrap();
        assert_eq!(status.last4.as_deref(), Some("WXYZ"));
        assert_eq!(store.get("anthropic").unwrap().as_deref(), Some(KEY));
        let json = serde_json::to_string(&status).unwrap();
        assert!(!json.contains("abcdefgh"), "{json}");
    }

    #[test]
    fn status_reports_saved_and_missing_keys() {
        let store = MemoryStore::default();
        assert_eq!(status(&store, "anthropic").unwrap().last4, None);
        save(&store, "anthropic", KEY).unwrap();
        assert_eq!(status(&store, "anthropic").unwrap().last4.as_deref(), Some("WXYZ"));
        assert_eq!(status(&store, "openai").unwrap().last4, None, "keys are per platform");
    }

    #[test]
    fn saving_again_replaces_the_key() {
        let store = MemoryStore::default();
        save(&store, "anthropic", KEY).unwrap();
        save(&store, "anthropic", "sk-ant-api03-another-key-1234").unwrap();
        assert_eq!(status(&store, "anthropic").unwrap().last4.as_deref(), Some("1234"));
    }

    #[test]
    fn delete_removes_the_key_and_is_safe_to_repeat() {
        let store = MemoryStore::default();
        save(&store, "anthropic", KEY).unwrap();
        assert_eq!(delete(&store, "anthropic").unwrap().last4, None);
        assert_eq!(store.get("anthropic").unwrap(), None);
        delete(&store, "anthropic").unwrap();
    }

    #[test]
    fn rejects_bad_pastes_without_echoing_them() {
        let store = MemoryStore::default();
        for bad in ["", "   ", "short", "sk-ant has spaces in it", &"x".repeat(MAX_LEN + 1)] {
            let err = save(&store, "anthropic", bad).unwrap_err().to_string();
            assert!(bad.trim().is_empty() || !err.contains(bad.trim()), "{err}");
        }
        assert!(store.0.borrow().is_empty());
    }

    #[test]
    fn rejects_unknown_platforms() {
        let store = MemoryStore::default();
        let err = save(&store, "../evil", KEY).unwrap_err().to_string();
        assert!(err.contains("doesn't support"), "{err}");
        assert!(status(&store, "nope").is_err());
        assert!(delete(&store, "nope").is_err());
        assert!(store.0.borrow().is_empty());
    }

    #[test]
    fn a_store_that_loses_the_key_is_an_error() {
        let err = save(&ForgetfulStore, "anthropic", KEY).unwrap_err().to_string();
        assert!(err.contains("didn't keep the key") && !err.contains(KEY), "{err}");
    }

    #[test]
    fn last4_counts_characters_not_bytes() {
        assert_eq!(last4("abcdéfgh"), "éfgh");
        assert_eq!(last4("abcdefgé"), "efgé");
    }

    /// Run with `cargo test -- --ignored` on a desktop with a credential store.
    #[test]
    #[ignore = "uses the real OS credential store"]
    fn os_store_round_trip() {
        let platform = "splitr-test-entry";
        OsStore.delete(platform).unwrap();
        assert_eq!(OsStore.get(platform).unwrap(), None);
        OsStore.set(platform, KEY).unwrap();
        assert_eq!(OsStore.get(platform).unwrap().as_deref(), Some(KEY));
        OsStore.delete(platform).unwrap();
        assert_eq!(OsStore.get(platform).unwrap(), None);
    }

    #[test]
    fn store_errors_never_print_stored_bytes() {
        let err = store_error(keyring::Error::BadEncoding(KEY.as_bytes().to_vec())).to_string();
        assert!(!err.contains("abcdefgh") && !err.contains("115"), "{err}");
    }
}
