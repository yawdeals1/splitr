//! Platforms that speak the OpenAI chat completions format: OpenRouter, NVIDIA, Groq, xAI,
//! DeepSeek, Mistral and Together AI, plus a custom one at an address the user enters (#30).
//!
//! The platforms are data (`compatible.json`), so adding one needs no code. The key is read from
//! the credential store for each call and sent only to the platform it was saved for, as a bearer
//! token (NFR-05).

use std::sync::LazyLock;
use std::time::Duration;

use reqwest::Url;
use serde::Deserialize;
use serde_json::{Value, json};

use super::http::{Api, TIMEOUT};
use super::{AiRequest, Provider, system_prompt, user_message};
use crate::error::{Result, SplitError};

/// The id of the platform at an address the user enters.
pub const CUSTOM: &str = "custom";
/// The longest model list kept. OpenRouter alone lists hundreds.
const MAX_MODELS: usize = 2000;

/// One built-in platform, from `compatible.json`. The file also holds the company and key hint the
/// interface shows; `src/ai.test.ts` checks that the interface matches it.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Platform {
    pub id: String,
    pub name: String,
    /// The address `/chat/completions` and `/models` are added to.
    pub base_url: String,
    pub default_model: Option<String>,
}

/// The built-in platforms. The `platforms_file_is_valid` test checks the file, so it can't be
/// empty in a build that passed CI.
static PLATFORMS: LazyLock<Vec<Platform>> =
    LazyLock::new(|| serde_json::from_str(include_str!("compatible.json")).unwrap_or_default());

pub fn platforms() -> &'static [Platform] {
    &PLATFORMS
}

fn platform(id: &str) -> Option<&'static Platform> {
    PLATFORMS.iter().find(|p| p.id == id)
}

/// True for the built-in platforms and the custom one.
pub fn is_compatible(id: &str) -> bool {
    id == CUSTOM || platform(id).is_some()
}

/// The default model of a built-in platform, if the data file names one. The others offer their
/// model list instead, because it changes often.
pub fn default_model(id: &str) -> Option<&'static str> {
    platform(id)?.default_model.as_deref()
}

/// Where requests for `id` go, and its name for messages. `custom_url` is used for the custom
/// platform only.
struct Endpoint {
    name: String,
    base: String,
    /// True for a plain-HTTP address on this computer, such as a local model (#25).
    local: bool,
}

fn endpoint(id: &str, custom_url: &str) -> Result<Endpoint> {
    if id == CUSTOM {
        let (base, local) = check_url(custom_url)?;
        return Ok(Endpoint { name: "the custom platform".into(), base, local });
    }
    let platform = platform(id).ok_or_else(|| SplitError::Ai(format!("Splitr doesn't know the AI platform \"{id}\"")))?;
    Ok(Endpoint { name: platform.name.clone(), base: platform.base_url.clone(), local: false })
}

/// The base address of a custom platform, without a trailing "/", and whether it is local. Remote
/// addresses must use HTTPS; plain HTTP is allowed only on this computer.
pub fn check_url(url: &str) -> Result<(String, bool)> {
    let url = url.trim();
    if url.is_empty() {
        return Err(SplitError::Ai("Enter the custom platform's address under AI in Settings".into()));
    }
    let parsed = Url::parse(url).map_err(|_| {
        SplitError::Ai(format!("\"{url}\" isn't a web address. Enter one such as https://example.com/v1"))
    })?;
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.query().is_some() || parsed.fragment().is_some()
    {
        return Err(SplitError::Ai(
            "Enter the custom platform's address without a user name, password or \"?\" part. The key goes in the key field"
                .into(),
        ));
    }
    let local = matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    match parsed.scheme() {
        "https" => Ok((url.trim_end_matches('/').to_string(), false)),
        "http" if local => Ok((url.trim_end_matches('/').to_string(), true)),
        "http" => Err(SplitError::Ai(
            "Use an https:// address for the custom platform. Plain http:// is only allowed for this computer (localhost)"
                .into(),
        )),
        _ => Err(SplitError::Ai(format!("\"{url}\" isn't a web address. Enter one such as https://example.com/v1"))),
    }
}

/// A client for one platform. Deliberately not `Debug`, so the key can't end up in a log.
pub struct ChatCompletions {
    api: Api,
    name: String,
    base: String,
    key: String,
    model: String,
}

impl ChatCompletions {
    /// A client for platform `id`; `custom_url` is the address of the custom platform.
    pub fn new(id: &str, custom_url: &str, key: String, model: String) -> Result<Self> {
        let endpoint = endpoint(id, custom_url)?;
        Self::build(endpoint.name, endpoint.base, endpoint.local, key, model, TIMEOUT)
    }

    /// A client for any chat completions address, such as OpenCode Go (#32).
    pub fn build(name: String, base: String, local: bool, key: String, model: String, timeout: Duration) -> Result<Self> {
        Ok(Self { api: Api::build(name.clone(), timeout, local)?, name, base, key, model })
    }

