//! The OpenAI API (Responses API) over HTTPS (#28, SRS 5.4).
//!
//! The key is read from the credential store for each call and sent only to OpenAI, as a bearer
//! token (NFR-05). It is never logged and never part of an error message.

use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use super::http::{Api, TIMEOUT};
use super::{AiRequest, Provider, system_prompt, user_message};
use crate::error::{Result, SplitError};

const API_URL: &str = "https://api.openai.com/v1/responses";
/// A reply is a short JSON list of ranges; the rest is room for reasoning, which counts too.
const MAX_OUTPUT_TOKENS: u32 = 16_000;

/// A client for one call. Deliberately not `Debug`, so the key can't end up in a log.
pub struct OpenAi {
    api: Api,
    /// The platform's name for messages: "OpenAI", or another platform on this API (#32).
    name: String,
    url: String,
    key: String,
    model: String,
}

impl OpenAi {
    pub fn new(key: String, model: String) -> Result<Self> {
        Self::build("OpenAI", API_URL.into(), key, model, TIMEOUT)
    }

    /// A client for any endpoint that speaks the Responses API, such as OpenCode Go (#32).
    pub fn build(name: &str, url: String, key: String, model: String, timeout: Duration) -> Result<Self> {
        Ok(Self { api: Api::new(name, timeout)?, name: name.into(), url, key, model })
    }

    /// The HTTPS client, to add headers or usage-limit messages for another platform (#32).
    pub fn api_mut(&mut self) -> &mut Api {
        &mut self.api
    }

    fn body(&self, request: &AiRequest) -> Value {
        // The prompt asks for a bare JSON array, which strict structured output can't describe
        // (its root must be an object), so the reply is parsed and validated by the caller (#16).
        json!({
            "model": self.model,
            "instructions": system_prompt(&request.instruction),
            "input": user_message(request),
            "max_output_tokens": MAX_OUTPUT_TOKENS,
            // Nothing is kept on OpenAI's side after the call.
            "store": false,
        })
    }
}

impl Provider for OpenAi {
    async fn complete(&self, request: &AiRequest) -> Result<String> {
        if request.pages.is_empty() {
            return Err(SplitError::Ai(format!("There are no pages to send to {}", self.name)));
        }
        if request.instruction.trim().is_empty() {
            return Err(SplitError::Ai("Describe how to split the file before using Smart mode".into()));
        }
        let auth = format!("Bearer {}", self.key);
        let reply = self.api.post(&self.url, &[("authorization", &auth)], &self.body(request), &self.model).await?;
        reply_text(&self.name, &reply)
    }
}

#[derive(Deserialize)]
struct Reply {
    status: Option<String>,
    incomplete_details: Option<Incomplete>,
    #[serde(default)]
    output: Vec<Item>,
}

#[derive(Deserialize)]
struct Incomplete {
    reason: Option<String>,
}

#[derive(Deserialize)]
struct Item {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    content: Vec<Part>,
}

#[derive(Deserialize)]
struct Part {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

/// The text of a successful reply from `name`. Reasoning items are skipped.
fn reply_text(name: &str, body: &str) -> Result<String> {
    let reply: Reply = serde_json::from_str(body)
        .map_err(|_| SplitError::Ai(format!("{name} sent an answer Splitr couldn't read. Try again")))?;
    let declined = || SplitError::Ai(format!("{name} declined to split this file. Try rewording the instruction"));
    let parts = || reply.output.iter().filter(|i| i.kind == "message").flat_map(|i| &i.content);
    if parts().any(|p| p.kind == "refusal") {
        return Err(declined());
    }
    match (reply.status.as_deref(), reply.incomplete_details.and_then(|d| d.reason).as_deref()) {
        (Some("incomplete"), Some("content_filter")) => return Err(declined()),
        (Some("incomplete"), _) => {
            return Err(SplitError::Ai(format!(
                "{name}'s answer was cut off because it was too long. Try splitting a smaller part of the file"
            )));
        }
        (Some("failed" | "cancelled"), _) => {
            return Err(SplitError::Ai(format!("{name} couldn't finish the answer. Try again")));
        }
        _ => {}
    }
    let text: String = parts().filter(|p| p.kind == "output_text").map(|p| p.text.as_str()).collect();
    if text.trim().is_empty() {
        return Err(SplitError::Ai(format!("{name} sent an empty answer. Try again")));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::PageExcerpt;
    use crate::ai::http::test_server::{body, serve};

    const KEY: &str = "sk-proj-test-key-WXYZ";
    const MODEL: &str = "gpt-6.1-sol";

    fn request() -> AiRequest {
        AiRequest {
            instruction: "Split by chapter".into(),
            pages: vec![
                PageExcerpt { page: 1, head: "Preface".into(), tail: "".into() },
                PageExcerpt { page: 2, head: "Chapter 1".into(), tail: "end".into() },
            ],
        }
    }

    fn run(url: String) -> Result<String> {
        let openai = OpenAi::build("OpenAI", url, KEY.into(), MODEL.into(), Duration::from_secs(5)).unwrap();
        tauri::async_runtime::block_on(openai.complete(&request()))
    }

    const OK: &str = r#"{"status":"completed","output":[
        {"type":"reasoning","summary":[]},
        {"type":"message","role":"assistant","content":[
            {"type":"output_text","text":"[{\"start\":1,","annotations":[]},
            {"type":"output_text","text":"\"end\":2,\"title\":\"Book\"}]","annotations":[]}]}]}"#;

