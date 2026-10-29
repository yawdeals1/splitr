//! The Google Gemini API (generateContent) over HTTPS, with an AI Studio API key (#29, SRS 5.4).
//!
//! The key is read from the credential store for each call and sent only to Google, in the
//! `x-goog-api-key` header rather than the URL, so it can't appear in an error (NFR-05).

use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};

use super::http::{Api, TIMEOUT};
use super::{AiRequest, Provider, system_prompt, user_message};
use crate::error::{Result, SplitError};

const API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta/models";
/// A reply is a short JSON list of ranges; the rest is room for thinking, which counts too.
const MAX_OUTPUT_TOKENS: u32 = 16_000;

/// A client for one call. Deliberately not `Debug`, so the key can't end up in a log.
pub struct Gemini {
    api: Api,
    base: String,
    key: String,
    model: String,
}

impl Gemini {
    pub fn new(key: String, model: String) -> Result<Self> {
        Self::build(API_BASE.into(), key, model, TIMEOUT)
    }

    fn build(base: String, key: String, model: String, timeout: Duration) -> Result<Self> {
        Ok(Self { api: Api::new("Gemini", timeout)?, base, key, model: model_id(&model)? })
    }

    fn url(&self) -> String {
        format!("{}/{}:generateContent", self.base, self.model)
    }

    fn body(&self, request: &AiRequest) -> Value {
        json!({
            "systemInstruction": { "parts": [{ "text": system_prompt(&request.instruction) }] },
            "contents": [{ "role": "user", "parts": [{ "text": user_message(request) }] }],
            "generationConfig": {
                "maxOutputTokens": MAX_OUTPUT_TOKENS,
                // JSON mode with the SRS 6.3 shape, so the reply is a bare list of ranges.
                "responseMimeType": "application/json",
                "responseSchema": {
                    "type": "ARRAY",
                    "items": {
                        "type": "OBJECT",
                        "properties": {
                            "start": { "type": "INTEGER" },
                            "end": { "type": "INTEGER" },
                            "title": { "type": "STRING" },
                        },
                        "required": ["start", "end", "title"],
                        "propertyOrdering": ["start", "end", "title"],
                    },
                },
            },
        })
    }
}

/// The model id for the URL path: "gemini-4-argon", also accepted as "models/gemini-4-argon".
/// Anything else that could change the path is refused.
fn model_id(model: &str) -> Result<String> {
    let id = model.trim().trim_start_matches("models/");
    let valid = !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'));
    if !valid || id.starts_with('.') {
        return Err(SplitError::Ai(format!("\"{model}\" isn't a Gemini model name. Check the model under AI in Settings")));
    }
    Ok(id.to_string())
}

impl Provider for Gemini {
    async fn complete(&self, request: &AiRequest) -> Result<String> {
        if request.pages.is_empty() {
            return Err(SplitError::Ai("There are no pages to send to Gemini".into()));
        }
        if request.instruction.trim().is_empty() {
            return Err(SplitError::Ai("Describe how to split the file before using Smart mode".into()));
        }
        let headers = [("x-goog-api-key", self.key.as_str())];
        let reply = self.api.post(&self.url(), &headers, &self.body(request), &self.model).await?;
        reply_text(&reply)
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Reply {
    #[serde(default)]
    candidates: Vec<Candidate>,
    prompt_feedback: Option<Feedback>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Feedback {
    block_reason: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    content: Option<Content>,
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Content {
    #[serde(default)]
    parts: Vec<Part>,
}

#[derive(Deserialize)]
struct Part {
    #[serde(default)]
    text: String,
    /// True on thought summaries, which aren't the answer.
    #[serde(default)]
    thought: bool,
}

/// The text of a successful reply. `response` parses and checks it like any other platform's, so
/// a reply of the wrong shape is asked for again once and then fixed by hand (FR-36).
fn reply_text(body: &str) -> Result<String> {
    let reply: Reply = serde_json::from_str(body)
        .map_err(|_| SplitError::Ai("Gemini sent an answer Splitr couldn't read. Try again".into()))?;
    if reply.prompt_feedback.and_then(|f| f.block_reason).is_some() {
        return Err(SplitError::Ai("Gemini declined to split this file. Try rewording the instruction".into()));
    }
    let candidate = reply
        .candidates
        .into_iter()
        .next()
        .ok_or_else(|| SplitError::Ai("Gemini sent an empty answer. Try again".into()))?;
    match candidate.finish_reason.as_deref() {
        None | Some("STOP") => {}
        Some("MAX_TOKENS") => {
            return Err(SplitError::Ai(
                "Gemini's answer was cut off because it was too long. Try splitting a smaller part of the file".into(),
            ));
        }
        Some("SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII") => {
            return Err(SplitError::Ai("Gemini declined to split this file. Try rewording the instruction".into()));
        }
        Some(other) => return Err(SplitError::Ai(format!("Gemini stopped before finishing ({other}). Try again"))),
    }
    let text: String = candidate
        .content
        .map(|c| c.parts.into_iter().filter(|p| !p.thought).map(|p| p.text).collect())
        .unwrap_or_default();
    if text.trim().is_empty() {
        return Err(SplitError::Ai("Gemini sent an empty answer. Try again".into()));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::PageExcerpt;
    use crate::ai::http::test_server::{body, serve};

    const KEY: &str = "AIzaSyTestKeyTestKeyTestKeyTestKeyWXYZ";
    const MODEL: &str = "gemini-4-argon";

    fn request() -> AiRequest {
        AiRequest {
            instruction: "Split by chapter".into(),
            pages: vec![
                PageExcerpt { page: 1, head: "Preface".into(), tail: "".into() },
                PageExcerpt { page: 2, head: "Chapter 1".into(), tail: "end".into() },
            ],
        }
    }

    fn run(base: String, model: &str) -> Result<String> {
        let gemini = Gemini::build(base, KEY.into(), model.into(), Duration::from_secs(5)).unwrap();
        tauri::async_runtime::block_on(gemini.complete(&request()))
    }

    const OK: &str = r#"{"candidates":[{"content":{"role":"model","parts":[
        {"text":"thinking about chapters","thought":true},
        {"text":"[{\"start\":1,\"end\":2,"},{"text":"\"title\":\"Book\"}]"}]},
        "finishReason":"STOP"}]}"#;

