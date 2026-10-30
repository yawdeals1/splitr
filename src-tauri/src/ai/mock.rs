//! A scripted AI provider for tests (#18, SRS 9.2). It answers from a script and records every
//! request, so the Smart flow can be tested end to end with no network and no key.

use std::cell::RefCell;

use super::{AiRequest, Provider};
use crate::error::Result;

/// Answers each request with `script(request, n)`, where `n` counts calls from 0.
pub struct Scripted<F: Fn(&AiRequest, usize) -> Result<String>> {
    script: F,
    seen: RefCell<Vec<AiRequest>>,
}

impl<F: Fn(&AiRequest, usize) -> Result<String>> Scripted<F> {
    pub fn new(script: F) -> Self {
        Self { script, seen: RefCell::new(Vec::new()) }
    }

    /// Every request sent so far, in order.
    pub fn requests(&self) -> Vec<AiRequest> {
        self.seen.borrow().clone()
    }
}

impl<F: Fn(&AiRequest, usize) -> Result<String>> Provider for Scripted<F> {
    async fn complete(&self, request: &AiRequest) -> Result<String> {
        let n = self.seen.borrow().len();
        self.seen.borrow_mut().push(request.clone());
        (self.script)(request, n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::PageExcerpt;

    #[test]
    fn answers_from_the_script_and_records_requests() {
        let mock = Scripted::new(|request: &AiRequest, n| Ok(format!("{n}:{}", request.instruction)));
        let request = AiRequest { instruction: "x".into(), pages: vec![PageExcerpt { page: 1, head: "a".into(), tail: "".into() }] };
        let first = tauri::async_runtime::block_on(mock.complete(&request)).unwrap();
        let second = tauri::async_runtime::block_on(mock.complete(&request)).unwrap();
        assert_eq!((first.as_str(), second.as_str()), ("0:x", "1:x"));
        assert_eq!(mock.requests().len(), 2);
    }
}