    #[test]
    fn sends_the_key_and_prompt_and_returns_the_text() {
        let (url, got) = serve("/v1/responses", "200 OK", "", OK);
        assert_eq!(run(url).unwrap(), r#"[{"start":1,"end":2,"title":"Book"}]"#);

        let raw = got.recv().unwrap();
        let lower = raw.to_lowercase();
        assert!(lower.starts_with("post /v1/responses "), "{raw}");
        assert!(lower.contains(&format!("authorization: bearer {}", KEY.to_lowercase())));
        let sent = body(&raw);
        assert_eq!(sent["model"], MODEL);
        assert_eq!(sent["store"], false);
        assert!(sent["instructions"].as_str().unwrap().contains("Task: Split by chapter"));
        let pages: Value = serde_json::from_str(sent["input"].as_str().unwrap()).unwrap();
        assert_eq!(pages["pages"][1]["head"], "Chapter 1");
        // Reasoning models reject sampling settings.
        for field in ["temperature", "top_p"] {
            assert!(sent.get(field).is_none(), "{field} was sent");
        }
    }

    #[test]
    fn errors_name_openai_and_never_show_the_key() {
        let body = r#"{"error":{"message":"You exceeded your current quota","type":"insufficient_quota","code":"insufficient_quota"}}"#;
        let (url, _got) = serve("/v1/responses", "429 Too Many Requests", "", body);
        let err = run(url).unwrap_err().to_string();
        assert!(err.contains("used up your OpenAI quota"), "{err}");
        assert!(!err.contains(KEY));
        let (url, _got) = serve("/v1/responses", "401 Unauthorized", "", "{}");
        assert!(run(url).unwrap_err().to_string().contains("OpenAI didn't accept your API key"));
    }

    #[test]
    fn empty_requests_are_not_sent() {
        let openai = OpenAi::build("OpenAI", "http://127.0.0.1:9/v1/responses".into(), KEY.into(), MODEL.into(), Duration::from_secs(5)).unwrap();
        let mut empty = request();
        empty.pages.clear();
        assert!(tauri::async_runtime::block_on(openai.complete(&empty)).unwrap_err().to_string().contains("no pages"));
    }

    #[test]
    fn reply_text_handles_refusals_cut_offs_and_bad_bodies() {
        let refusal = r#"{"status":"completed","output":[{"type":"message","content":[{"type":"refusal","refusal":"no"}]}]}"#;
        assert!(reply_text("OpenAI", refusal).unwrap_err().to_string().contains("declined"));
        let filtered = r#"{"status":"incomplete","incomplete_details":{"reason":"content_filter"},"output":[]}"#;
        assert!(reply_text("OpenAI", filtered).unwrap_err().to_string().contains("declined"));
        let cut = r#"{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"},"output":[]}"#;
        assert!(reply_text("OpenAI", cut).unwrap_err().to_string().contains("cut off"));
        let failed = r#"{"status":"failed","output":[]}"#;
        assert!(reply_text("OpenAI", failed).unwrap_err().to_string().contains("couldn't finish"));
        let empty = r#"{"status":"completed","output":[{"type":"message","content":[{"type":"output_text","text":" "}]}]}"#;
        assert!(reply_text("OpenAI", empty).unwrap_err().to_string().contains("empty"));
        assert!(reply_text("OpenAI", "<html>").unwrap_err().to_string().contains("couldn't read"));
    }

    /// The manual check for #28. Needs an OpenAI key saved in the app and network access; sends
    /// four made-up pages. Run with `cargo test real_openai -- --ignored --nocapture`.
    #[test]
    #[ignore = "calls the real OpenAI API with the key saved in the OS credential store"]
    fn real_openai_call() {
        use crate::keystore::{self, OsStore};
        let key = keystore::key_for(&OsStore, "openai").unwrap();
        let openai = OpenAi::new(key, crate::ai::default_model("openai").unwrap().into()).unwrap();
        let page = |page, head: &str| PageExcerpt { page, head: head.into(), tail: String::new() };
        let request = AiRequest {
            instruction: "Split this course material so each chapter is one part. Use chapter titles as part titles.".into(),
            pages: vec![
                page(1, "Preface"),
                page(2, "Chapter 1 Computer Abstractions and Technology"),
                page(3, "1.2 Eight Great Ideas in Computer Architecture"),
                page(4, "Chapter 2 Instructions: Language of the Computer"),
            ],
        };
        let reply = tauri::async_runtime::block_on(openai.complete(&request)).unwrap();
        println!("{reply}");
        assert!(reply.trim_start().starts_with('['), "{reply}");
    }
}
