//! Smart mode on the user's ChatGPT plan (#31): the Responses API with the access token from Sign
//! in with ChatGPT. Plan usage needs `store: false` and `stream: true`, and leaves out settings
//! such as `max_output_tokens`. The token goes only to OpenAI and is never logged.

use serde_json::{Value, json};

use super::http::{CONNECT_TIMEOUT, TIMEOUT, same_host_only};
use super::{AiRequest, Provider, system_prompt, user_message};
use crate::error::{Result, SplitError};

const API_URL: &str = "https://api.openai.com/v1/responses";

/// A client for one split. Deliberately not `Debug`, so the token can't end up in a log.
pub struct ChatGpt {
    http: reqwest::Client,
    url: String,
    token: String,
    model: String,
}

impl ChatGpt {
    pub fn new(token: String, model: String) -> Result<Self> {
        Self::with_url(API_URL.into(), token, model)
    }

    fn with_url(url: String, token: String, model: String) -> Result<Self> {
        let http = reqwest::Client::builder()
            .https_only(cfg!(not(test)))
            .redirect(same_host_only())
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(TIMEOUT)
            .build()
            .map_err(|e| SplitError::Ai(format!("Splitr couldn't set up its connection to ChatGPT ({e})")))?;
        Ok(Self { http, url, token, model })
    }

    fn body(&self, request: &AiRequest) -> Value {
        json!({
            "model": self.model,
            "instructions": system_prompt(&request.instruction),
            "input": [{ "role": "user", "content": user_message(request) }],
            "store": false,
            "stream": true,
        })
    }
}

impl Provider for ChatGpt {
    async fn complete(&self, request: &AiRequest) -> Result<String> {
        if request.pages.is_empty() {
            return Err(SplitError::Ai("There are no pages to send to ChatGPT".into()));
        }
        let response = self
            .http
            .post(&self.url)
            .bearer_auth(&self.token)
            .header("accept", "text/event-stream")
            .json(&self.body(request))
            .send()
            .await
            .map_err(|e| network_error(&e))?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(|e| network_error(&e))?;
        if !(200..300).contains(&status) {
            return Err(error_for(status, &ErrorBody::read(&body)));
        }
        stream_text(&body)
    }
}

fn network_error(e: &reqwest::Error) -> SplitError {
    if e.is_timeout() {
        SplitError::Ai(format!("ChatGPT didn't answer within {} seconds. Try again", TIMEOUT.as_secs()))
    } else if e.is_connect() {
        SplitError::Ai("Splitr couldn't reach ChatGPT. Check your internet connection and try again".into())
    } else {
        SplitError::Ai(format!("The request to ChatGPT failed ({e})"))
    }
}

/// The `code`, `message` and `param` of an error, from a JSON error body or a failed stream event.
#[derive(Debug, Default)]
struct ErrorBody {
    code: String,
    message: Option<String>,
    param: Option<String>,
}

impl ErrorBody {
    fn read(body: &str) -> Self {
        let value: Value = serde_json::from_str(body).unwrap_or_default();
        Self::from_error(value.get("error").unwrap_or(&value))
    }

    fn from_error(error: &Value) -> Self {
        let text = |k: &str| error.get(k).and_then(Value::as_str).map(str::trim).filter(|s| !s.is_empty()).map(String::from);
        // Admission errors may be {"detail": "..."} instead.
        Self { code: text("code").unwrap_or_default(), message: text("message").or_else(|| text("detail")), param: text("param") }
    }
}

/// A clear message for an error from ChatGPT plan usage (OpenAI's "Errors and recovery").
fn error_for(status: u16, e: &ErrorBody) -> SplitError {
    let detail = e.message.as_ref().map(|m| format!(" ({m})")).unwrap_or_default();
    SplitError::Ai(match (e.code.as_str(), status) {
        ("subscription_sharing_usage_limit_exceeded", _) | ("", 429) => {
            "You've reached the limit Splitr may use of your ChatGPT plan. Check ChatGPT → Settings → Usage, \
             or use an API key instead"
                .into()
        }
        ("subscription_sharing_user_not_eligible", _) => {
            "Your ChatGPT plan can't be used in other apps. Use an API key under AI in Settings instead".into()
        }
        ("subscription_sharing_usage_unavailable" | "subscription_sharing_user_unavailable", _) | ("", 503) => {
            "ChatGPT couldn't check your plan usage just now. Try again in a minute".into()
        }
        ("subscription_sharing_unsupported_capability", _) => {
            let what = e.param.as_ref().map(|p| format!(" ({p})")).unwrap_or_default();
            format!("ChatGPT can't use this setting with your plan{what}. Check the model under AI in Settings")
        }
        ("subscription_sharing_invalid_user", _) | ("", 401) => {
            "ChatGPT didn't accept your sign-in. Disconnect and connect ChatGPT again under AI in Settings".into()
        }
        ("chatpass_v2_scope_not_authorized" | "chatpass_v2_invalid_authorization_context", _) => {
            "Splitr isn't allowed to use your ChatGPT plan. Connect ChatGPT again and allow plan usage".into()
        }
        ("subscription_sharing_route_not_supported", _) | ("", 403) => format!("ChatGPT refused this request{detail}"),
        ("model_not_found", _) | ("", 404) => {
            "ChatGPT doesn't offer this model to your account. Check the model name under AI in Settings".into()
        }
        (_, 500..=599) => format!("ChatGPT had a problem on its side (error {status}). Try again in a minute"),
        // A stream that failed after the 200 it started with: that status says nothing.
        (_, 200) => format!("ChatGPT couldn't finish its answer{detail}. Try again"),
        _ => format!("ChatGPT couldn't answer (error {status}){detail}"),
    })
}

