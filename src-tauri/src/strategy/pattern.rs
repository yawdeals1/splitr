use regex::Regex;

use super::{DocumentText, Strategy};
use crate::error::{Result, SplitError};
use crate::ranges::{Range, from_starts};

/// Starts a new part on every page whose text matches the pattern (FR-31).
pub struct PatternStrategy {
    regex: Regex,
}

impl PatternStrategy {
    #[allow(dead_code)]
    pub fn new(pattern: &str) -> Result<Self> {
        Ok(Self { regex: Regex::new(pattern)? })
    }
}

impl Strategy for PatternStrategy {
    fn propose(&self, doc: &DocumentText) -> Result<Vec<Range>> {
        let matches: Vec<(u32, String)> = doc
            .pages
            .iter()
            .enumerate()
            .filter_map(|(i, text)| {
                self.regex.find(text).map(|m| (i as u32 + 1, m.as_str().trim().to_string()))
            })
            .collect();
        if matches.is_empty() {
            return Err(SplitError::NothingFound);
        }
        let starts: Vec<u32> = matches.iter().map(|(page, _)| *page).collect();
        let mut ranges = from_starts(&starts, doc.page_count);
        for range in &mut ranges {
            if let Some((_, title)) = matches.iter().find(|(page, _)| *page == range.start) {
                range.title = title.clone();
            }
        }
        Ok(ranges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_matching_pages() {
        let doc = DocumentText {
            page_count: 4,
            pages: vec!["Invoice #001".into(), "items".into(), "Invoice #002".into(), "items".into()],
            ..Default::default()
        };
        let r = PatternStrategy::new(r"Invoice #\d+").unwrap().propose(&doc).unwrap();
        assert_eq!(r.len(), 2);
        assert_eq!((r[1].start, r[1].end, r[1].title.as_str()), (3, 4, "Invoice #002"));
    }
}
