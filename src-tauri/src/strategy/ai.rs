use super::{DocumentText, Strategy};
use crate::error::{Result, SplitError};
use crate::ranges::Range;

/// Sends per-page excerpts to the AI provider and parses the returned ranges (FR-33 to FR-36).
/// Built in Phase 2, see SRS 6.2, 6.3 and Appendix A.
#[allow(dead_code)]
pub struct AiStrategy {
    pub instruction: String,
}

impl Strategy for AiStrategy {
    fn propose(&self, _doc: &DocumentText) -> Result<Vec<Range>> {
        Err(SplitError::NotImplemented("Smart (AI) split"))
    }
}