/// The answer text of a streamed reply (server-sent events). Only `response.completed` counts as
/// success; `response.failed` carries an error code, and `response.incomplete` was cut off.
fn stream_text(body: &str) -> Result<String> {
    let mut text = String::new();
    let mut completed = false;
    for line in body.lines() {
        let Some(data) = line.strip_prefix("data:").map(str::trim) else { continue };
        let Ok(event) = serde_json::from_str::<Value>(data) else { continue };
        match event.get("type").and_then(Value::as_str).unwrap_or("") {
            "response.output_text.delta" => text.push_str(event.get("delta").and_then(Value::as_str).unwrap_or("")),
            "response.refusal.delta" => {
                return Err(SplitError::Ai("ChatGPT declined to split this file. Try rewording the instruction".into()));
            }
            "response.completed" => completed = true,
            "response.failed" => {
                let error = event.pointer("/response/error").cloned().unwrap_or_default();
                return Err(error_for(200, &ErrorBody::from_error(&error)));
            }
            "response.incomplete" => {
                return Err(SplitError::Ai(
                    "ChatGPT's answer was cut off. Try splitting a smaller part of the file".into(),
                ));
            }
            "error" => return Err(error_for(200, &ErrorBody::from_error(event.get("error").unwrap_or(&event)))),
            _ => {}
        }
    }
    if !completed {
        return Err(SplitError::Ai("ChatGPT stopped before finishing its answer. Try again".into()));
    }
    if text.trim().is_empty() {
        return Err(SplitError::Ai("ChatGPT sent an empty answer. Try again".into()));
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::PageExcerpt;
    use crate::ai::http::test_server::{body, serve};

    fn request() -> AiRequest {
        AiRequest {
            instruction: "Split by chapter".into(),
            pages: vec![PageExcerpt { page: 1, head: "Chapter 1".into(), tail: String::new() }],
        }
    }

    fn sse(events: &[Value]) -> String {
        events.iter().map(|e| format!("event: {}\ndata: {e}\n\n", e["type"].as_str().unwrap())).collect()
    }

    fn call(status: &str, reply: &str) -> (Result<String>, String) {
        let (url, got) = serve("/v1/responses", status, "", reply);
        let client = ChatGpt::with_url(url, "access-token-WXYZ".into(), "gpt-6.1-sol".into()).unwrap();
        let result = tauri::async_runtime::block_on(client.complete(&request()));
        (result, got.recv().unwrap())
    }

    #[test]
    fn streams_the_answer_with_the_settings_plan_usage_needs() {
        let reply = sse(&[
            json!({"type": "response.created"}),
            json!({"type": "response.output_text.delta", "delta": "[{\"start\":1,"}),
            json!({"type": "response.output_text.delta", "delta": "\"end\":1,\"title\":\"One\"}]"}),
            json!({"type": "response.completed"}),
        ]);
        let (result, raw) = call("200 OK", &reply);
        assert_eq!(result.unwrap(), r#"[{"start":1,"end":1,"title":"One"}]"#);
        assert!(raw.to_lowercase().contains("authorization: bearer access-token-wxyz"));
        let sent = body(&raw);
        assert_eq!((sent["store"].clone(), sent["stream"].clone()), (json!(false), json!(true)));
        assert_eq!(sent["input"][0]["role"], "user");
        assert!(sent["instructions"].as_str().unwrap().contains("Task: Split by chapter"));
        for left_out in ["max_output_tokens", "temperature", "metadata", "user"] {
            assert!(sent.get(left_out).is_none(), "{left_out} must not be sent");
        }
    }

    #[test]
    fn plan_limits_and_other_errors_say_what_to_do() {
        let failed = |code: &str| sse(&[json!({"type": "response.failed", "response": {"error": {"code": code, "message": "m"}}})]);
        let (r, _) = call("200 OK", &failed("subscription_sharing_usage_limit_exceeded"));
        assert!(r.unwrap_err().to_string().contains("Settings → Usage"));
        let (r, _) = call("403 Forbidden", r#"{"error":{"code":"subscription_sharing_user_not_eligible"}}"#);
        assert!(r.unwrap_err().to_string().contains("can't be used in other apps"));
        let (r, _) = call("503 Service Unavailable", r#"{"detail":"direct routing unavailable"}"#);
        assert!(r.unwrap_err().to_string().contains("Try again in a minute"));
        let (r, _) = call("400 Bad Request", r#"{"error":{"code":"subscription_sharing_unsupported_capability","param":"model"}}"#);
        assert!(r.unwrap_err().to_string().contains("(model)"));
        let (r, _) = call("401 Unauthorized", "{}");
        let err = r.unwrap_err().to_string();
        assert!(err.contains("connect ChatGPT again") && !err.contains("access-token"), "{err}");
    }

    #[test]
    fn an_unfinished_or_cut_off_stream_is_an_error() {
        let (r, _) = call("200 OK", &sse(&[json!({"type": "response.output_text.delta", "delta": "[]"})]));
        assert!(r.unwrap_err().to_string().contains("stopped before finishing"));
        let (r, _) = call("200 OK", &sse(&[json!({"type": "response.incomplete"})]));
        assert!(r.unwrap_err().to_string().contains("cut off"));
        let (r, _) = call("200 OK", &sse(&[json!({"type": "response.completed"})]));
        assert!(r.unwrap_err().to_string().contains("empty answer"));
    }
}
