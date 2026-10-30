use serde::Serialize;

use super::DocumentText;
use crate::ai::{Provider, excerpts, response};
use crate::error::{Result, SplitError};
use crate::ranges::Range;

/// Splits with the AI (FR-33 to FR-36): short excerpts of each page in overlapping chunks (#14),
/// each sent to the provider, and the answers parsed, merged and validated (#16). Unlike the rule
/// strategies it waits on the network, so it is async and not a `Strategy`.
pub struct AiStrategy {
    /// The preset's prompt or the user's custom instruction.
    pub instruction: String,
}

/// What an AI split gave. The review screen opens for `Parts` and `Unusable` (FR-36).
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AiRun {
    /// Validated parts that cover every page (FR-35).
    Parts { ranges: Vec<Range> },
    /// The answer still failed after the retry. The user sets the parts by hand.
    Unusable { problem: String },
    /// The user pressed Cancel: nothing more is sent.
    Cancelled,
}

impl AiStrategy {
    /// Splits `doc` with `provider`, calling `progress(done, total)` as chunks are answered.
    /// Provider errors, such as a rejected key, are returned as errors.
    pub async fn propose<P: Provider>(&self, provider: &P, doc: &DocumentText, progress: impl FnMut(u32, u32)) -> Result<AiRun> {
        let instruction = self.instruction.trim();
        if instruction.is_empty() {
            return Err(SplitError::Ai("Type an instruction for the AI first".into()));
        }
        let chunks = excerpts::chunks(instruction, doc);
        // The AI is asked to cover every page (Appendix A), so gaps are a problem it must fix.
        match response::split_with_progress(provider, &chunks, doc.page_count, false, progress).await {
            Ok(ranges) => Ok(AiRun::Parts { ranges }),
            Err(e @ SplitError::AiReply(..)) => Ok(AiRun::Unusable { problem: e.to_string() }),
            Err(e) => Err(e),
        }
    }
}

/// A cancelled AI call or text read ends the run quietly instead of as an error.
pub fn cancelled_is_ok(result: Result<AiRun>) -> Result<AiRun> {
    match result {
        Err(SplitError::AiCancelled | SplitError::ExtractCancelled) => Ok(AiRun::Cancelled),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::ai::AiRequest;

    /// Answers each request with `answer` and keeps every request it was sent.
    struct Scripted<F: Fn(&AiRequest) -> Result<String>> {
        answer: F,
        sent: RefCell<Vec<AiRequest>>,
    }

    impl<F: Fn(&AiRequest) -> Result<String>> Provider for Scripted<F> {
        async fn complete(&self, request: &AiRequest) -> Result<String> {
            self.sent.borrow_mut().push(request.clone());
            (self.answer)(request)
        }
    }

    fn scripted<F: Fn(&AiRequest) -> Result<String>>(answer: F) -> Scripted<F> {
        Scripted { answer, sent: RefCell::new(Vec::new()) }
    }

    fn doc(pages: u32) -> DocumentText {
        DocumentText { page_count: pages, pages: (1..=pages).map(|n| format!("Page {n}")).collect(), outline: Vec::new() }
    }

    fn run<P: Provider>(instruction: &str, provider: &P, doc: &DocumentText) -> Result<AiRun> {
        let strategy = AiStrategy { instruction: instruction.into() };
        tauri::async_runtime::block_on(strategy.propose(provider, doc, |_, _| {}))
    }

    #[test]
    fn sends_the_instruction_with_excerpts_and_returns_the_parts() {
        let provider = scripted(|_| Ok(r#"[{"start":1,"end":2,"title":"A"},{"start":3,"end":3,"title":"B"}]"#.into()));
        let result = run("  Split by letter ", &provider, &doc(3)).unwrap();
        let ranges = vec![
            Range { start: 1, end: 2, title: "A".into() },
            Range { start: 3, end: 3, title: "B".into() },
        ];
        assert_eq!(result, AiRun::Parts { ranges });
        let sent = provider.sent.borrow();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].instruction, "Split by letter");
        assert_eq!(sent[0].pages.iter().map(|p| p.head.as_str()).collect::<Vec<_>>(), ["Page 1", "Page 2", "Page 3"]);
    }

    #[test]
    fn an_answer_that_fails_twice_is_unusable_not_an_error() {
        let provider = scripted(|_| Ok("I can't tell".into()));
        let AiRun::Unusable { problem } = run("Split", &provider, &doc(3)).unwrap() else { panic!() };
        assert!(problem.contains("pages 1 to 3 could not be used"), "{problem}");
        assert!(problem.ends_with("Set the parts by hand instead"), "{problem}");
        assert_eq!(provider.sent.borrow().len(), 2);
        // Leaving pages out counts as a bad answer too.
        let gap = scripted(|_| Ok(r#"[{"start":1,"end":2,"title":"A"}]"#.into()));
        assert!(matches!(run("Split", &gap, &doc(3)).unwrap(), AiRun::Unusable { .. }));
    }

    #[test]
    fn provider_errors_and_empty_instructions_are_errors() {
        let rejected = scripted(|_| Err(SplitError::Ai("Your key was rejected".into())));
        assert_eq!(run("Split", &rejected, &doc(3)).unwrap_err().to_string(), "Your key was rejected");
        let unused = scripted(|_| unreachable!());
        assert_eq!(run(" \n", &unused, &doc(3)).unwrap_err().to_string(), "Type an instruction for the AI first");
        assert!(unused.sent.borrow().is_empty());
    }

    #[test]
    fn cancelling_ends_the_run_quietly() {
        assert_eq!(cancelled_is_ok(Err(SplitError::AiCancelled)).unwrap(), AiRun::Cancelled);
        assert_eq!(cancelled_is_ok(Err(SplitError::ExtractCancelled)).unwrap(), AiRun::Cancelled);
        assert!(cancelled_is_ok(Err(SplitError::NoOutline)).is_err());
    }
}
