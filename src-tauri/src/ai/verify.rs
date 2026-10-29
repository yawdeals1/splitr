//! Checks a saved API key with one cheap call to its own platform (#27): a model list, or
//! OpenRouter's key endpoint. The key is never sent anywhere else, and never tried against several
//! platforms to find where it belongs (NFR-05).

use std::time::Duration;

use serde::Serialize;

use super::compatible;
use super::http::Api;
use crate::error::{Result, SplitError};

/// A key check needs no thinking time.
const TIMEOUT: Duration = Duration::from_secs(20);

/// What a key check found. Mirrored by `KeyCheck` in `src/types.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum KeyCheck {
    /// The platform accepted the key.
    Connected,
    /// The platform has no call that checks a key without running a request.
    Unchecked { reason: String },
}

/// How to check a key: the platform's name, the address to GET, the header that carries the key,
/// and whether the address is on this computer.
#[derive(Debug, PartialEq, Eq)]
struct Check {
    name: String,
    url: String,
    /// `x-api-key`, `x-goog-api-key` or `authorization` (a bearer token).
    header: &'static str,
    local: bool,
}

/// The check for `platform`, or `Err` with the platform's name if it can't be checked.
/// `custom_url` is the custom platform's address.
fn check_for(platform: &str, custom_url: &str) -> Result<std::result::Result<Check, String>> {
    let fixed = |name: &str, url: &str, header| Ok(Ok(Check { name: name.into(), url: url.into(), header, local: false }));
    match platform {
        "anthropic" => fixed("Claude", "https://api.anthropic.com/v1/models?limit=1", "x-api-key"),
        "openai" => fixed("OpenAI", "https://api.openai.com/v1/models", "authorization"),
        "gemini" => fixed("Gemini", "https://generativelanguage.googleapis.com/v1beta/models?pageSize=1", "x-goog-api-key"),
        // OpenCode Go lists its models without a key, so a list call proves nothing.
        "opencode-go" => Ok(Err("OpenCode Go".into())),
        id if compatible::is_compatible(id) => Ok(compatible::check_target(id, custom_url)?
            .map(|t| Check { name: t.name, url: t.url, header: "authorization", local: t.local })),
        _ => Err(SplitError::ApiKey(format!("Splitr doesn't support the AI platform \"{platform}\""))),
    }
}

/// Checks `key` for `platform` with one call to that platform only.
pub async fn verify(platform: &str, custom_url: &str, key: &str) -> Result<KeyCheck> {
    match check_for(platform, custom_url)? {
        Ok(check) => run(&check, key, TIMEOUT).await,
        Err(name) => Ok(KeyCheck::Unchecked {
            reason: format!("{name} can't check a key without running a request, so it is checked on your first Smart split"),
        }),
    }
}

async fn run(check: &Check, key: &str, timeout: Duration) -> Result<KeyCheck> {
    let api = Api::build(check.name.clone(), timeout, check.local)?;
    let bearer = format!("Bearer {key}");
    let value = if check.header == "authorization" { bearer.as_str() } else { key };
    let mut headers = vec![(check.header, value)];
    if check.header == "x-api-key" {
        headers.push(("anthropic-version", "2023-06-01"));
    }
    api.get(&check.url, &headers).await?;
    Ok(KeyCheck::Connected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::http::test_server::serve;

    const KEY: &str = "sk-test-key-WXYZ";

    #[test]
    fn each_platform_is_checked_at_its_own_address() {
        let check = |p| check_for(p, "").unwrap().ok().unwrap();
        assert_eq!(check("anthropic").url, "https://api.anthropic.com/v1/models?limit=1");
        assert_eq!(check("anthropic").header, "x-api-key");
        assert_eq!(check("openai").url, "https://api.openai.com/v1/models");
        assert_eq!(check("gemini").header, "x-goog-api-key");
        // OpenRouter's model list is public, so its key endpoint is used.
        assert_eq!(check("openrouter").url, "https://openrouter.ai/api/v1/key");
        assert_eq!(check("groq").url, "https://api.groq.com/openai/v1/models");
        assert_eq!(check("deepseek").url, "https://api.deepseek.com/models");
        assert_eq!(check_for("opencode-go", "").unwrap(), Err("OpenCode Go".into()));
        // NVIDIA's model list answers without a key, so a list call would pass any key.
        assert_eq!(check_for("nvidia", "").unwrap(), Err("NVIDIA".into()));
        let custom = check_for("custom", "http://localhost:11434/v1").unwrap().ok().unwrap();
        assert_eq!((custom.url.as_str(), custom.local), ("http://localhost:11434/v1/models", true));
        assert!(check_for("custom", "http://example.com/v1").is_err());
        assert!(check_for("../evil", "").is_err());
    }

    fn local(url: &str, header: &'static str) -> Check {
        Check { name: "Example".into(), url: url.into(), header, local: true }
    }

    #[test]
    fn a_working_key_is_connected() {
        let (url, got) = serve("/v1/models", "200 OK", "", r#"{"data":[]}"#);
        let result = tauri::async_runtime::block_on(run(&local(&url, "authorization"), KEY, Duration::from_secs(5)));
        assert_eq!(result.unwrap(), KeyCheck::Connected);
        let raw = got.recv().unwrap();
        assert!(raw.starts_with("GET /v1/models "), "{raw}");
        assert!(raw.to_lowercase().contains(&format!("authorization: bearer {}", KEY.to_lowercase())));
    }

    #[test]
    fn anthropic_style_checks_send_the_version() {
        let (url, got) = serve("/v1/models", "200 OK", "", "{}");
        tauri::async_runtime::block_on(run(&local(&url, "x-api-key"), KEY, Duration::from_secs(5))).unwrap();
        let raw = got.recv().unwrap().to_lowercase();
        assert!(raw.contains(&format!("x-api-key: {}", KEY.to_lowercase())) && raw.contains("anthropic-version: 2023-06-01"));
        assert!(!raw.contains("authorization:"));
    }

    #[test]
    fn a_rejected_key_says_so_without_showing_it() {
        let (url, _got) = serve("/v1/models", "401 Unauthorized", "", r#"{"error":{"message":"Incorrect API key"}}"#);
        let err = tauri::async_runtime::block_on(run(&local(&url, "authorization"), KEY, Duration::from_secs(5)))
            .unwrap_err()
            .to_string();
        assert!(err.contains("Example didn't accept your API key"), "{err}");
        assert!(!err.contains(KEY));
    }

    #[test]
    fn opencode_go_keys_are_left_unchecked() {
        let result = tauri::async_runtime::block_on(verify("opencode-go", "", KEY)).unwrap();
        assert!(matches!(result, KeyCheck::Unchecked { .. }));
    }
}
