//! Smart mode across several chunks with a scripted AI provider (#18, SRS 9.2): the real excerpt
//! builder (#14) chunks a long document, `ai::mock::Scripted` answers each chunk from its excerpts
//! alone, and the answers are retried, merged and validated (#16). No network and no key.
//!
//! `fixtures::ai_presets_split_and_export_with_a_mocked_provider` covers a sample PDF from opening
//! to export in one chunk; these tests cover what only shows with several chunks.

use crate::ai::excerpts::{CHUNK_PAGES, HEAD_CHARS, OVERLAP, TAIL_CHARS};
use crate::ai::mock::Scripted;
use crate::ai::AiRequest;
use crate::error::Result;
use crate::ranges::Range;
use crate::strategy::{AiRun, AiStrategy, DocumentText};

const PAGES: u32 = 300;
/// Where each chapter starts. 115 and 225 fall where two chunks overlap; 100 is just before one.
const CHAPTERS: [u32; 5] = [1, 100, 115, 225, 260];

/// A 300-page book with long pages. The middle of each page holds a marker that must never be sent.
fn book() -> DocumentText {
    let body = "Lorem ipsum dolor sit amet, consectetur adipiscing elit. ".repeat(20);
    let pages = (1..=PAGES)
        .map(|page| {
            let heading = if CHAPTERS.contains(&page) { format!("Chapter {page}") } else { format!("Text {page}") };
            format!("{heading}\n{body}\nPrivate middle of page {page}\n{body}\n{body}\nFooter {page}")
        })
        .collect();
    DocumentText { page_count: PAGES, pages, outline: Vec::new() }
}

/// Answers like a model reading only the excerpts: a part starts at each "Chapter" heading. Pages
/// before the first heading in a chunk continue the part from the chunk before.
fn reads_chapters(request: &AiRequest) -> String {
    let mut parts: Vec<serde_json::Value> = Vec::new();
    for page in &request.pages {
        if page.head.starts_with("Chapter") || parts.is_empty() {
            let title = page.head.split(" / ").next().unwrap_or("");
            parts.push(serde_json::json!({ "start": page.page, "end": page.page, "title": title }));
        } else {
            parts.last_mut().unwrap()["end"] = page.page.into();
        }
    }
    serde_json::Value::from(parts).to_string()
}

fn run(provider: &Scripted<impl Fn(&AiRequest, usize) -> Result<String>>) -> AiRun {
    let strategy = AiStrategy { instruction: "Split so that each chapter is one part".into() };
    tauri::async_runtime::block_on(strategy.propose(provider, &book(), |_, _| {})).unwrap()
}

fn chapters() -> Vec<Range> {
    let ends = CHAPTERS.iter().skip(1).map(|start| start - 1).chain([PAGES]);
    CHAPTERS.iter().zip(ends).map(|(&start, end)| Range { start, end, title: format!("Chapter {start}") }).collect()
}

fn span(request: &AiRequest) -> (u32, u32) {
    (request.pages.first().unwrap().page, request.pages.last().unwrap().page)
}

#[test]
fn a_long_book_is_sent_in_overlapping_chunks_and_merged_into_chapters() {
    let provider = Scripted::new(|request: &AiRequest, _| Ok(reads_chapters(request)));
    assert_eq!(run(&provider), AiRun::Parts { ranges: chapters() });

    // FR-34, NFR-04: three requests that overlap by OVERLAP pages and cover every page once each.
    let sent = provider.requests();
    let spans: Vec<(u32, u32)> = sent.iter().map(span).collect();
    assert_eq!(spans, [(1, CHUNK_PAGES), (CHUNK_PAGES - OVERLAP + 1, 2 * CHUNK_PAGES - OVERLAP), (221, PAGES)]);
    for request in &sent {
        let pages: Vec<u32> = request.pages.iter().map(|p| p.page).collect();
        assert_eq!(pages, (span(request).0..=span(request).1).collect::<Vec<_>>());
    }

    // FR-33, FR-61: only short excerpts leave the machine, never the middle of a page.
    for page in sent.iter().flat_map(|r| &r.pages) {
        assert!(page.head.chars().count() <= HEAD_CHARS, "page {}: {}", page.page, page.head);
        assert!(page.tail.chars().count() <= TAIL_CHARS, "page {}: {}", page.page, page.tail);
        assert!(page.tail.ends_with(&format!("Footer {}", page.page)), "page {}: {}", page.page, page.tail);
        assert!(!page.head.contains("Private") && !page.tail.contains("Private"), "page {}", page.page);
    }
}

#[test]
fn only_the_chunk_with_a_bad_answer_is_asked_again() {
    // FR-36: the second chunk's first answer isn't JSON. It alone is sent again, once.
    let provider = Scripted::new(|request: &AiRequest, n| {
        Ok(if n == 1 { "Sure! Here are the chapters:".into() } else { reads_chapters(request) })
    });
    assert_eq!(run(&provider), AiRun::Parts { ranges: chapters() });
    let spans: Vec<(u32, u32)> = provider.requests().iter().map(span).collect();
    assert_eq!(spans.len(), 4);
    assert_eq!(spans[1], spans[2], "the retry repeats the same chunk");
}

#[test]
fn a_chunk_that_fails_twice_leaves_the_parts_to_the_user() {
    // FR-36: the third chunk claims pages outside itself twice, so the review screen opens with
    // the problem instead of parts.
    let provider = Scripted::new(|request: &AiRequest, _| {
        Ok(if span(request).0 == 221 { r#"[{"start":1,"end":300,"title":"All"}]"#.into() } else { reads_chapters(request) })
    });
    let AiRun::Unusable { problem } = run(&provider) else { panic!("expected the answer to be unusable") };
    assert!(problem.contains("pages 221 to 300"), "{problem}");
    assert_eq!(provider.requests().len(), 4);
}