    #[test]
    fn sends_the_key_in_a_header_and_asks_for_json() {
        let (url, got) = serve("/v1beta/models/gemini-4-argon:generateContent", "200 OK", "", OK);
        let base = url.trim_end_matches("/gemini-4-argon:generateContent").to_string();
        assert_eq!(run(base, "models/gemini-4-argon").unwrap(), r#"[{"start":1,"end":2,"title":"Book"}]"#);

        let raw = got.recv().unwrap();
        let lower = raw.to_lowercase();
        assert!(lower.starts_with("post /v1beta/models/gemini-4-argon:generatecontent "), "{raw}");
        assert!(lower.contains(&format!("x-goog-api-key: {}", KEY.to_lowercase())));
        // The key is never in the URL, which error messages may show.
        assert!(!raw.lines().next().unwrap().contains(KEY));
        let sent = body(&raw);
        assert!(sent["systemInstruction"]["parts"][0]["text"].as_str().unwrap().contains("Task: Split by chapter"));
        let pages: Value = serde_json::from_str(sent["contents"][0]["parts"][0]["text"].as_str().unwrap()).unwrap();
        assert_eq!(pages["pages"][1]["head"], "Chapter 1");
        assert_eq!(sent["generationConfig"]["responseMimeType"], "application/json");
        assert_eq!(sent["generationConfig"]["responseSchema"]["items"]["required"], json!(["start", "end", "title"]));
    }

    #[test]
    fn errors_name_gemini_and_never_show_the_key() {
        let body = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT"}}"#;
        let (url, _got) = serve("/m/gemini-4-argon:generateContent", "400 Bad Request", "", body);
        let err = run(url.replace("/gemini-4-argon:generateContent", ""), MODEL).unwrap_err().to_string();
        assert!(err.contains("Gemini didn't accept your API key"), "{err}");
        assert!(!err.contains(KEY));
    }

    #[test]
    fn model_names_cant_change_the_path() {
        assert_eq!(model_id(" models/gemini-4-argon ").unwrap(), "gemini-4-argon");
        assert_eq!(model_id("gemini-3.5-flash_preview").unwrap(), "gemini-3.5-flash_preview");
        for bad in ["", "../files", "a/b", "x?key=1", "..", "a b"] {
            assert!(model_id(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn reply_text_handles_blocks_cut_offs_and_bad_bodies() {
        let blocked = r#"{"candidates":[],"promptFeedback":{"blockReason":"SAFETY"}}"#;
        assert!(reply_text(blocked).unwrap_err().to_string().contains("declined"));
        let safety = r#"{"candidates":[{"finishReason":"SAFETY"}]}"#;
        assert!(reply_text(safety).unwrap_err().to_string().contains("declined"));
        let cut = r#"{"candidates":[{"content":{"parts":[{"text":"[{\"start\":1"}]},"finishReason":"MAX_TOKENS"}]}"#;
        assert!(reply_text(cut).unwrap_err().to_string().contains("cut off"));
        let odd = r#"{"candidates":[{"finishReason":"OTHER"}]}"#;
        assert!(reply_text(odd).unwrap_err().to_string().contains("(OTHER)"));
        assert!(reply_text(r#"{"candidates":[]}"#).unwrap_err().to_string().contains("empty"));
        let object = r#"{"candidates":[{"content":{"parts":[{"text":"{\"parts\":[]}"}]},"finishReason":"STOP"}]}"#;
        // `response` checks the shape and retries once (FR-36); the provider passes the text on.
        assert_eq!(reply_text(object).unwrap(), r#"{"parts":[]}"#);
        assert!(reply_text("<html>").unwrap_err().to_string().contains("couldn't read"));
    }

    /// The manual check for #29. Needs a Gemini key saved in the app and network access; sends
    /// four made-up pages. Run with `cargo test real_gemini -- --ignored --nocapture`.
    #[test]
    #[ignore = "calls the real Gemini API with the key saved in the OS credential store"]
    fn real_gemini_call() {
        use crate::keystore::{self, OsStore};
        let key = keystore::key_for(&OsStore, "gemini").unwrap();
        let gemini = Gemini::new(key, crate::ai::default_model("gemini").unwrap().into()).unwrap();
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
        let reply = tauri::async_runtime::block_on(gemini.complete(&request)).unwrap();
        println!("{reply}");
        assert!(reply.trim_start().starts_with('['), "{reply}");
    }
}
