//! The Smart flow end to end with a scripted AI provider (#18, SRS 9.2): open a PDF, send page
//! excerpts in chunks, check and merge the answers (retrying a bad one once), and export the parts.
//! No network and no key: the provider is `ai::mock::Scripted`.
//!
//! Page text extraction (#7) and excerpt building (#14) aren't in yet, so the excerpts here are
//! built from the text the test PDF is known to have on each page ("Page n").

use std::fs;

use crate::ai::mock::Scripted;
use crate::ai::{AiRequest, PageExcerpt, response};
use crate::error::SplitError;
use crate::export::{self, ExportRequest};
use crate::naming;
use crate::pdf::tests::{document, save, start_engine};
use crate::pdf::{self};
use crate::ranges::Range;

const TASK: &str = "Split so that each chapter is one part";

/// Excerpts for pages `first` to `last`, as #14 will send them: the page number and short text.
fn chunk(first: u32, last: u32) -> AiRequest {
    let pages = (first..=last)
        .map(|page| PageExcerpt { page, head: format!("Page {page}"), tail: String::new() })
        .collect();
    AiRequest { instruction: TASK.into(), pages }
}

/// A reply in the SRS 6.3 shape.
fn reply(parts: &[(u32, u32, &str)]) -> String {
    let items: Vec<String> = parts
        .iter()
        .map(|(start, end, title)| format!(r#"{{"start":{start},"end":{end},"title":"{title}"}}"#))
        .collect();
    format!("[{}]", items.join(","))
}

/// The parts a careful model finds in pages `first` to `last` of a 10-page book whose chapters
/// start on pages 1, 4 and 8.
fn chapters_in(first: u32, last: u32) -> String {
    let chapters = [(1, 3, "Chapter 1"), (4, 7, "Chapter 2"), (8, 10, "Chapter 3")];
    let parts: Vec<(u32, u32, &str)> = chapters
        .iter()
        .filter(|(start, end, _)| *end >= first && *start <= last)
        .map(|(start, end, title)| ((*start).max(first), (*end).min(last), *title))
        .collect();
    reply(&parts)
}

fn first_and_last(request: &AiRequest) -> (u32, u32) {
    let pages: Vec<u32> = request.pages.iter().map(|p| p.page).collect();
    (*pages.iter().min().unwrap(), *pages.iter().max().unwrap())
}

#[test]
fn a_smart_split_with_overlapping_chunks_exports_one_file_per_part() {
    start_engine();
    let dir = tempfile::tempdir().unwrap();
    let source = save(&dir, "book.pdf", document(10));
    let before = fs::read(&source).unwrap();

    // Open (FR-01, FR-02).
    let (info, _doc) = pdf::open_document(&source, None).unwrap();
    assert_eq!(info.page_count, 10);

    // AI split in two overlapping chunks (FR-33, FR-34), each answered for its own pages.
    let mock = Scripted::new(|request: &AiRequest, _| {
        let (first, last) = first_and_last(request);
        Ok(chapters_in(first, last))
    });
    let chunks = [chunk(1, 6), chunk(5, 10)];
    let ranges = tauri::async_runtime::block_on(response::split(&mock, &chunks, info.page_count, false)).unwrap();
    assert_eq!(
        ranges,
        [
            Range { start: 1, end: 3, title: "Chapter 1".into() },
            Range { start: 4, end: 7, title: "Chapter 2".into() },
            Range { start: 8, end: 10, title: "Chapter 3".into() },
        ]
    );

    // Only short excerpts and the instruction were sent, each page in its chunk (FR-33, FR-61).
    let sent = mock.requests();
    assert_eq!(sent.len(), 2);
    for request in &sent {
        assert_eq!(request.instruction, TASK);
        assert!(request.pages.iter().all(|p| p.head.len() < 200 && p.tail.len() < 200));
    }

    // Export (FR-50 to FR-52), named from the default template.
    let out = tempfile::tempdir().unwrap();
    let titles: Vec<String> = ranges.iter().map(|r| r.title.clone()).collect();
    let names = naming::export_names("{original} - {index} - {title}", &info.file_name, &titles);
    let saved = export::export(&ExportRequest {
        path: source.clone(),
        password: None,
        ranges: ranges.clone(),
        folder: out.path().to_string_lossy().into_owned(),
        names: names.parts,
    })
    .unwrap();
    assert_eq!(saved.renamed, 0);
    let files: Vec<String> = saved
        .paths
        .iter()
        .map(|p| std::path::Path::new(p).file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    assert_eq!(files, ["book - 01 - Chapter 1.pdf", "book - 02 - Chapter 2.pdf", "book - 03 - Chapter 3.pdf"]);
    let pages: Vec<u32> = saved.paths.iter().map(|p| pdf::open_document(p, None).unwrap().0.page_count).collect();
    assert_eq!(pages, [3, 4, 3]);

    // The original is never changed (FR-04).
    assert_eq!(fs::read(&source).unwrap(), before);
}

#[test]
fn a_bad_answer_is_asked_for_again_once() {
    // The first answer is not JSON; the second is right (FR-36).
    let mock = Scripted::new(|request: &AiRequest, n| {
        let (first, last) = first_and_last(request);
        Ok(if n == 0 { "Sure! Here are the chapters:".into() } else { chapters_in(first, last) })
    });
    let ranges = tauri::async_runtime::block_on(response::split(&mock, &[chunk(1, 10)], 10, false)).unwrap();
    assert_eq!(ranges.len(), 3);
    assert_eq!(mock.requests().len(), 2, "one retry, no more");
}

#[test]
fn answers_that_break_the_rules_are_retried_then_reported() {
    // Overlapping parts, then pages past the chunk: both fail validation (FR-35), so the user is
    // told what was wrong and can set the parts by hand (FR-36).
    let mock = Scripted::new(|_: &AiRequest, n| {
        Ok(if n == 0 { reply(&[(1, 6, "A"), (5, 10, "B")]) } else { reply(&[(1, 12, "Everything")]) })
    });
    let err = tauri::async_runtime::block_on(response::split(&mock, &[chunk(1, 10)], 10, false)).unwrap_err();
    assert!(matches!(err, SplitError::AiReply(1, 10, _)), "{err}");
    assert_eq!(mock.requests().len(), 2);
}

#[test]
fn a_provider_error_stops_the_split_at_once() {
    // A bad key or a rate limit isn't retried: asking again wouldn't help.
    let mock = Scripted::new(|_: &AiRequest, _| Err(SplitError::Ai("Claude didn't accept your API key".into())));
    let err = tauri::async_runtime::block_on(response::split(&mock, &[chunk(1, 5), chunk(6, 10)], 10, false)).unwrap_err();
    assert!(err.to_string().contains("didn't accept your API key"), "{err}");
    assert_eq!(mock.requests().len(), 1);
}

#[test]
fn a_failed_split_exports_nothing() {
    start_engine();
    let dir = tempfile::tempdir().unwrap();
    let source = save(&dir, "book.pdf", document(10));
    let out = tempfile::tempdir().unwrap();

    let mock = Scripted::new(|_: &AiRequest, _| Ok("[]".into()));
    let result = tauri::async_runtime::block_on(response::split(&mock, &[chunk(1, 10)], 10, false));
    assert!(result.is_err());

    // The interface only offers export once it has parts; without them there is nothing to write,
    // and an empty export is refused before any file is created (NFR-09).
    let err = export::export(&ExportRequest {
        path: source,
        password: None,
        ranges: Vec::new(),
        folder: out.path().to_string_lossy().into_owned(),
        names: Vec::new(),
    })
    .unwrap_err();
    assert!(matches!(err, SplitError::InvalidRanges(_)), "{err}");
    assert_eq!(fs::read_dir(out.path()).unwrap().count(), 0);
}

#[test]
fn custom_mode_may_leave_pages_out() {
    // With gaps allowed (Custom mode, FR-20), the AI may skip pages, such as a cover.
    let mock = Scripted::new(|_: &AiRequest, _| Ok(reply(&[(2, 5, "Body"), (7, 10, "Appendix")])));
    let ranges = tauri::async_runtime::block_on(response::split(&mock, &[chunk(1, 10)], 10, true)).unwrap();
    assert_eq!(ranges.iter().map(|r| (r.start, r.end)).collect::<Vec<_>>(), [(2, 5), (7, 10)]);
}
