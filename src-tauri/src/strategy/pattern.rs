use regex::Regex;

use super::{DocumentText, Strategy};
use crate::error::{Result, SplitError};
use crate::ranges::{Range, from_starts};

/// Starts a new part on every page whose text matches the pattern (FR-31). A page that repeats the
/// identifier that started the part, such as an account number in every page's header, continues
/// it. A match with no digit is a keyword, not an identifier, so it always starts a part.
pub struct PatternStrategy {
    regex: Regex,
}

impl PatternStrategy {
    pub fn new(pattern: &str) -> Result<Self> {
        Ok(Self { regex: Regex::new(pattern)? })
    }
}

impl Strategy for PatternStrategy {
    fn propose(&self, doc: &DocumentText) -> Result<Vec<Range>> {
        let mut matches: Vec<(u32, String)> = Vec::new();
        for (i, text) in doc.pages.iter().enumerate() {
            let Some(found) = self.regex.find(text) else { continue };
            let title = found.as_str().trim();
            if !title.is_empty() && matches.last().is_some_and(|(_, last)| same_match(last, title)) {
                continue;
            }
            matches.push((i as u32 + 1, title.to_string()));
        }
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

/// True if two matches are the same identifier, ignoring letter case and how they were spaced.
fn same_match(a: &str, b: &str) -> bool {
    if !b.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    let words = |text: &str| text.split_whitespace().map(str::to_lowercase).collect::<Vec<_>>();
    words(a) == words(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_repeated_match_continues_the_part() {
        let pages = ["Account 111", "Account 111", "ACCOUNT  111", "no header", "Account 222", "Account 111"];
        let doc = DocumentText { page_count: 6, pages: pages.map(String::from).to_vec(), ..Default::default() };
        let r = PatternStrategy::new(r"(?i)account\s+\d+").unwrap().propose(&doc).unwrap();
        assert_eq!(r.iter().map(|r| (r.start, r.end)).collect::<Vec<_>>(), [(1, 4), (5, 5), (6, 6)]);
        assert_eq!(r[0].title, "Account 111");
    }

    #[test]
    fn a_repeated_keyword_still_starts_a_part() {
        let pages = ["Certificate", "Certificate", "other"];
        let doc = DocumentText { page_count: 3, pages: pages.map(String::from).to_vec(), ..Default::default() };
        let r = PatternStrategy::new("Certificate").unwrap().propose(&doc).unwrap();
        assert_eq!(r.iter().map(|r| (r.start, r.end)).collect::<Vec<_>>(), [(1, 1), (2, 3)]);
    }

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
