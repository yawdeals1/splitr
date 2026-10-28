use super::{DocumentText, Strategy};
use crate::error::{Result, SplitError};
use crate::ranges::Range;

/// Treats near-empty pages as separators (FR-32).
///
/// For now a page is blank when its text has at most `max_chars` visible characters.
/// Scanned pages have no text layer, so pixel-based detection replaces this once the
/// PDF engine can render pages.
pub struct BlankPageStrategy {
    pub max_chars: usize,
    pub drop_separators: bool,
}

impl Strategy for BlankPageStrategy {
    fn propose(&self, doc: &DocumentText) -> Result<Vec<Range>> {
        let is_blank =
            |text: &String| text.chars().filter(|c| !c.is_whitespace()).count() <= self.max_chars;
        let mut ranges = Vec::new();
        let mut current: Option<u32> = None;
        let mut found = false;
        for page in 1..=doc.page_count {
            let blank = doc.pages.get(page as usize - 1).is_none_or(is_blank);
            if blank {
                found = true;
                if let Some(start) = current.take() {
                    ranges.push(Range::new(start, page - 1));
                }
                if !self.drop_separators {
                    ranges.push(Range::new(page, page));
                }
            } else if current.is_none() {
                current = Some(page);
            }
        }
        if let Some(start) = current {
            ranges.push(Range::new(start, doc.page_count));
        }
        if !found {
            return Err(SplitError::NothingFound);
        }
        Ok(ranges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_at_blank_pages() {
        let doc = DocumentText {
            page_count: 5,
            pages: vec!["a".into(), "b".into(), "  ".into(), "c".into(), "d".into()],
            ..Default::default()
        };
        let s = BlankPageStrategy { max_chars: 0, drop_separators: true };
        assert_eq!(s.propose(&doc).unwrap(), vec![Range::new(1, 2), Range::new(4, 5)]);
    }
}
