use serde::{Deserialize, Serialize};

use crate::error::{Result, SplitError};

/// One output file: an inclusive, 1-based page range (SRS 1.3, 6.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub start: u32,
    pub end: u32,
    #[serde(default)]
    pub title: String,
}

impl Range {
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end, title: String::new() }
    }
}

/// Parses Custom mode input such as "1-14, 15-30, 31" (FR-20).
pub fn parse_custom(input: &str, page_count: u32) -> Result<Vec<Range>> {
    let mut ranges = Vec::new();
    for part in input.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (start, end) = match part.split_once('-') {
            Some((a, b)) => (parse_page(a)?, parse_page(b)?),
            None => {
                let page = parse_page(part)?;
                (page, page)
            }
        };
        ranges.push(Range::new(start, end));
    }
    if ranges.is_empty() {
        return Err(SplitError::InvalidRanges("Enter at least one page range".into()));
    }
    validate(&ranges, page_count, true)?;
    Ok(ranges)
}

fn parse_page(s: &str) -> Result<u32> {
    s.trim()
        .parse()
        .map_err(|_| SplitError::InvalidRanges(format!("\"{}\" is not a page number", s.trim())))
}

/// Checks ranges are in bounds, ascending and non-overlapping, and cover every page
/// unless `allow_gaps` is set (FR-35).
pub fn validate(ranges: &[Range], page_count: u32, allow_gaps: bool) -> Result<()> {
    validate_within(ranges, 1, page_count, allow_gaps)
}

/// `validate` for pages `first` to `last` only, such as one AI chunk (FR-34, FR-35).
pub fn validate_within(ranges: &[Range], first: u32, last: u32, allow_gaps: bool) -> Result<()> {
    let mut next = first;
    for r in ranges {
        if r.start < first || r.end > last {
            return Err(SplitError::InvalidRanges(format!(
                "Range {}-{} is outside pages {first} to {last}",
                r.start, r.end
            )));
        }
        if r.start > r.end {
            return Err(SplitError::InvalidRanges(format!(
                "Range {}-{} ends before it starts",
                r.start, r.end
            )));
        }
        if r.start < next {
            return Err(SplitError::InvalidRanges(format!(
                "Range {}-{} overlaps or is out of order",
                r.start, r.end
            )));
        }
        if !allow_gaps && r.start != next {
            return Err(SplitError::InvalidRanges(format!(
                "Pages {next} to {} are not in any part",
                r.start - 1
            )));
        }
        next = r.end + 1;
    }
    if !allow_gaps && next != last + 1 {
        return Err(SplitError::InvalidRanges(format!(
            "Pages {next} to {last} are not in any part"
        )));
    }
    Ok(())
}

/// Turns sorted start pages into contiguous ranges that cover the whole document.
pub fn from_starts(starts: &[u32], page_count: u32) -> Vec<Range> {
    let mut starts: Vec<u32> = starts.iter().copied().filter(|&p| p >= 1 && p <= page_count).collect();
    starts.sort_unstable();
    starts.dedup();
    if starts.first() != Some(&1) {
        starts.insert(0, 1);
    }
    starts
        .iter()
        .enumerate()
        .map(|(i, &start)| {
            let end = starts.get(i + 1).map_or(page_count, |next| next - 1);
            Range::new(start, end)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_custom_ranges() {
        let r = parse_custom("1-14, 15-30, 31", 31).unwrap();
        assert_eq!(r, vec![Range::new(1, 14), Range::new(15, 30), Range::new(31, 31)]);
    }

    #[test]
    fn rejects_bad_custom_ranges() {
        assert!(parse_custom("", 10).is_err());
        assert!(parse_custom("a-3", 10).is_err());
        assert!(parse_custom("1-5, 4-8", 10).is_err());
        assert!(parse_custom("5-3", 10).is_err());
        assert!(parse_custom("1-11", 10).is_err());
        assert!(parse_custom("0-2", 10).is_err());
    }

    #[test]
    fn validate_requires_full_coverage_without_gaps() {
        let r = vec![Range::new(1, 4), Range::new(6, 10)];
        assert!(validate(&r, 10, true).is_ok());
        assert!(validate(&r, 10, false).is_err());
        assert!(validate(&[Range::new(1, 9)], 10, false).is_err());
    }

    #[test]
    fn validate_within_checks_only_its_pages() {
        let r = vec![Range::new(11, 15), Range::new(16, 20)];
        assert!(validate_within(&r, 11, 20, false).is_ok());
        let err = validate_within(&r, 11, 18, false).unwrap_err().to_string();
        assert_eq!(err, "Range 16-20 is outside pages 11 to 18");
        let err = validate_within(&r[1..], 11, 20, false).unwrap_err().to_string();
        assert_eq!(err, "Pages 11 to 15 are not in any part");
        assert!(validate_within(&r[1..], 11, 20, true).is_ok());
        assert!(validate_within(&[Range::new(10, 12)], 11, 20, true).is_err());
    }

    #[test]
    fn builds_ranges_from_starts() {
        assert_eq!(from_starts(&[5, 3], 8), vec![Range::new(1, 2), Range::new(3, 4), Range::new(5, 8)]);
        assert_eq!(from_starts(&[], 3), vec![Range::new(1, 3)]);
    }
}
