//! A model running on this computer, so Smart mode works offline (FR-81, #25). Ollama, LM Studio,
//! llama.cpp's server and Jan all offer the OpenAI chat completions format, so this reuses
//! `compatible::ChatCompletions` without a key.
//!
//! The address must be on this computer (localhost, 127.0.0.1 or ::1), so no page text leaves it.
//! `http::Api` also turns off the system proxy and redirects for these calls, and Ollama's cloud
//! models, which pass the prompt on to Ollama's servers, are refused.

use std::time::Duration;

use reqwest::Url;

use super::compatible::ChatCompletions;
use crate::error::{Result, SplitError};

/// The service id in settings.
pub const LOCAL: &str = "local";
/// The name shown in messages.
const NAME: &str = "Local model";
/// Ollama's OpenAI-compatible address, used until the user enters another.
pub const DEFAULT_URL: &str = "http://localhost:11434/v1";
/// A model on an ordinary computer, without a fast graphics card, can take minutes per chunk.
const TIMEOUT: Duration = Duration::from_secs(600);

/// The base address of the local model server, without a trailing "/". It must be on this
/// computer; plain HTTP is fine there.
pub fn check_url(url: &str) -> Result<String> {
    let url = url.trim();
    let bad = || {
        SplitError::Ai(format!(
            "\"{url}\" isn't an address on this computer. Enter one such as {DEFAULT_URL} under AI in Settings"
        ))
    };
    let parsed = Url::parse(url).map_err(|_| bad())?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(bad());
    }
    if !matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")) {
        return Err(SplitError::Ai(format!(
            "The local model's address must be on this computer (localhost), so no page text leaves it. For a server \
             elsewhere, choose \"Other OpenAI-compatible platform\" instead of \"{url}\""
        )));
    }
    if !parsed.username().is_empty() || parsed.password().is_some() || parsed.query().is_some() || parsed.fragment().is_some()
    {
        return Err(SplitError::Ai(
            "Enter the local model's address without a user name, password or \"?\" part".into(),
        ));
    }
    Ok(url.trim_end_matches('/').to_string())
}

/// True for an Ollama cloud model, such as "gpt-oss:120b-cloud": Ollama runs it on its own
/// servers, so the page text would leave the computer.
pub fn is_cloud(model: &str) -> bool {
    let model = model.trim().to_lowercase();
    model.ends_with("-cloud") || model.ends_with(":cloud")
}

/// A client for the local model server at `url` (blank means `DEFAULT_URL`) with `model`.
pub fn client(url: &str, model: String) -> Result<ChatCompletions> {
    if is_cloud(&model) {
        return Err(SplitError::Ai(format!(
            "\"{}\" is an Ollama cloud model, which runs on Ollama's servers rather than this computer. Choose a model              that runs locally",
            model.trim()
        )));
    }
    let url = if url.trim().is_empty() { DEFAULT_URL } else { url };
    ChatCompletions::build(NAME.into(), check_url(url)?, true, String::new(), model, TIMEOUT)
}

