//! What the AI step sends (SRS 6.2): the first and last lines of each page, never the whole page
//! (FR-33, FR-61), in overlapping chunks so a large file needs only a few requests (FR-34, NFR-04).
//! `response::merge` joins the chunk answers where the chunks overlap.

// Smart mode calls this when it runs the AI step (#64).
#![allow(dead_code)]

use super::{AiRequest, PageExcerpt};
use crate::strategy::DocumentText;

/// Lines at the top of a page that go into `head`. Titles, headings and numbers that start a new
/// part are almost always here.
const HEAD_LINES: usize = 3;
/// Lines at the bottom of a page that go into `tail`: "continued", totals and signatures.
const TAIL_LINES: usize = 2;
/// The most characters of a page in `head` and in `tail`. Longer text is cut at a word.
pub const HEAD_CHARS: usize = 200;
pub const TAIL_CHARS: usize = 100;
/// Pages in one request. With up to 300 characters a page, a chunk stays near 10,000 tokens.
pub const CHUNK_PAGES: u32 = 120;
/// Pages each chunk shares with the next, so a part that starts near a chunk's edge is seen with
/// the pages around it. A 300-page file takes 3 requests (NFR-04).
pub const OVERLAP: u32 = 10;

/// The excerpt of one page: its first lines as `head` and its last lines as `tail`. Lines in
/// `head` are not repeated in `tail`, so a short page has an empty `tail`.
pub fn excerpt(page: u32, text: &str) -> PageExcerpt {
    let lines: Vec<String> = text
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect();
    let head_end = lines.len().min(HEAD_LINES);
    let tail_start = lines.len().saturating_sub(TAIL_LINES).max(head_end);
    PageExcerpt {
        page,
        head: shorten(&lines[..head_end].join(" / "), HEAD_CHARS, false),
        tail: shorten(&lines[tail_start..].join(" / "), TAIL_CHARS, true),
    }
}

/// At most `max` characters of `text`, cut at a word and marked with "…". `from_end` keeps the
/// end of the text instead of its start.
fn shorten(text: &str, max: usize, from_end: bool) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.to_string();
    }
    // One character is the "…".
    let keep = max - 1;
    if from_end {
        let cut: String = text.chars().skip(count - keep).collect();
        // Drop the partial word at the start, unless the whole tail is one word.
        let cut = cut.split_once(' ').map_or(cut.as_str(), |(_, rest)| rest).to_string();
        format!("…{cut}")
    } else {
        let cut: String = text.chars().take(keep).collect();
        let cut = cut.rsplit_once(' ').map_or(cut.as_str(), |(rest, _)| rest).to_string();
        format!("{cut}…")
    }
}

/// Requests for every page of `doc`, `CHUNK_PAGES` pages each, overlapping by `OVERLAP` pages.
/// Every request carries the same `instruction`, the preset or custom prompt.
pub fn chunks(instruction: &str, doc: &DocumentText) -> Vec<AiRequest> {
    page_spans(doc.page_count, CHUNK_PAGES, OVERLAP)
        .into_iter()
        .map(|(first, last)| AiRequest {
            instruction: instruction.to_string(),
            pages: (first..=last)
                .map(|page| excerpt(page, doc.pages.get(page as usize - 1).map_or("", String::as_str)))
                .collect(),
        })
        .collect()
}

