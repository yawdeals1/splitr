//! OpenCode Go, a subscription to open models used through one API key (#32).
//!
//! Each model has a native API style; `opencode_go.json` lists the models that use the Responses
//! API, Anthropic-style messages or Google's API, and every other model uses chat completions.
//! OpenCode translates between styles, so a model missing from the table still works. The table is
//! data, because OpenCode's model list changes. The key goes only to OpenCode (NFR-05), in the
//! header each style uses.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::json;

use super::compatible::ChatCompletions;
use super::gemini::Gemini;
use super::http::{Api, TIMEOUT};
use super::openai::OpenAi;
use super::{AiRequest, Provider, system_prompt, user_message};
use crate::error::{Result, SplitError};

const NAME: &str = "OpenCode Go";
/// The Anthropic API version OpenCode's messages route follows.
const ANTHROPIC_VERSION: &str = "2023-06-01";
/// A reply is a short JSON list of ranges, so this leaves plenty of room after thinking.
const MAX_TOKENS: u32 = 16_000;

/// How a model is called.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// OpenAI chat completions: most models.
    Chat,
    /// The OpenAI Responses API.
    Responses,
    /// Anthropic-style messages.
    Messages,
    /// Google's generateContent.
    Google,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Table {
    base_url: String,
    default_model: Option<String>,
    routes: Routes,
}

#[derive(Debug, Default, Deserialize)]
struct Routes {
    #[serde(default)]
    responses: Vec<String>,
    #[serde(default)]
    messages: Vec<String>,
    #[serde(default)]
    google: Vec<String>,
}

/// The routes table. The `table_is_valid` test checks the file.
static TABLE: LazyLock<Table> = LazyLock::new(|| serde_json::from_str(include_str!("opencode_go.json")).unwrap_or_default());

/// The model Smart mode uses with OpenCode Go until the user picks another.
pub fn default_model() -> Option<&'static str> {
    TABLE.default_model.as_deref()
}

/// The API style for `model`.
pub fn route(model: &str) -> Route {
    let routes = &TABLE.routes;
    let listed = |list: &[String]| list.iter().any(|m| m.eq_ignore_ascii_case(model.trim()));
    if listed(&routes.responses) {
        Route::Responses
    } else if listed(&routes.messages) {
        Route::Messages
    } else if listed(&routes.google) {
        Route::Google
    } else {
        Route::Chat
    }
}

/// A new id for one request, sent as `x-opencode-session` so OpenCode can route and cache well.
fn session_id() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
    format!("splitr-{nanos:x}-{:x}", NEXT.fetch_add(1, Ordering::Relaxed))
}

/// Sets up a client for OpenCode: its usage-limit messages and the headers it asks clients to send.
fn configure(api: &mut Api) {
    api.with_usage_limits();
    api.add_header("x-opencode-session", session_id());
    api.add_header("user-agent", format!("splitr/{}", env!("CARGO_PKG_VERSION")));
}

/// Sends `request` to `model` on OpenCode Go, in the model's API style.
pub async fn complete(key: String, model: String, request: &AiRequest) -> Result<String> {
    complete_at(&TABLE.base_url, key, model, request, TIMEOUT).await
}

async fn complete_at(base: &str, key: String, model: String, request: &AiRequest, timeout: Duration) -> Result<String> {
    if model.trim().is_empty() {
        return Err(SplitError::Ai("Choose an OpenCode Go model under AI in Settings".into()));
    }
    match route(&model) {
        Route::Chat => {
            let mut client = ChatCompletions::build(NAME.into(), format!("{base}/openai/v1"), false, key, model, timeout)?;
            configure(client.api_mut());
            client.complete(request).await
        }
        Route::Responses => {
            let mut client = OpenAi::build(NAME, format!("{base}/openai/v1/responses"), key, model, timeout)?;
            configure(client.api_mut());
            client.complete(request).await
        }
        Route::Google => {
            let mut client = Gemini::build(NAME, format!("{base}/google/v1beta/models"), key, model, timeout)?;
            configure(client.api_mut());
            client.complete(request).await
        }
        Route::Messages => messages(base, key, model, request, timeout).await,
    }
}

