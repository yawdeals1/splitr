use super::{DocumentText, Strategy};
use crate::error::{Result, SplitError};
use crate::ranges::{Range, from_starts};

/// Fixed mode: every N pages, or N equal parts (FR-21).
pub enum FixedStrategy {
    EveryN(u32),
    /// Exactly N parts whose sizes differ by at most one page; the longer parts come first.
    Parts(u32),
}

impl Strategy for FixedStrategy {
    fn propose(&self, doc: &DocumentText) -> Result<Vec<Range>> {
        let pages = doc.page_count;
        let starts: Vec<u32> = match *self {
            FixedStrategy::EveryN(n) if n > 0 => (1..=pages).step_by(n as usize).collect(),
            FixedStrategy::Parts(n) if n > 0 && n <= pages => {
                let (size, longer) = (pages / n, pages % n);
                (0..n).map(|i| 1 + i * size + i.min(longer)).collect()
            }
            _ => {
                return Err(SplitError::InvalidRanges(
                    "Choose a number between 1 and the page count".into(),
                ));
            }
        };
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
    fn equal_parts_always_gives_n_parts_within_a_page_of_each_other() {
        // A fixed part size of ceil(10 / 6) = 2 would give only 5 parts.
        let r = FixedStrategy::Parts(6).propose(&doc(10)).unwrap();
        assert_eq!(r.iter().map(|r| r.end - r.start + 1).collect::<Vec<_>>(), [2, 2, 2, 2, 1, 1]);
        for pages in 1..=40 {
            for n in 1..=pages {
                let r = FixedStrategy::Parts(n).propose(&doc(pages)).unwrap();
                let sizes: Vec<u32> = r.iter().map(|r| r.end - r.start + 1).collect();
                assert_eq!(sizes.len(), n as usize, "{pages} pages into {n}");
                assert_eq!(sizes.iter().sum::<u32>(), pages);
                assert!(sizes.iter().max().unwrap() - sizes.iter().min().unwrap() <= 1, "{pages}/{n}: {sizes:?}");
            }
        }
    }

    #[test]
    fn rejects_out_of_range_counts() {
        assert!(FixedStrategy::EveryN(0).propose(&doc(5)).is_err());
        assert!(FixedStrategy::Parts(6).propose(&doc(5)).is_err());
    }
}