/// The first and last page of each chunk. The last chunk ends at `page_count`; each chunk after
/// the first starts `overlap` pages before the previous one ends.
fn page_spans(page_count: u32, size: u32, overlap: u32) -> Vec<(u32, u32)> {
    let mut spans = Vec::new();
    let mut first = 1;
    while first <= page_count {
        let last = (first + size - 1).min(page_count);
        spans.push((first, last));
        if last == page_count {
            break;
        }
        first = last + 1 - overlap;
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn head_and_tail_are_the_first_and_last_lines() {
        let text = "  Chapter 3 \n\nThe   Sea\nline a\nline b\nline c\nPage 41\n";
        let e = excerpt(41, text);
        assert_eq!(e, PageExcerpt { page: 41, head: "Chapter 3 / The Sea / line a".into(), tail: "line c / Page 41".into() });
    }

    #[test]
    fn short_and_empty_pages() {
        assert_eq!(excerpt(1, "Preface\n").head, "Preface");
        assert_eq!(excerpt(1, "Preface\n").tail, "");
        // Lines in head are not repeated in tail.
        let e = excerpt(2, "a\nb\nc\nd");
        assert_eq!((e.head.as_str(), e.tail.as_str()), ("a / b / c", "d"));
        let blank = excerpt(3, " \n\t\n");
        assert_eq!((blank.head.as_str(), blank.tail.as_str()), ("", ""));
    }

    #[test]
    fn excerpts_stay_within_their_limits_and_leave_out_the_middle() {
        let word = |n: usize| format!("word{n}");
        let long_line = (0..200).map(word).collect::<Vec<_>>().join(" ");
        let middle = "THE MIDDLE OF THE PAGE";
        let text = format!("{long_line}\n{middle}\n{}\n{long_line}", "x\n".repeat(50));
        let e = excerpt(1, &text);
        assert!(e.head.chars().count() <= HEAD_CHARS, "{}", e.head);
        assert!(e.tail.chars().count() <= TAIL_CHARS, "{}", e.tail);
        assert!(e.head.starts_with("word0 word1") && e.head.ends_with('…'), "{}", e.head);
        assert!(e.tail.starts_with('…') && e.tail.ends_with("word199"), "{}", e.tail);
        // Cut at words: no partial word next to the "…".
        assert!(e.head.trim_end_matches('…').split(' ').all(|w| w.starts_with("word")));
        assert!(e.tail.trim_start_matches('…').split(' ').all(|w| w.starts_with("word")));
        assert!(!e.head.contains(middle) && !e.tail.contains(middle));
    }

    #[test]
    fn limits_count_characters_not_bytes() {
        let text = "é".repeat(500);
        let e = excerpt(1, &text);
        assert_eq!(e.head.chars().count(), HEAD_CHARS);
        assert_eq!(e.tail, "");
        let two = format!("title\n{}", "ü".repeat(500));
        assert!(excerpt(1, &two).head.chars().count() <= HEAD_CHARS);
    }

    #[test]
    fn chunks_overlap_and_cover_every_page() {
        assert_eq!(page_spans(0, 120, 10), []);
        assert_eq!(page_spans(1, 120, 10), [(1, 1)]);
        assert_eq!(page_spans(120, 120, 10), [(1, 120)]);
        assert_eq!(page_spans(121, 120, 10), [(1, 120), (111, 121)]);
        assert_eq!(page_spans(300, 120, 10), [(1, 120), (111, 230), (221, 300)]);
        for pages in [1, 50, 119, 120, 121, 300, 1000, 1234] {
            let spans = page_spans(pages, CHUNK_PAGES, OVERLAP);
            assert_eq!(spans[0].0, 1);
            assert_eq!(spans.last().unwrap().1, pages);
            for pair in spans.windows(2) {
                assert_eq!(pair[0].1 - pair[1].0 + 1, OVERLAP, "{pages} pages: {spans:?}");
            }
            assert!(spans.iter().all(|(first, last)| last - first < CHUNK_PAGES));
        }
    }

    #[test]
    fn a_300_page_file_takes_a_few_small_requests() {
        let doc = DocumentText {
            page_count: 300,
            pages: (1..=300).map(|n| format!("Page {n} heading\n{}\nPage {n} footer", "body text ".repeat(400))).collect(),
            outline: Vec::new(),
        };
        let requests = chunks("Split by chapter", &doc);
        assert_eq!(requests.len(), 3);
        let pages: Vec<u32> = requests.iter().flat_map(|r| r.pages.iter().map(|p| p.page)).collect();
        assert_eq!(pages.len(), 300 + 2 * OVERLAP as usize);
        assert_eq!(requests[1].pages[0].page, 111);
        assert!(requests.iter().all(|r| r.instruction == "Split by chapter"));
        assert!(requests[2].pages.last().unwrap().head.starts_with("Page 300 heading / body text"));
        for request in &requests {
            let size = crate::ai::user_message(request).len();
            assert!(size < 50_000, "a request is {size} bytes");
        }
    }

    #[test]
    fn pages_missing_from_the_text_send_empty_excerpts() {
        let doc = DocumentText { page_count: 2, pages: vec!["Only page one".into()], outline: Vec::new() };
        let pages = &chunks("x", &doc)[0].pages;
        assert_eq!(pages[1], PageExcerpt { page: 2, head: String::new(), tail: String::new() });
    }
}