    /// The HTTPS client, to add headers or usage-limit messages for another platform (#32).
    pub fn api_mut(&mut self) -> &mut Api {
        &mut self.api
    }

    fn body(&self, request: &AiRequest) -> Value {
        // No token limit or sampling settings: platforms disagree on their names and some
        // reasoning models reject them.
        json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": system_prompt(&request.instruction) },
                { "role": "user", "content": user_message(request) },
            ],
        })
    }

    /// The models the platform offers, sorted (#30). Sent to that platform only.
    pub async fn models(&self) -> Result<Vec<String>> {
        let auth = format!("Bearer {}", self.key);
        let reply = self.api.get(&format!("{}/models", self.base), &[("authorization", &auth)]).await?;
        model_ids(&reply).ok_or_else(|| SplitError::Ai(format!("{} sent a model list Splitr couldn't read", self.name)))
    }
}

impl Provider for ChatCompletions {
    async fn complete(&self, request: &AiRequest) -> Result<String> {
        let name = &self.name;
        if request.pages.is_empty() {
            return Err(SplitError::Ai(format!("There are no pages to send to {name}")));
        }
        if request.instruction.trim().is_empty() {
            return Err(SplitError::Ai("Describe how to split the file before using Smart mode".into()));
        }
        if self.model.trim().is_empty() {
            return Err(SplitError::Ai(format!("Choose a model for {name} under AI in Settings")));
        }
        let auth = format!("Bearer {}", self.key);
        let url = format!("{}/chat/completions", self.base);
        let reply = self.api.post(&url, &[("authorization", &auth)], &self.body(request), &self.model).await?;
        reply_text(name, &reply)
    }
}

#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    message: Option<Message>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Message {
    content: Option<String>,
    refusal: Option<String>,
}

/// The text of a successful reply from `name`.
fn reply_text(name: &str, body: &str) -> Result<String> {
    let reply: Reply = serde_json::from_str(body)
        .map_err(|_| SplitError::Ai(format!("{name} sent an answer Splitr couldn't read. Try again")))?;
    let choice = reply
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| SplitError::Ai(format!("{name} sent an empty answer. Try again")))?;
    let message = choice.message.unwrap_or(Message { content: None, refusal: None });
    let declined = || SplitError::Ai(format!("{name} declined to split this file. Try rewording the instruction"));
    if message.refusal.is_some_and(|r| !r.trim().is_empty()) {
        return Err(declined());
    }
    match choice.finish_reason.as_deref() {
        Some("length") => {
            return Err(SplitError::Ai(format!(
                "{name}'s answer was cut off because it was too long. Try splitting a smaller part of the file"
            )));
        }
        Some("content_filter") => return Err(declined()),
        _ => {}
    }
    let text = message.content.unwrap_or_default();
    if text.trim().is_empty() {
        return Err(SplitError::Ai(format!("{name} sent an empty answer. Try again")));
    }
    Ok(text)
}

