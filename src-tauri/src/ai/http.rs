//! The HTTPS calls shared by the API-key platforms (#28 to #30): one JSON POST, with network and
//! HTTP errors turned into messages that name the platform (SRS 5.4).
//!
//! Keys travel only in the headers the caller passes, to the one platform they belong to
//! (NFR-05). reqwest's errors name the URL, never the headers, so a key can't reach a message.

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::Value;

use crate::error::{Result, SplitError};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// A large chunk can take a model a while to think through.
pub const TIMEOUT: Duration = Duration::from_secs(180);

/// A client for one platform. Deliberately not `Debug`, so nothing it is given can end up in a log.
pub struct Api {
    http: reqwest::Client,
    /// The platform's name, as the user knows it ("OpenAI").
    name: String,
    timeout: Duration,
}

impl Api {
    pub fn new(name: &str, timeout: Duration) -> Result<Self> {
        Self::build(name.into(), timeout, false)
    }

    /// Like [`Api::new`]; `local` allows plain HTTP, for an address on this computer that the
    /// caller has checked (a custom platform or local model, #30, #25).
    pub fn build(name: String, timeout: Duration, local: bool) -> Result<Self> {
        let http = reqwest::Client::builder()
            // Tests talk to a local server over plain HTTP; the app uses HTTPS except on this computer.
            .https_only(cfg!(not(test)) && !local)
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(timeout)
            .build()
            .map_err(|e| SplitError::Ai(format!("Splitr couldn't set up its connection to {name} ({e})")))?;
        Ok(Self { http, name, timeout })
    }

    /// POSTs `body` as JSON to `url` with `headers`, and returns the reply body of a successful
    /// call. `model` names the model in a "not found" message.
    pub async fn post(&self, url: &str, headers: &[(&str, &str)], body: &Value, model: &str) -> Result<String> {
        self.send(self.http.post(url).json(body), headers, model).await
    }

    /// GETs `url` with `headers`, and returns the reply body of a successful call.
    pub async fn get(&self, url: &str, headers: &[(&str, &str)]) -> Result<String> {
        self.send(self.http.get(url), headers, "").await
    }

    async fn send(&self, mut call: reqwest::RequestBuilder, headers: &[(&str, &str)], model: &str) -> Result<String> {
        for (name, value) in headers {
            call = call.header(*name, *value);
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
            return Err(self.status_error(status, &text, retry_after, model));
        }
        Ok(text)
    }

    fn network_error(&self, e: &reqwest::Error) -> SplitError {
        let name = &self.name;
        if e.is_timeout() {
            SplitError::Ai(format!(
                "{name} didn't answer within {} seconds. Try again, or split a smaller part of the file",
                self.timeout.as_secs()
            ))
        } else if e.is_connect() {
            SplitError::Ai(format!("Splitr couldn't reach {name}. Check your internet connection and try again"))
        } else {
            SplitError::Ai(format!("The request to {name} failed ({e})"))
        }
    }

    fn status_error(&self, status: StatusCode, body: &str, retry_after: Option<u64>, model: &str) -> SplitError {
        let name = &self.name;
        let error = ApiError::read(body);
        let detail = error.message.as_ref().map(|m| format!(" ({m})")).unwrap_or_default();
        let message = match status.as_u16() {
            // A bad key is a 401 on most platforms and a 400 or 403 with a key error on some.
            _ if error.is_bad_key() => format!("{name} didn't accept your API key. Check it in Settings"),
            401 => format!("{name} didn't accept your API key. Check it in Settings"),
            402 => format!("Your {name} account has no credit left for this request. Check billing on {name}'s site{detail}"),
            // Running out of quota is a 429 too, but waiting won't help.
            429 if error.is_quota() => {
                format!("You've used up your {name} quota or credit. Check your plan and billing on {name}'s site{detail}")
            }
            429 => match retry_after {
                Some(secs) => format!("You've reached your {name} rate limit. Wait {secs} seconds and try again"),
                None => format!("You've reached your {name} rate limit. Wait a minute and try again"),
            },
            400 => format!("{name} couldn't accept this request{detail}"),
            403 => format!("Your {name} API key isn't allowed to make this request{detail}"),
            404 => format!("{name} doesn't offer the model \"{model}\" to your account. Check the model name in Settings"),
            413 => format!("This request is too large for {name}. Split a smaller part of the file"),
            500..=599 => format!("{name} had a problem on its side (error {}). Try again in a minute", status.as_u16()),
            code => format!("{name} answered with error {code}{detail}"),
        };
        SplitError::Ai(message)
    }
}

/// The parts of an error body that platforms share, in the OpenAI style
/// (`{"error": {"message", "type", "code"}}`) or the Google style (`{"error": {"message", "status"}}`).
#[derive(Debug, Default, PartialEq)]
struct ApiError {
    message: Option<String>,
    /// `type`, `code` and `status`, lower case, to match on.
    kinds: Vec<String>,
}

