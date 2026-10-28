use super::{DocumentText, Strategy};
use crate::error::{Result, SplitError};
use crate::ranges::{Range, from_starts};

/// One part per top-level bookmark (FR-30). The caller falls back to the preset
/// `fallback` strategy when this returns `NoOutline`.
pub struct OutlineStrategy;

impl Strategy for OutlineStrategy {
    fn propose(&self, doc: &DocumentText) -> Result<Vec<Range>> {
        if doc.outline.is_empty() {
            return Err(SplitError::NoOutline);
        }
        let starts: Vec<u32> = doc.outline.iter().map(|e| e.page).collect();
        let mut ranges = from_starts(&starts, doc.page_count);
        for range in &mut ranges {
            if let Some(entry) = doc.outline.iter().find(|e| e.page == range.start) {
                range.title = entry.title.clone();
            }
        }
        Ok(ranges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategy::OutlineEntry;

    #[test]
    fn one_part_per_bookmark() {
        let doc = DocumentText {
            page_count: 20,
            outline: vec![
                OutlineEntry { title: "Chapter 1".into(), page: 3 },
                OutlineEntry { title: "Chapter 2".into(), page: 11 },
            ],
            ..Default::default()
        };
        let r = OutlineStrategy.propose(&doc).unwrap();
        assert_eq!(r.len(), 3);
        assert_eq!((r[1].start, r[1].end, r[1].title.as_str()), (3, 10, "Chapter 1"));
    }

    #[test]
    fn errors_without_bookmarks() {
        let doc = DocumentText { page_count: 5, ..Default::default() };
        assert!(OutlineStrategy.propose(&doc).is_err());
    }
}
