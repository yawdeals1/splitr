//! AI providers for Smart mode (SRS 3, 6.2, Appendix A). Every platform sits behind `Provider`, so
//! the rest of the app doesn't know which one answers. Only the AI strategy calls these (NFR-06).

pub mod claude;

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

use crate::error::{Result, SplitError};

/// The AI service Smart mode uses until the user picks another: their ChatGPT account (#31).
pub const DEFAULT_PLATFORM: &str = "chatgpt";

/// The model each service uses until the user sets another. The ChatGPT default is the model
/// OpenAI's Sign in with ChatGPT docs use; the OpenAI API matches it. The OpenAI-compatible
/// platforms keep theirs in #30's data file, and a custom platform has none.
const DEFAULT_MODELS: &[(&str, &str)] = &[
    ("chatgpt", "gpt-6.1-sol"),
    ("openai", "gpt-6.1-sol"),
    ("anthropic", "claude-opus-5-5"),
    ("gemini", "gemini-4-argon"),
];

/// The default model for `platform`, if it has one.
pub fn default_model(platform: &str) -> Option<&'static str> {
    DEFAULT_MODELS.iter().find(|(id, _)| *id == platform).map(|(_, model)| *model)
}

/// Every default, for the settings screen.
pub fn default_models() -> BTreeMap<String, String> {
    DEFAULT_MODELS.iter().map(|(id, model)| (id.to_string(), model.to_string())).collect()
}

/// Services Smart mode can be set to: the ChatGPT account, or a platform with a saved key.
pub fn is_platform(platform: &str) -> bool {
    platform == "chatgpt" || crate::keystore::is_platform(platform)
}

/// One page in a request: short excerpts, never the whole page (FR-33, FR-61). Built by #14.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageExcerpt {
    pub page: u32,
    pub head: String,
    pub tail: String,
}

/// One chunk of a request (SRS 6.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiRequest {
    pub instruction: String,
    pub pages: Vec<PageExcerpt>,
}

/// A platform that turns a request into the model's reply text. The reply is parsed and
/// validated by the caller (FR-35, #16).
pub trait Provider {
    async fn complete(&self, request: &AiRequest) -> Result<String>;
}

/// Lets the user stop the AI calls in progress (#15). Dropping a call's future also drops its
/// HTTP request, so nothing more is sent or waited for.
pub struct AiCalls {
    cancel: watch::Sender<u64>,
}

impl Default for AiCalls {
    fn default() -> Self {
        Self { cancel: watch::channel(0).0 }
    }
}

impl AiCalls {
    /// Runs `work` until it finishes or `cancel` is called.
    pub async fn run<T>(&self, work: impl Future<Output = Result<T>>) -> Result<T> {
        let mut cancelled = self.cancel.subscribe();
        tokio::select! {
            result = work => result,
            _ = cancelled.changed() => Err(SplitError::AiCancelled),
        }
    }

    /// Cancels every call running now. Later calls are not affected.
    pub fn cancel(&self) {
        self.cancel.send_modify(|n| *n = n.wrapping_add(1));
    }
}

/// The system prompt: the task, then the fixed rules (SRS Appendix A).
pub fn system_prompt(task: &str) -> String {
    format!(
        "You are given a list of pages from a PDF, each with a short excerpt.\n\
         Task: {}\n\
         Rules:\n\
         - Return only a JSON array, no other text.\n\
         - Each item has: start (int), end (int), title (string).\n\
         - Ranges must not overlap and must use the page numbers provided.\n\
         - Ranges must be in ascending order.\n\
         - Cover every page unless the task says to skip some.\n\
         - If you are unsure where a part begins, choose the most likely page.",
        task.trim()
    )
}

/// The user message: the pages as JSON in the SRS 6.2 shape.
pub fn user_message(request: &AiRequest) -> String {
    // Serializing plain strings and numbers can't fail.
    serde_json::to_string(request).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_holds_the_task_and_every_rule() {
        let prompt = system_prompt("  Split by chapter.\n");
        assert!(prompt.starts_with("You are given a list of pages"));
        assert!(prompt.contains("Task: Split by chapter.\nRules:"));
        for rule in ["only a JSON array", "start (int), end (int), title (string)", "must not overlap",
            "ascending order", "Cover every page", "most likely page"]
        {
            assert!(prompt.contains(rule), "missing rule: {rule}");
        }
    }

    #[test]
    fn chatgpt_is_the_default_and_every_default_is_a_known_platform() {
        assert_eq!(default_model(DEFAULT_PLATFORM), Some("gpt-6.1-sol"));
        assert_eq!(default_model("openai"), default_model("chatgpt"));
        assert_eq!(default_model("custom"), None);
        for (id, model) in DEFAULT_MODELS {
            assert!(is_platform(id), "{id}");
            assert!(!model.trim().is_empty());
        }
        assert!(!is_platform("../evil"));
    }

    #[test]
    fn cancel_stops_running_calls_only() {
        let calls = AiCalls::default();
        let (stopped, other) = tauri::async_runtime::block_on(async {
            tokio::join!(calls.run(std::future::pending::<Result<()>>()), async { calls.cancel() })
        });
        assert!(matches!(stopped, Err(SplitError::AiCancelled)));
        assert_eq!(other, ());
        // A call that starts after the cancel runs normally.
        let later = tauri::async_runtime::block_on(calls.run(async { Ok(7) }));
        assert_eq!(later.unwrap(), 7);
    }

    #[test]
    fn user_message_uses_the_srs_shape() {
        let request = AiRequest {
            instruction: "Split by invoice".into(),
            pages: vec![PageExcerpt { page: 1, head: "Invoice \"7\"".into(), tail: "Total".into() }],
        };
        let value: serde_json::Value = serde_json::from_str(&user_message(&request)).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "instruction": "Split by invoice",
                "pages": [{ "page": 1, "head": "Invoice \"7\"", "tail": "Total" }]
            })
        );
    }
}