impl ApiError {
    fn read(body: &str) -> Self {
        let Ok(value) = serde_json::from_str::<Value>(body) else {
            return Self::default();
        };
        // Some platforms send a list with one error.
        let error = match value.get("error") {
            Some(Value::Array(list)) => list.first().cloned().unwrap_or_default(),
            Some(error) => error.clone(),
            None => value.get(0).and_then(|v| v.get("error")).cloned().unwrap_or_default(),
        };
        let message = error
            .get("message")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|m| !m.is_empty())
            .map(String::from);
        let kinds = ["type", "code", "status"]
            .iter()
            .filter_map(|field| error.get(field).and_then(Value::as_str))
            .map(str::to_lowercase)
            .collect();
        Self { message, kinds }
    }

    fn has(&self, kind: &str) -> bool {
        self.kinds.iter().any(|k| k == kind)
    }

    fn is_bad_key(&self) -> bool {
        self.has("invalid_api_key")
            || self.message.as_deref().is_some_and(|m| m.to_lowercase().contains("api key not valid"))
    }

    fn is_quota(&self) -> bool {
        self.has("insufficient_quota")
            || self.message.as_deref().is_some_and(|m| {
                let m = m.to_lowercase();
                // Gemini reports per-minute limits as a quota too; those pass if you wait.
                let per_minute = m.contains("per minute") || m.contains("perminute") || m.contains("per_minute");
                !per_minute && (m.contains("quota") || m.contains("billing") || m.contains("credit"))
            })
    }
}

/// A local HTTP server for provider tests: serves one canned response and sends back the raw
/// request it got.
#[cfg(test)]
pub mod test_server {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc;
    use std::thread;

    /// Serves one response with `status`, extra `headers` (each ending in "\r\n") and `body` at
    /// `http://127.0.0.1:<port><path>`, which it returns.
    pub fn serve(path: &str, status: &str, headers: &str, body: &str) -> (String, mpsc::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}{path}", listener.local_addr().unwrap());
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

    /// The JSON body of a raw request.
    pub fn body(raw: &str) -> serde_json::Value {
        serde_json::from_str(&raw[raw.find("\r\n\r\n").unwrap() + 4..]).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::test_server::serve;
    use super::*;
    use serde_json::json;

    fn post(url: &str) -> Result<String> {
        let api = Api::new("Example", Duration::from_secs(5)).unwrap();
        tauri::async_runtime::block_on(api.post(url, &[("authorization", "Bearer secret-key")], &json!({"a": 1}), "m-1"))
    }

    #[test]
    fn sends_the_headers_and_body_and_returns_the_reply() {
        let (url, got) = serve("/v1/x", "200 OK", "", r#"{"ok":true}"#);
        assert_eq!(post(&url).unwrap(), r#"{"ok":true}"#);
        let raw = got.recv().unwrap();
        assert!(raw.to_lowercase().contains("authorization: bearer secret-key"));
        assert_eq!(test_server::body(&raw), json!({"a": 1}));
    }

    #[test]
    fn http_errors_become_clear_messages_that_name_the_platform() {
        let openai_error = |kind: &str, message: &str| {
            format!(r#"{{"error":{{"message":"{message}","type":"{kind}","code":"{kind}"}}}}"#)
        };
        let cases = [
            ("401 Unauthorized", "", openai_error("invalid_request_error", "x"), "Example didn't accept your API key"),
            ("400 Bad Request", "", openai_error("invalid_api_key", "bad"), "didn't accept your API key"),
            (
                "400 Bad Request",
                "",
                r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT"}}"#.into(),
                "didn't accept your API key",
            ),
            ("429 Too Many Requests", "", openai_error("insufficient_quota", "You exceeded your current quota"), "used up your Example quota"),
            ("429 Too Many Requests", "retry-after: 30\r\n", openai_error("rate_limit_exceeded", "slow down"), "Wait 30 seconds"),
            ("429 Too Many Requests", "", "{}".into(), "Wait a minute"),
            (
                "429 Too Many Requests",
                "",
                r#"{"error":{"code":429,"message":"Quota exceeded for metric: generate_content_requests per minute","status":"RESOURCE_EXHAUSTED"}}"#.into(),
                "rate limit",
            ),
            ("404 Not Found", "", "{}".into(), "doesn't offer the model \"m-1\""),
            ("400 Bad Request", "", openai_error("invalid_request_error", "max tokens"), "couldn't accept this request (max tokens)"),
            ("503 Service Unavailable", "", "<html>".into(), "error 503"),
        ];
        for (status, headers, body, expected) in cases {
            let (url, _got) = serve("/v1/x", status, headers, &body);
            let err = post(&url).unwrap_err().to_string();
            assert!(err.contains(expected), "{status} {body}: {err}");
            assert!(!err.contains("secret-key"), "{err}");
        }
    }

    #[test]
    fn no_server_is_a_connection_error() {
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
        let err = post(&format!("http://127.0.0.1:{port}/v1/x")).unwrap_err();
        assert!(err.to_string().contains("couldn't reach Example"), "{err}");
    }

    #[test]
    fn error_bodies_are_read_in_every_style() {
        let list = ApiError::read(r#"[{"error":{"message":" busy ","status":"UNAVAILABLE"}}]"#);
        assert_eq!(list, ApiError { message: Some("busy".into()), kinds: vec!["unavailable".into()] });
        assert_eq!(ApiError::read("not json"), ApiError::default());
        assert_eq!(ApiError::read(r#"{"error":{"message":""}}"#).message, None);
    }
}
