//! Works out which AI platform an API key belongs to from its prefix (#27).
//!
//! Detection is local: no network call, and the key is never tried against several platforms
//! (NFR-05). Platforms and prefixes are data in `RULES`; adding one needs no logic change.

use serde::Serialize;

/// AI platforms a key can belong to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Anthropic,
    OpenAi,
    Gemini,
    OpenRouter,
    Nvidia,
    Groq,
    Xai,
    DeepSeek,
    Mistral,
    Together,
    #[serde(rename = "opencode-go")]
    OpencodeGo,
    Custom,
}

/// What a key's prefix says about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Detection {
    /// The prefix names one platform. The user can still correct it before saving.
    Platform { platform: Platform },
    /// The prefix fits several platforms, so the user picks one.
    Ambiguous { options: &'static [Platform] },
    /// A key of the wrong kind, with the reason to show.
    Rejected { reason: &'static str },
}

enum Outcome {
    Platform(Platform),
    Ambiguous(&'static [Platform]),
    Rejected(&'static str),
}

struct Rule {
    prefix: &'static str,
    /// Total key length when the platform fixes it.
    len: Option<usize>,
    outcome: Outcome,
}

const fn rule(prefix: &'static str, outcome: Outcome) -> Rule {
    Rule { prefix, len: None, outcome }
}

/// Checked in order, so more specific prefixes come first.
const RULES: &[Rule] = &[
    rule("sk-ant-oat", Outcome::Rejected(
        "This is a Claude subscription token, not an API key. Create an API key in the Claude Console.",
    )),
    rule("sk-ant-admin", Outcome::Rejected(
        "This is an Anthropic Admin key, which can't run AI requests. Use a standard API key.",
    )),
    // Any other Anthropic key goes to Anthropic, never to the "sk-" guess below (NFR-05).
    rule("sk-ant-", Outcome::Platform(Platform::Anthropic)),
    rule("sk-or-v1-", Outcome::Platform(Platform::OpenRouter)),
    rule("sk-admin-", Outcome::Rejected(
        "This is an OpenAI Admin key, which can't run AI requests. Use a project API key.",
    )),
    rule("sk-proj-", Outcome::Platform(Platform::OpenAi)),
    rule("sk-svcacct-", Outcome::Platform(Platform::OpenAi)),
    Rule { prefix: "AIza", len: Some(39), outcome: Outcome::Platform(Platform::Gemini) },
    rule("nvapi-", Outcome::Platform(Platform::Nvidia)),
    rule("gsk_", Outcome::Platform(Platform::Groq)),
    rule("xai-", Outcome::Platform(Platform::Xai)),
    rule("sk-", Outcome::Ambiguous(&[Platform::OpenAi, Platform::DeepSeek])),
];

/// Platforms to offer when no prefix matches.
const UNKNOWN: &[Platform] = &[Platform::OpencodeGo, Platform::Mistral, Platform::Together, Platform::Custom];

/// Detects the platform for `key`. Surrounding whitespace is ignored.
pub fn detect(key: &str) -> Detection {
    let key = key.trim();
    if key.is_empty() {
        return Detection::Rejected { reason: "Paste an API key first." };
    }
    if key.chars().any(char::is_whitespace) {
        return Detection::Rejected { reason: "An API key has no spaces. Check that you copied only the key." };
    }
    let matched = RULES
        .iter()
        .find(|r| key.starts_with(r.prefix) && r.len.is_none_or(|len| key.len() == len));
    match matched.map(|r| &r.outcome) {
        Some(Outcome::Platform(platform)) => Detection::Platform { platform: *platform },
        Some(Outcome::Ambiguous(options)) => Detection::Ambiguous { options },
        Some(Outcome::Rejected(reason)) => Detection::Rejected { reason },
        None => Detection::Ambiguous { options: UNKNOWN },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn platform(key: &str) -> Option<Platform> {
        match detect(key) {
            Detection::Platform { platform } => Some(platform),
            _ => None,
        }
    }

    fn rejected(key: &str) -> bool {
        matches!(detect(key), Detection::Rejected { .. })
    }

    #[test]
    fn detects_each_platform() {
        assert_eq!(platform("sk-ant-api03-abcDEF123"), Some(Platform::Anthropic));
        // A newer Anthropic key format is never offered to OpenAI or DeepSeek.
        assert_eq!(platform("sk-ant-new01-abcDEF123"), Some(Platform::Anthropic));
        assert_eq!(platform("sk-or-v1-0123456789abcdef"), Some(Platform::OpenRouter));
        assert_eq!(platform("sk-proj-abc123"), Some(Platform::OpenAi));
        assert_eq!(platform("sk-svcacct-abc123"), Some(Platform::OpenAi));
        assert_eq!(platform(&format!("AIza{}", "x".repeat(35))), Some(Platform::Gemini));
        assert_eq!(platform("nvapi-abc123"), Some(Platform::Nvidia));
        assert_eq!(platform("gsk_abc123"), Some(Platform::Groq));
        assert_eq!(platform("xai-abc123"), Some(Platform::Xai));
    }

    #[test]
    fn plain_sk_is_ambiguous() {
        assert_eq!(detect("sk-0123456789abcdef"), Detection::Ambiguous { options: &[Platform::OpenAi, Platform::DeepSeek] });
    }

    #[test]
    fn unknown_prefix_asks_the_user() {
        assert_eq!(detect("abc123def456"), Detection::Ambiguous { options: UNKNOWN });
        // A Gemini-looking key of the wrong length isn't trusted as Gemini.
        assert_eq!(detect("AIzaShort"), Detection::Ambiguous { options: UNKNOWN });
    }

    #[test]
    fn rejects_wrong_kinds_of_key() {
        assert!(rejected("sk-admin-abc123"));
        assert!(rejected("sk-ant-oat01-abc123"));
        assert!(rejected("sk-ant-admin01-abc123"));
        assert!(rejected(""));
        assert!(rejected("   "));
        assert!(rejected("sk-proj-abc 123"));
    }

    #[test]
    fn specific_prefixes_win_over_general_ones() {
        // Every prefix that starts with another prefix must come before it.
        for (i, later) in RULES.iter().enumerate() {
            for earlier in &RULES[..i] {
                assert!(
                    !(later.prefix.starts_with(earlier.prefix) && later.prefix != earlier.prefix),
                    "{} is shadowed by {}",
                    later.prefix,
                    earlier.prefix
                );
            }
        }
    }

    #[test]
    fn trims_whitespace() {
        assert_eq!(platform("  gsk_abc123\n"), Some(Platform::Groq));
    }

    #[test]
    fn serializes_for_the_interface() {
        let json = serde_json::to_string(&detect("sk-proj-x")).unwrap();
        assert_eq!(json, r#"{"kind":"platform","platform":"openai"}"#);
        let json = serde_json::to_string(&detect("sk-x")).unwrap();
        assert_eq!(json, r#"{"kind":"ambiguous","options":["openai","deepseek"]}"#);
    }
}
