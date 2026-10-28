use super::{DocumentText, Strategy};
use crate::error::{Result, SplitError};
use crate::ranges::{Range, from_starts};

/// Fixed mode: every N pages, or N equal parts (FR-21).
pub enum FixedStrategy {
    EveryN(u32),
    Parts(u32),
}

impl Strategy for FixedStrategy {
    fn propose(&self, doc: &DocumentText) -> Result<Vec<Range>> {
        let pages = doc.page_count;
        let size = match *self {
            FixedStrategy::EveryN(n) if n > 0 => n,
            FixedStrategy::Parts(n) if n > 0 && n <= pages => pages.div_ceil(n),
            _ => {
                return Err(SplitError::InvalidRanges(
                    "Choose a number between 1 and the page count".into(),
                ));
            }
        };
        let starts: Vec<u32> = (1..=pages).step_by(size as usize).collect();
        Ok(from_starts(&starts, pages))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(pages: u32) -> DocumentText {
        DocumentText { page_count: pages, ..Default::default() }
    }

    #[test]
    fn every_n_pages() {
        let r = FixedStrategy::EveryN(4).propose(&doc(10)).unwrap();
        assert_eq!(r, vec![Range::new(1, 4), Range::new(5, 8), Range::new(9, 10)]);
    }

    #[test]
    fn equal_parts() {
        let r = FixedStrategy::Parts(3).propose(&doc(9)).unwrap();
        assert_eq!(r, vec![Range::new(1, 3), Range::new(4, 6), Range::new(7, 9)]);
    }

    #[test]
    fn rejects_out_of_range_counts() {
        assert!(FixedStrategy::EveryN(0).propose(&doc(5)).is_err());
        assert!(FixedStrategy::Parts(6).propose(&doc(5)).is_err());
    }
}