/// The model ids in a `/models` reply: `{"data": [{"id": ...}]}`, or a bare list.
fn model_ids(body: &str) -> Option<Vec<String>> {
    let value: Value = serde_json::from_str(body).ok()?;
    let list = value.get("data").unwrap_or(&value).as_array()?;
    let mut ids: Vec<String> = list
        .iter()
        .filter_map(|m| m.get("id").and_then(Value::as_str))
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .map(String::from)
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids.truncate(MAX_MODELS);
    Some(ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::PageExcerpt;
    use crate::ai::http::test_server::{body, serve};

    const KEY: &str = "sk-or-v1-test-key-WXYZ";

    fn request() -> AiRequest {
        AiRequest {
            instruction: "Split by chapter".into(),
            pages: vec![PageExcerpt { page: 1, head: "Chapter 1".into(), tail: "end".into() }],
        }
    }

    fn client(base: &str, model: &str) -> ChatCompletions {
        ChatCompletions::build("OpenRouter".into(), base.into(), false, KEY.into(), model.into(), Duration::from_secs(5)).unwrap()
    }

    const OK: &str = r#"{"choices":[{"index":0,"message":{"role":"assistant","content":"[{\"start\":1,\"end\":1,\"title\":\"One\"}]","refusal":null},"finish_reason":"stop"}]}"#;

    #[test]
    fn platforms_file_is_valid() {
        let platforms = platforms();
        assert!(platforms.len() >= 7, "compatible.json didn't load");
        for p in platforms {
            assert!(crate::keystore::is_platform(&p.id), "{} has no credential store entry", p.id);
            assert!(p.base_url.starts_with("https://") && !p.base_url.ends_with('/'), "{}", p.base_url);
            assert!(!p.name.trim().is_empty());
            assert!(p.default_model.as_deref().is_none_or(|m| !m.trim().is_empty()));
        }
        assert_eq!(default_model("deepseek"), Some("deepseek-flash"));
        assert!(is_compatible("openrouter") && is_compatible(CUSTOM) && !is_compatible("openai"));
    }

    #[test]
    fn a_built_in_platform_gets_the_prompt_and_key() {
        let (url, got) = serve("/api/v1/chat/completions", "200 OK", "", OK);
        let base = url.trim_end_matches("/chat/completions");
        let reply = tauri::async_runtime::block_on(client(base, "some/model").complete(&request())).unwrap();
        assert_eq!(reply, r#"[{"start":1,"end":1,"title":"One"}]"#);

        let raw = got.recv().unwrap();
        assert!(raw.to_lowercase().starts_with("post /api/v1/chat/completions "), "{raw}");
        assert!(raw.to_lowercase().contains(&format!("authorization: bearer {}", KEY.to_lowercase())));
        let sent = body(&raw);
        assert_eq!(sent["model"], "some/model");
        assert_eq!(sent["messages"][0]["role"], "system");
        assert!(sent["messages"][0]["content"].as_str().unwrap().contains("Task: Split by chapter"));
        assert_eq!(sent["messages"][1]["role"], "user");
        assert!(sent.get("max_tokens").is_none() && sent.get("temperature").is_none());
    }

    #[test]
    fn a_custom_platform_uses_the_address_entered() {
        let (url, got) = serve("/v1/chat/completions", "200 OK", "", OK);
        let base = format!("{}/", url.trim_end_matches("/chat/completions"));
        // The test server is plain HTTP on 127.0.0.1, which a custom platform may use.
        let custom = ChatCompletions::new(CUSTOM, &base, "local-key-1234".into(), "llama".into()).unwrap();
        tauri::async_runtime::block_on(custom.complete(&request())).unwrap();
        assert!(got.recv().unwrap().starts_with("POST /v1/chat/completions "));
    }

    #[test]
    fn custom_addresses_must_be_https_unless_local() {
        assert_eq!(check_url(" https://llm.example.com/v1/ ").unwrap(), ("https://llm.example.com/v1".into(), false));
        assert_eq!(check_url("http://localhost:11434/v1").unwrap(), ("http://localhost:11434/v1".into(), true));
        assert!(check_url("http://127.0.0.1:8080/v1").unwrap().1);
        assert!(check_url("http://[::1]:8080/v1").unwrap().1);
        for bad in ["", "llm.example.com", "http://llm.example.com/v1", "ftp://x/v1", "https://user:pw@x.com/v1",
            "https://x.com/v1?key=abc", "http://localhost.evil.com/v1"]
        {
            assert!(check_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn errors_name_the_platform_and_never_show_the_key() {
        let (url, _got) = serve("/v1/chat/completions", "401 Unauthorized", "", r#"{"error":{"message":"No auth"}}"#);
        let base = url.trim_end_matches("/chat/completions");
        let err = tauri::async_runtime::block_on(client(base, "m").complete(&request())).unwrap_err().to_string();
        assert!(err.contains("OpenRouter didn't accept your API key"), "{err}");
        assert!(!err.contains(KEY));
        let no_model = tauri::async_runtime::block_on(client(base, " ").complete(&request())).unwrap_err();
        assert!(no_model.to_string().contains("Choose a model for OpenRouter"), "{no_model}");
    }

    #[test]
    fn lists_the_platforms_models() {
        let list = r#"{"object":"list","data":[{"id":"b/model"},{"id":"a/model"},{"id":"a/model"},{"id":" "}]}"#;
        let (url, got) = serve("/v1/models", "200 OK", "", list);
        let base = url.trim_end_matches("/models");
        let models = tauri::async_runtime::block_on(client(base, "m").models()).unwrap();
        assert_eq!(models, ["a/model", "b/model"]);
        let raw = got.recv().unwrap();
        assert!(raw.starts_with("GET /v1/models "), "{raw}");
        assert!(raw.to_lowercase().contains("authorization: bearer"));
        assert_eq!(model_ids(r#"[{"id":"x"}]"#), Some(vec!["x".to_string()]));
        assert_eq!(model_ids("{}"), None);
    }

    #[test]
    fn reply_text_handles_refusals_cut_offs_and_bad_bodies() {
        let refusal = r#"{"choices":[{"message":{"content":null,"refusal":"I can't"},"finish_reason":"stop"}]}"#;
        assert!(reply_text("X", refusal).unwrap_err().to_string().contains("X declined"));
        let filtered = r#"{"choices":[{"message":{"content":""},"finish_reason":"content_filter"}]}"#;
        assert!(reply_text("X", filtered).unwrap_err().to_string().contains("declined"));
        let cut = r#"{"choices":[{"message":{"content":"[{"},"finish_reason":"length"}]}"#;
        assert!(reply_text("X", cut).unwrap_err().to_string().contains("cut off"));
        assert!(reply_text("X", r#"{"choices":[]}"#).unwrap_err().to_string().contains("empty"));
        assert!(reply_text("X", "<html>").unwrap_err().to_string().contains("couldn't read"));
    }
}