/// An Anthropic-style messages call, with the key in `x-api-key` as Anthropic's clients send it.
async fn messages(base: &str, key: String, model: String, request: &AiRequest, timeout: Duration) -> Result<String> {
    if request.pages.is_empty() {
        return Err(SplitError::Ai(format!("There are no pages to send to {NAME}")));
    }
    if request.instruction.trim().is_empty() {
        return Err(SplitError::Ai("Describe how to split the file before using Smart mode".into()));
    }
    let mut api = Api::new(NAME, timeout)?;
    configure(&mut api);
    let body = json!({
        "model": model,
        "max_tokens": MAX_TOKENS,
        "system": system_prompt(&request.instruction),
        "messages": [{ "role": "user", "content": user_message(request) }],
    });
    let headers = [("x-api-key", key.as_str()), ("anthropic-version", ANTHROPIC_VERSION)];
    let reply = api.post(&format!("{base}/anthropic/v1/messages"), &headers, &body, &model).await?;
    super::claude::reply_text_for(NAME, &reply)
}

/// The models OpenCode Go offers now, from its `/v1/models` list (#32). Sent to OpenCode only.
pub async fn models(key: String) -> Result<Vec<String>> {
    models_at(&TABLE.base_url, key, TIMEOUT).await
}

async fn models_at(base: &str, key: String, timeout: Duration) -> Result<Vec<String>> {
    let mut client = ChatCompletions::build(NAME.into(), format!("{base}/v1"), false, key, String::new(), timeout)?;
    configure(client.api_mut());
    client.models().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::PageExcerpt;
    use crate::ai::http::test_server::{body, serve};

    const KEY: &str = "ocgo-test-key-WXYZ";

    fn request() -> AiRequest {
        AiRequest {
            instruction: "Split by chapter".into(),
            pages: vec![PageExcerpt { page: 1, head: "Chapter 1".into(), tail: "end".into() }],
        }
    }

    /// Runs `model` against a local server at `path` and returns the raw request it got.
    fn call(path: &str, reply: &str, model: &str) -> (Result<String>, String) {
        let (url, got) = serve(path, "200 OK", "", reply);
        let base = &url[..url.find(path).unwrap()];
        let result = tauri::async_runtime::block_on(complete_at(base, KEY.into(), model.into(), &request(), Duration::from_secs(5)));
        (result, got.recv().unwrap())
    }

    const RANGES: &str = r#"[{"start":1,"end":1,"title":"One"}]"#;

    #[test]
    fn table_is_valid() {
        assert!(TABLE.base_url.starts_with("https://") && !TABLE.base_url.ends_with('/'));
        assert_eq!(default_model(), Some("deepseek-v4.1-flash"));
        assert_eq!(route("deepseek-v4.1-flash"), Route::Chat);
        assert_eq!(route(" GPT-6-Luna "), Route::Responses);
        assert_eq!(route("claude-haiku-5-5"), Route::Messages);
        assert_eq!(route("gemini-4-argon"), Route::Google);
        assert_eq!(route("a-model-added-next-month"), Route::Chat);
    }

    #[test]
    fn chat_models_use_chat_completions_with_a_bearer_key() {
        let reply = format!(r#"{{"choices":[{{"message":{{"content":{}}},"finish_reason":"stop"}}]}}"#, serde_json::to_string(RANGES).unwrap());
        let (result, raw) = call("/openai/v1/chat/completions", &reply, "kimi-k3");
        assert_eq!(result.unwrap(), RANGES);
        let lower = raw.to_lowercase();
        assert!(lower.contains(&format!("authorization: bearer {}", KEY.to_lowercase())));
        assert!(lower.contains("x-opencode-session: splitr-"));
        assert!(lower.contains("user-agent: splitr/"));
        assert_eq!(body(&raw)["model"], "kimi-k3");
    }

    #[test]
    fn responses_models_use_the_responses_api() {
        let reply = format!(
            r#"{{"status":"completed","output":[{{"type":"message","content":[{{"type":"output_text","text":{}}}]}}]}}"#,
            serde_json::to_string(RANGES).unwrap()
        );
        let (result, raw) = call("/openai/v1/responses", &reply, "gpt-6-luna");
        assert_eq!(result.unwrap(), RANGES);
        assert!(raw.to_lowercase().contains("authorization: bearer"));
        assert!(body(&raw)["instructions"].as_str().unwrap().contains("Task: Split by chapter"));
    }

    #[test]
    fn messages_models_use_anthropic_style_with_x_api_key() {
        let reply = format!(r#"{{"content":[{{"type":"text","text":{}}}],"stop_reason":"end_turn"}}"#, serde_json::to_string(RANGES).unwrap());
        let (result, raw) = call("/anthropic/v1/messages", &reply, "minimax-m3");
        assert_eq!(result.unwrap(), RANGES);
        let lower = raw.to_lowercase();
        assert!(lower.contains(&format!("x-api-key: {}", KEY.to_lowercase())));
        assert!(lower.contains("anthropic-version: 2023-06-01"));
        assert!(!lower.contains("authorization:"));
        assert_eq!(body(&raw)["max_tokens"], MAX_TOKENS);
    }

    #[test]
    fn google_models_use_generate_content() {
        let reply = format!(r#"{{"candidates":[{{"content":{{"parts":[{{"text":{}}}]}},"finishReason":"STOP"}}]}}"#, serde_json::to_string(RANGES).unwrap());
        let (result, raw) = call("/google/v1beta/models/gemini-4-argon:generateContent", &reply, "gemini-4-argon");
        assert_eq!(result.unwrap(), RANGES);
        assert!(raw.to_lowercase().contains("x-goog-api-key:"));
    }

    #[test]
    fn a_usage_limit_says_when_it_resets() {
        let (url, _got) = serve(
            "/openai/v1/chat/completions",
            "429 Too Many Requests",
            "retry-after: 7200\r\n",
            r#"{"error":{"message":"Usage limit reached for this model"}}"#,
        );
        let base = url.trim_end_matches("/openai/v1/chat/completions");
        let err = tauri::async_runtime::block_on(complete_at(base, KEY.into(), "kimi-k3".into(), &request(), Duration::from_secs(5)))
            .unwrap_err()
            .to_string();
        assert!(err.contains("reached your OpenCode Go usage limit") && err.contains("in about 2 hours"), "{err}");
        assert!(!err.contains(KEY));
    }

    #[test]
    fn lists_models_from_opencode() {
        let (url, got) = serve("/v1/models", "200 OK", "", r#"{"object":"list","data":[{"id":"kimi-k3"},{"id":"glm-5.3"}]}"#);
        let base = url.trim_end_matches("/v1/models");
        let models = tauri::async_runtime::block_on(models_at(base, KEY.into(), Duration::from_secs(5))).unwrap();
        assert_eq!(models, ["glm-5.3", "kimi-k3"]);
        assert!(got.recv().unwrap().starts_with("GET /v1/models "));
    }

    #[test]
    fn a_blank_model_is_not_sent() {
        let err = tauri::async_runtime::block_on(complete_at("http://127.0.0.1:9", KEY.into(), " ".into(), &request(), Duration::from_secs(5)));
        assert!(err.unwrap_err().to_string().contains("Choose an OpenCode Go model"));
    }

    /// The manual check for #32. Needs an OpenCode Go key saved in the app and network access.
    /// Run with `cargo test real_opencode_go -- --ignored --nocapture`.
    #[test]
    #[ignore = "calls the real OpenCode Go API with the key saved in the OS credential store"]
    fn real_opencode_go_call() {
        use crate::keystore::{self, OsStore};
        let key = keystore::key_for(&OsStore, "opencode-go").unwrap();
        let reply = tauri::async_runtime::block_on(complete(key, default_model().unwrap().into(), &request())).unwrap();
        println!("{reply}");
        assert!(reply.trim_start().starts_with('['), "{reply}");
    }
}
