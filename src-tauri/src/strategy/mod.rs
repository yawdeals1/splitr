//! Split strategies (SRS 3, NFR-14). Each strategy reads the extracted document and proposes ranges.
//! Adding a strategy means adding a module here, not changing the others.

mod ai;
mod blank;
mod fixed;
mod outline;
mod pattern;

#[allow(unused_imports)]
pub use ai::AiStrategy;
#[allow(unused_imports)]
pub use blank::BlankPageStrategy;
pub use fixed::FixedStrategy;
#[allow(unused_imports)]
pub use outline::OutlineStrategy;
#[allow(unused_imports)]
pub use pattern::PatternStrategy;

use serde::Serialize;

use crate::error::Result;
use crate::ranges::Range;

/// What the PDF engine extracts for the strategies to work on.
#[derive(Debug, Default)]
pub struct DocumentText {
    pub page_count: u32,
    /// Text of each page, index 0 is page 1.
    pub pages: Vec<String>,
    /// The bookmarks to split at: the top-level ones, or the chapters under a single one (#73).
    pub outline: Vec<OutlineEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OutlineEntry {
    pub title: String,
    pub page: u32,
}

pub trait Strategy {
    fn propose(&self, doc: &DocumentText) -> Result<Vec<Range>>;
}
