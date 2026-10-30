//! The Claude API (Anthropic Messages API) over HTTPS (#15, SRS 5.4).
//!
//! The key is read from the credential store for each call and sent only to Anthropic, in the
//! `x-api-key` header (NFR-05). It is never logged and never part of an error message.

use std::time::Duration;

use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{Value, json};

use super::{AiRequest, Provider, system_prompt, user_message};
use crate::error::{Result, SplitError};

const API_URL: &str = "https://api.anthropic.com/v1/messages";
const API_VERSION: &str = "2023-06-01";
/// A reply is a short JSON list of ranges, so this leaves plenty of room after thinking.
const MAX_TOKENS: u32 = 16_000;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// A large chunk can take the model a while to think through.
const TIMEOUT: Duration = Duration::from_secs(180);

/// Models that accept server-side refusal fallbacks: if the model declines, Anthropic retries the
/// request on another model inside the same call.
const FALLBACK_MODELS: &[&str] = &["claude-opus-5-5", "claude-opus-5", "claude-fable-5-1", "claude-sonnet-5-5"];
const FALLBACK_BETA: &str = "server-side-fallback-2026-07-01";

/// A client for one call. Deliberately not `Debug`, so the key can't end up in a log.
pub struct Claude {
    http: reqwest::Client,
    url: String,
    key: String,
    model: String,
    timeout: Duration,
}

impl Claude {
    pub fn new(key: String, model: String) -> Result<Self> {
        Self::build(API_URL.into(), key, model, TIMEOUT)
    }

    fn build(url: String, key: String, model: String, timeout: Duration) -> Result<Self> {
        let http = reqwest::Client::builder()
            // Tests talk to a local server over plain HTTP; the app only ever uses HTTPS.
            .https_only(cfg!(not(test)))
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(timeout)
            .build()
            .map_err(|e| SplitError::Ai(format!("Splitr couldn't set up its connection to Claude ({e})")))?;
        Ok(Self { http, url, key, model, timeout })
    }

    fn body(&self, request: &AiRequest) -> Value {
        let mut body = json!({
            "model": self.model,
            "max_tokens": MAX_TOKENS,
            "system": system_prompt(&request.instruction),
            "messages": [{ "role": "user", "content": user_message(request) }],
        });
        if FALLBACK_MODELS.contains(&self.model.as_str()) {
            body["fallbacks"] = json!("default");
        }
        body
    }

    async fn send(&self, request: &AiRequest) -> Result<String> {
        let mut call = self
            .http
            .post(&self.url)
            .header("x-api-key", &self.key)
            .header("anthropic-version", API_VERSION)
            .json(&self.body(request));
        if FALLBACK_MODELS.contains(&self.model.as_str()) {
            call = call.header("anthropic-beta", FALLBACK_BETA);
        }
        let response = call.send().await.map_err(|e| self.network_error(&e))?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let text = response.text().await.map_err(|e| self.network_error(&e))?;
        if !status.is_success() {
            return Err(self.status_error(status, &text, retry_after));
        }
        reply_text(&text)
    }

    fn network_error(&self, e: &reqwest::Error) -> SplitError {
        // reqwest's messages name the URL, never the headers, so the key can't appear in them.
        if e.is_timeout() {
            SplitError::Ai(format!(
                "Claude didn't answer within {} seconds. Try again, or split a smaller part of the file",
                self.timeout.as_secs()
            ))
        } else if e.is_connect() {
            SplitError::Ai("Splitr couldn't reach Claude. Check your internet connection and try again".into())
        } else {
            SplitError::Ai(format!("The request to Claude failed ({e})"))
        }
    }

    fn status_error(&self, status: StatusCode, body: &str, retry_after: Option<u64>) -> SplitError {
        let detail = api_message(body).map(|m| format!(" ({m})")).unwrap_or_default();
        let message = match status.as_u16() {
            400 => format!("Claude couldn't accept this request{detail}"),
            401 => "Claude didn't accept your API key. Check it in Settings, or create a new one in the \
                    Anthropic Console"
                .into(),
            402 => format!("Your Anthropic account can't pay for this request. Check billing in the Anthropic Console{detail}"),
            403 => format!("Your API key isn't allowed to make this request{detail}"),
            404 => format!(
                "Claude doesn't offer the model \"{}\" to your account. Check the model name in Settings",
                self.model
            ),
            413 => "This request is too large for Claude. Split a smaller part of the file".into(),
            429 => match retry_after {
                Some(secs) => format!("You've reached your Claude rate limit. Wait {secs} seconds and try again"),
                None => "You've reached your Claude rate limit. Wait a minute and try again".into(),
            },
            529 => "Claude is overloaded right now. Try again in a minute".into(),
            500..=599 => format!("Claude had a problem on its side (error {}). Try again in a minute", status.as_u16()),
            code => format!("Claude answered with error {code}{detail}"),
        };
        SplitError::Ai(message)
    }
}