/// The models the server at `url` runs on this computer, without its cloud models.
pub async fn models(url: &str) -> Result<Vec<String>> {
    let mut models = client(url, String::new())?.models().await?;
    models.retain(|m| !is_cloud(m));
    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::http::test_server::{body, serve};
    use crate::ai::{AiRequest, PageExcerpt, Provider};

    const OK: &str = r#"{"choices":[{"message":{"content":"[{\"start\":1,\"end\":2,\"title\":\"All\"}]"},"finish_reason":"stop"}]}"#;

    fn request() -> AiRequest {
        AiRequest {
            instruction: "Split by chapter".into(),
            pages: vec![PageExcerpt { page: 1, head: "Chapter 1".into(), tail: "end".into() }],
        }
    }

    #[test]
    fn only_addresses_on_this_computer_are_allowed() {
        assert_eq!(check_url(" http://localhost:11434/v1/ ").unwrap(), "http://localhost:11434/v1");
        assert_eq!(check_url("http://127.0.0.1:1234/v1").unwrap(), "http://127.0.0.1:1234/v1");
        assert_eq!(check_url("https://[::1]:8080/v1").unwrap(), "https://[::1]:8080/v1");
        for bad in ["", "localhost:11434", "ftp://localhost/v1", "http://192.168.1.5:11434/v1",
            "https://api.openai.com/v1", "http://localhost.evil.com/v1", "http://user:pw@localhost/v1",
            "http://localhost/v1?x=1"]
        {
            assert!(check_url(bad).is_err(), "{bad}");
        }
        let remote = check_url("https://llm.example.com/v1").unwrap_err().to_string();
        assert!(remote.contains("must be on this computer"), "{remote}");
    }

    #[test]
    fn a_blank_address_means_ollama() {
        assert!(client(" ", "llama3".into()).is_ok());
        assert!(client("https://llm.example.com/v1", "llama3".into()).is_err());
    }

    #[test]
    fn sends_the_prompt_without_a_key() {
        let (url, got) = serve("/v1/chat/completions", "200 OK", "", OK);
        let base = url.trim_end_matches("/chat/completions");
        let reply = tauri::async_runtime::block_on(client(base, "llama3".into()).unwrap().complete(&request())).unwrap();
        assert_eq!(reply, r#"[{"start":1,"end":2,"title":"All"}]"#);
        let raw = got.recv().unwrap();
        assert!(raw.starts_with("POST /v1/chat/completions "), "{raw}");
        assert!(!raw.to_lowercase().contains("authorization:"), "{raw}");
        assert_eq!(body(&raw)["model"], "llama3");
    }

    #[test]
    fn lists_local_models_without_a_key() {
        let list = r#"{"data":[{"id":"qwen3"},{"id":"llama3"},{"id":"gpt-oss:120b-cloud"}]}"#;
        let (url, got) = serve("/v1/models", "200 OK", "", list);
        let base = url.trim_end_matches("/models");
        let models = tauri::async_runtime::block_on(models(base)).unwrap();
        assert_eq!(models, ["llama3", "qwen3"]);
        assert!(!got.recv().unwrap().to_lowercase().contains("authorization:"));
    }

    #[test]
    fn refuses_ollama_cloud_models() {
        assert!(is_cloud("gpt-oss:120b-cloud") && is_cloud("Gemma4:31B-Cloud ") && is_cloud("kimi:cloud"));
        assert!(!is_cloud("llama3:8b") && !is_cloud("cloudy-model"));
        let err = client(DEFAULT_URL, "gemma4:31b-cloud".into()).err().unwrap().to_string();
        assert!(err.contains("Ollama cloud model"), "{err}");
    }

    #[test]
    #[ignore = "runs a whole Smart split on the Ollama server on this computer, with its first local model"]
    fn real_local_split() {
        use crate::strategy::{AiRun, AiStrategy, DocumentText};
        let model = tauri::async_runtime::block_on(models(DEFAULT_URL)).unwrap().into_iter().next().unwrap();
        let pages = ["Chapter 1
The start", "more text", "Chapter 2
The middle", "more text", "Chapter 3
The end"];
        let doc = DocumentText { page_count: 5, pages: pages.map(String::from).to_vec(), outline: Vec::new() };
        let strategy = AiStrategy { instruction: "Split by chapter".into() };
        let run = tauri::async_runtime::block_on(strategy.propose(&client(DEFAULT_URL, model).unwrap(), &doc, |_, _| {}));
        // A small model may answer badly; the split still completes, for the review screen (FR-36).
        assert!(matches!(run.unwrap(), AiRun::Parts { .. } | AiRun::Unusable { .. }));
    }

    #[test]
    fn a_stopped_server_gets_a_local_message() {
        // Nothing listens on port 9 on a test machine.
        let err = tauri::async_runtime::block_on(client("http://127.0.0.1:9/v1", "m".into()).unwrap().complete(&request()))
            .unwrap_err()
            .to_string();
        assert!(err.contains("on this computer") && err.contains("running"), "{err}");
    }

    #[test]
    fn does_not_follow_a_redirect_off_this_computer() {
        let (url, _got) = serve("/v1/chat/completions", "302 Found", "location: https://example.com/steal\r\n", "");
        let base = url.trim_end_matches("/chat/completions");
        let err = tauri::async_runtime::block_on(client(base, "m".into()).unwrap().complete(&request())).unwrap_err();
        assert!(err.to_string().contains("302"), "{err}");
    }
}