impl Provider for Claude {
    async fn complete(&self, request: &AiRequest) -> Result<String> {
        if request.pages.is_empty() {
            return Err(SplitError::Ai("There are no pages to send to Claude".into()));
        }
        if request.instruction.trim().is_empty() {
            return Err(SplitError::Ai("Describe how to split the file before using Smart mode".into()));
        }
        self.send(request).await
    }
}

#[derive(Deserialize)]
struct Reply {
    #[serde(default)]
    content: Vec<Block>,
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

/// The text of a successful reply. Thinking and fallback blocks are skipped.
fn reply_text(body: &str) -> Result<String> {
    reply_text_for("Claude", body)
}

/// Like `reply_text`, for an Anthropic-style reply from the platform `name`, such as OpenCode Go (#32).
pub(crate) fn reply_text_for(name: &str, body: &str) -> Result<String> {
    let reply: Reply = serde_json::from_str(body)
        .map_err(|_| SplitError::Ai(format!("{name} sent an answer Splitr couldn't read. Try again")))?;
    match reply.stop_reason.as_deref() {
        Some("refusal") => {
            return Err(SplitError::Ai(format!("{name} declined to split this file. Try rewording the instruction")));
        }
        Some("max_tokens") => {
            return Err(SplitError::Ai(format!(
                "{name}'s answer was cut off because it was too long. Try splitting a smaller part of the file"
            )));
        }
        _ => {}
    }
    let text: String = reply.content.iter().filter(|b| b.kind == "text").map(|b| b.text.as_str()).collect();
    if text.trim().is_empty() {
        return Err(SplitError::Ai(format!("{name} sent an empty answer. Try again")));
    }
    Ok(text)
}

/// The `error.message` of an API error body, if there is one.
fn api_message(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let message = value.get("error")?.get("message")?.as_str()?.trim();
    (!message.is_empty()).then(|| message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::PageExcerpt;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    const KEY: &str = "sk-ant-api03-test-key-WXYZ";
    const MODEL: &str = "claude-opus-5-5";

    fn request() -> AiRequest {
        AiRequest {
            instruction: "Split by chapter".into(),
            pages: vec![
                PageExcerpt { page: 1, head: "Preface".into(), tail: "".into() },
                PageExcerpt { page: 2, head: "Chapter 1".into(), tail: "end".into() },
            ],
        }
    }

    /// Serves one canned HTTP response on a local port and sends back the raw request it got.
    fn serve(status: &str, headers: &str, body: &str) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/messages", listener.local_addr().unwrap());
        let response = format!(
            "HTTP/1.1 {status}\r\ncontent-type: application/json\r\n{headers}content-length: {}\r\nconnection: close\r\n\r\n{body}",
            body.len()
        );
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut raw = Vec::new();
            let mut buf = [0; 4096];
            // Read the headers, then the body by its length.
            loop {
                let n = stream.read(&mut buf).unwrap();
                raw.extend_from_slice(&buf[..n]);
                let text = String::from_utf8_lossy(&raw).to_string();
                if let Some(end) = text.find("\r\n\r\n") {
                    let len = text[..end]
                        .lines()
                        .find_map(|l| l.to_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap()))
                        .unwrap_or(0);
                    if raw.len() >= end + 4 + len || n == 0 {
                        break;
                    }
                }
            }
            stream.write_all(response.as_bytes()).unwrap();
            tx.send(String::from_utf8_lossy(&raw).to_string()).unwrap();
        });
        (url, rx)
    }

    fn client(url: String, model: &str) -> Claude {
        Claude::build(url, KEY.into(), model.into(), Duration::from_secs(5)).unwrap()
    }

    fn run(claude: &Claude) -> Result<String> {
        tauri::async_runtime::block_on(claude.complete(&request()))
    }

    const OK: &str = r#"{"content":[{"type":"thinking","thinking":""},{"type":"text","text":"[{\"start\":1,"},
        {"type":"text","text":"\"end\":2,\"title\":\"Book\"}]"}],"stop_reason":"end_turn"}"#;

    #[test]
    fn sends_the_key_version_and_prompt_and_returns_the_text() {
        let (url, got) = serve("200 OK", "", OK);
        let reply = run(&client(url, MODEL)).unwrap();
        assert_eq!(reply, r#"[{"start":1,"end":2,"title":"Book"}]"#);

        let raw = got.recv().unwrap();
        let lower = raw.to_lowercase();
        assert!(lower.starts_with("post /v1/messages "), "{raw}");
        assert!(lower.contains(&format!("x-api-key: {}", KEY.to_lowercase())));
        assert!(lower.contains("anthropic-version: 2023-06-01"));
        assert!(lower.contains(&format!("anthropic-beta: {FALLBACK_BETA}")));
        let body: Value = serde_json::from_str(&raw[raw.find("\r\n\r\n").unwrap() + 4..]).unwrap();
        assert_eq!(body["model"], MODEL);
        assert_eq!(body["fallbacks"], "default");
        assert!(body["system"].as_str().unwrap().contains("Task: Split by chapter"));
        let pages: Value = serde_json::from_str(body["messages"][0]["content"].as_str().unwrap()).unwrap();
        assert_eq!(pages["pages"][1]["head"], "Chapter 1");
        // Current models reject sampling settings and thinking budgets.
        for field in ["temperature", "top_p", "top_k", "thinking"] {
            assert!(body.get(field).is_none(), "{field} was sent");
        }
    }

    #[test]
    fn other_models_get_no_fallbacks() {
        let (url, got) = serve("200 OK", "", OK);
        run(&client(url, "claude-haiku-5-5")).unwrap();
        let raw = got.recv().unwrap();
        assert!(!raw.to_lowercase().contains("anthropic-beta"));
        assert!(!raw.contains("fallbacks"));
    }

    #[test]
    fn http_errors_become_clear_messages_without_the_key() {
        let cases = [
            ("401 Unauthorized", "", "didn't accept your API key"),
            ("404 Not Found", "", "doesn't offer the model \"claude-nope\""),
            ("429 Too Many Requests", "retry-after: 30\r\n", "Wait 30 seconds"),
            ("429 Too Many Requests", "", "Wait a minute"),
            ("529 Overloaded", "", "overloaded"),
            ("500 Internal Server Error", "", "error 500"),
            ("400 Bad Request", "", "(max_tokens is too large)"),
        ];
        for (status, headers, expected) in cases {
            let body = r#"{"type":"error","error":{"type":"x","message":"max_tokens is too large"}}"#;
            let (url, _got) = serve(status, headers, body);
            let err = run(&client(url, "claude-nope")).unwrap_err().to_string();
            assert!(err.contains(expected), "{status}: {err}");
            assert!(!err.contains(KEY), "{err}");
        }
    }

    #[test]
    fn a_slow_answer_times_out() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1/messages", listener.local_addr().unwrap());
        let hold = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            thread::sleep(Duration::from_secs(3));
            drop(stream);
        });
        let claude = Claude::build(url, KEY.into(), MODEL.into(), Duration::from_millis(300)).unwrap();
        let err = run(&claude).unwrap_err().to_string();
        assert!(err.contains("didn't answer within"), "{err}");
        hold.join().unwrap();
    }

    #[test]
    fn no_server_is_a_connection_error() {
        // Bind and drop to get a port nothing listens on.
        let port = TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let err = run(&client(format!("http://127.0.0.1:{port}/v1/messages"), MODEL)).unwrap_err();
        assert!(err.to_string().contains("couldn't reach Claude"), "{err}");
    }

    #[test]
    fn empty_requests_are_not_sent() {
        let claude = client("http://127.0.0.1:9/v1/messages".into(), MODEL);
        let mut empty = request();
        empty.pages.clear();
        let err = tauri::async_runtime::block_on(claude.complete(&empty)).unwrap_err();
        assert!(err.to_string().contains("no pages"), "{err}");
        let mut no_task = request();
        no_task.instruction = "  ".into();
        assert!(tauri::async_runtime::block_on(claude.complete(&no_task)).is_err());
    }

    #[test]
    fn reply_text_handles_stop_reasons_and_bad_bodies() {
        let refusal = r#"{"content":[],"stop_reason":"refusal","stop_details":{"type":"refusal"}}"#;
        assert!(reply_text(refusal).unwrap_err().to_string().contains("declined"));
        let cut = r#"{"content":[{"type":"text","text":"[{\"start\":1"}],"stop_reason":"max_tokens"}"#;
        assert!(reply_text(cut).unwrap_err().to_string().contains("cut off"));
        let empty = r#"{"content":[{"type":"text","text":"  "}],"stop_reason":"end_turn"}"#;
        assert!(reply_text(empty).unwrap_err().to_string().contains("empty"));
        assert!(reply_text("<html>").unwrap_err().to_string().contains("couldn't read"));
    }

    #[test]
    fn api_message_reads_only_error_bodies() {
        assert_eq!(api_message(r#"{"error":{"message":" bad "}}"#).as_deref(), Some("bad"));
        assert_eq!(api_message(r#"{"error":{"message":""}}"#), None);
        assert_eq!(api_message("not json"), None);
    }

    /// The manual check for #15. Needs a Claude key saved in the app and network access; sends
    /// four made-up pages. Run with `cargo test real_claude -- --ignored --nocapture`.
    #[test]
    #[ignore = "calls the real Claude API with the key saved in the OS credential store"]
    fn real_claude_call() {
        use crate::keystore::{self, OsStore};
        let key = keystore::key_for(&OsStore, "anthropic").unwrap();
        let model = crate::ai::default_model("anthropic").unwrap();
        let claude = Claude::new(key, model.into()).unwrap();
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
        let reply = tauri::async_runtime::block_on(claude.complete(&request)).unwrap();
        println!("{reply}");
        assert!(reply.trim_start().starts_with('['), "{reply}");
    }
}
