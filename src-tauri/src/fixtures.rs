//! The sample PDF set (SRS 9.1, #12) and the integration tests that open, split and export each
//! file with the rule strategies (SRS 9.2). Every file is generated here from made-up text, so the
//! set holds no personal or client data. The small files are committed in `tests/fixtures/`;
//! `npm run fixtures` writes them again, and the 1,000-page file only to `tests/fixtures/generated/`
//! (not committed). The tests build the 1,000-page file in a temp folder.

use std::path::PathBuf;

use lopdf::{Document, Object, ObjectId, Stream, dictionary};

/// The password of `encrypted-invoices.pdf`. A made-up test value.
pub const PASSWORD: &str = "fixture-password";

/// One page of a generated PDF.
pub enum Page {
    /// Lines of text, top to bottom.
    Text(Vec<String>),
    /// No content at all.
    Blank,
    /// A grey picture and no text layer, like a scan.
    Scan,
}

/// A bookmark and the bookmarks under it.
pub struct Mark {
    pub title: String,
    pub page: u32,
    pub children: Vec<Mark>,
}

fn mark(title: impl Into<String>, page: u32, children: Vec<Mark>) -> Mark {
    Mark { title: title.into(), page, children }
}

/// A PDF with these pages and this outline.
pub fn build(pages: &[Page], outline: &[Mark]) -> Document {
    let mut doc = Document::with_version("1.7");
    // A fixed /ID keeps the files the same on every run and lets the encrypted one be made.
    doc.trailer.set(
        "ID",
        Object::Array(vec![Object::string_literal(b"splitr-fixture-1"), Object::string_literal(b"splitr-fixture-1")]),
    );
    let pages_id = doc.new_object_id();
    let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
    let scan = doc.add_object(Stream::new(
        dictionary! {
            "Type" => "XObject", "Subtype" => "Image", "Width" => 8, "Height" => 8,
            "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8,
        },
        vec![0xB0; 64],
    ));
    let kids: Vec<ObjectId> = pages
        .iter()
        .map(|page| {
            let mut dict = dictionary! {
                "Type" => "Page", "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                "Resources" => dictionary! { "Font" => dictionary! { "F1" => font }, "XObject" => dictionary! { "Im1" => scan } },
            };
            let content = match page {
                Page::Blank => None,
                Page::Scan => Some("q 540 0 0 720 36 36 cm /Im1 Do Q".to_string()),
                Page::Text(lines) => {
                    let shown: Vec<String> = lines.iter().map(|l| format!("({}) Tj 0 -18 Td", escape(l))).collect();
                    Some(format!("BT /F1 12 Tf 72 720 Td {} ET", shown.join(" ")))
                }
            };
            if let Some(content) = content {
                dict.set("Contents", doc.add_object(Stream::new(dictionary! {}, content.into_bytes())));
            }
            doc.add_object(dict)
        })
        .collect();
    let count = kids.len() as i64;
    let kids: Vec<Object> = kids.iter().map(|&id| id.into()).collect();
    doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids.clone(), "Count" => count }));
    let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog);
    if !outline.is_empty() {
        let outlines = doc.new_object_id();
        let (first, last, count) = add_marks(&mut doc, outline, outlines, &kids);
        doc.objects.insert(
            outlines,
            Object::Dictionary(dictionary! { "Type" => "Outlines", "First" => first, "Last" => last, "Count" => count }),
        );
        doc.catalog_mut().unwrap().set("Outlines", outlines);
    }
    doc
}

/// Adds `marks` under `parent` and returns the first and last of them and how many are visible.
fn add_marks(doc: &mut Document, marks: &[Mark], parent: ObjectId, pages: &[Object]) -> (ObjectId, ObjectId, i64) {
    let ids: Vec<ObjectId> = marks.iter().map(|_| doc.new_object_id()).collect();
    let mut visible = 0;
    for (i, m) in marks.iter().enumerate() {
        let mut item = dictionary! {
            "Title" => Object::string_literal(m.title.as_str()), "Parent" => parent,
            "Dest" => vec![pages[m.page as usize - 1].clone(), "Fit".into()],
        };
        if i > 0 {
            item.set("Prev", ids[i - 1]);
        }
        if let Some(&next) = ids.get(i + 1) {
            item.set("Next", next);
        }
        visible += 1;
        if !m.children.is_empty() {
            let (first, last, count) = add_marks(doc, &m.children, ids[i], pages);
            item.set("First", first);
            item.set("Last", last);
            item.set("Count", count);
            visible += count;
        }
        doc.objects.insert(ids[i], Object::Dictionary(item));
    }
    (ids[0], *ids.last().unwrap(), visible)
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
}

/// Filler text for the body of a page.
fn body(topic: &str, page: u32) -> Vec<String> {
    (1..=6).map(|n| format!("{topic}: paragraph {n} of page {page}, ordinary sample text.")).collect()
}

fn page_with(heading: &[&str], topic: &str, page: u32) -> Page {
    let mut lines: Vec<String> = heading.iter().map(|l| l.to_string()).collect();
    lines.extend(body(topic, page));
    lines.push(format!("Page {page}"));
    Page::Text(lines)
}

/// Pages for documents that each start with a heading: `(heading lines, pages)`.
fn batch(docs: &[(&[&str], u32)], topic: &str) -> Vec<Page> {
    let mut pages = Vec::new();
    for (heading, length) in docs {
        for n in 0..*length {
            let number = pages.len() as u32 + 1;
            if n == 0 {
                pages.push(page_with(heading, topic, number));
            } else {
                pages.push(page_with(&["Continued from the previous page"], topic, number));
            }
        }
    }
    pages
}

fn chapters(titles: &[(&str, u32)]) -> Vec<Page> {
    let docs: Vec<([&str; 1], u32)> = titles.iter().map(|&(t, n)| ([t], n)).collect();
    let docs: Vec<(&[&str], u32)> = docs.iter().map(|(h, n)| (&h[..], *n)).collect();
    batch(&docs, "Lesson")
}

fn invoices() -> Vec<Page> {
    batch(
        &[
            (&["Northwind Paper Co.", "Invoice No: INV-1001", "Date: 2026-03-02"], 2),
            (&["Northwind Paper Co.", "Invoice No: INV-1002", "Date: 2026-03-09"], 1),
            (&["Northwind Paper Co.", "Invoice No: INV-1003", "Date: 2026-03-16"], 3),
            (&["Northwind Paper Co.", "Invoice No: INV-1004", "Date: 2026-03-23"], 1),
        ],
        "Line items",
    )
}

/// Every committed sample file and its contents, by file name.
pub fn files() -> Vec<(&'static str, Vec<u8>)> {
    let mut files = Vec::new();
    let mut add = |name: &'static str, mut doc: Document| {
        let mut bytes = Vec::new();
        doc.save_to(&mut bytes).unwrap();
        files.push((name, bytes));
    };

    let textbook = chapters(&[("Preface", 2), ("Chapter 1 Numbers", 4), ("Chapter 2 Shapes", 4), ("Chapter 3 Patterns", 4)]);
    add(
        "textbook-bookmarks.pdf",
        build(
            &textbook,
            &[
                mark("Preface", 1, vec![]),
                mark("Chapter 1 Numbers", 3, vec![mark("1.1 Counting", 4, vec![]), mark("1.2 Adding", 5, vec![])]),
                mark("Chapter 2 Shapes", 7, vec![mark("2.1 Circles", 9, vec![])]),
                mark("Chapter 3 Patterns", 11, vec![]),
            ],
        ),
    );
    add("textbook-no-bookmarks.pdf", build(&textbook, &[]));
    add(
        "course-notes-single-root.pdf",
        build(
            &chapters(&[("Week 1 Introduction", 3), ("Week 2 Practice", 3), ("Week 3 Review", 3)]),
            &[mark(
                "Course Notes",
                1,
                vec![mark("Week 1 Introduction", 1, vec![]), mark("Week 2 Practice", 4, vec![]), mark("Week 3 Review", 7, vec![])],
            )],
        ),
    );
    add(
        "field-guide-nested.pdf",
        build(
            &chapters(&[("Birds", 2), ("Trees", 2), ("Insects", 1), ("Rivers", 2)]),
            &[mark(
                "Field Guide",
                1,
                vec![mark(
                    "Contents",
                    1,
                    vec![mark("Birds", 1, vec![]), mark("Trees", 3, vec![]), mark("Insects", 5, vec![]), mark("Rivers", 6, vec![])],
                )],
            )],
        ),
    );
    add("invoice-batch.pdf", build(&invoices(), &[]));
    add(
        "bank-statements.pdf",
        build(
            &batch(
                &[
                    (&["Example Savings Bank", "Account Number: **** 0098", "Statement for March 2026"], 2),
                    (&["Example Savings Bank", "Account Number: **** 4417", "Statement for March 2026"], 2),
                    (&["Example Savings Bank", "Account Number: **** 7350", "Statement for March 2026"], 1),
                ],
                "Transactions",
            ),
            &[],
        ),
    );
    add(
        "employee-records.pdf",
        build(
            &batch(
                &[
                    (&["Staff file", "Employee ID: E-1001", "Department: Sample Team A"], 1),
                    (&["Staff file", "Employee ID: E-1002", "Department: Sample Team B"], 2),
                    (&["Staff file", "Employee ID: E-1003", "Department: Sample Team A"], 1),
                ],
                "Record",
            ),
            &[],
        ),
    );
    add(
        "insurance-claims.pdf",
        build(
            &batch(
                &[
                    (&["Claim form", "Claim No. CLM-2001"], 2),
                    (&["Claim form", "Claim No. CLM-2002"], 1),
                    (&["Claim form", "Claim No. CLM-2003"], 2),
                ],
                "Details",
            ),
            &[],
        ),
    );
    add(
        "bills-of-lading.pdf",
        build(
            &batch(
                &[(&["Bill of Lading No: BL-5001", "Vessel: Sample Carrier"], 1), (&["Bill of Lading No: BL-5002", "Vessel: Sample Carrier"], 2)],
                "Cargo",
            ),
            &[],
        ),
    );
    let mut separated = Vec::new();
    for (i, (title, length)) in [("Letter one", 2), ("Letter two", 1), ("Letter three", 2)].into_iter().enumerate() {
        if i > 0 {
            separated.push(Page::Blank);
        }
        for n in 0..length {
            let number = separated.len() as u32 + 1;
            separated.push(page_with(&[if n == 0 { title } else { "Continued" }], "Letter", number));
        }
    }
    add("blank-separators.pdf", build(&separated, &[]));
    add(
        "contract-bundle.pdf",
        build(
            &batch(
                &[
                    (&["Services Agreement", "Between Sample Party A and Sample Party B"], 3),
                    (&["Non-Disclosure Agreement", "Between Sample Party A and Sample Party C"], 2),
                    (&["Exhibit A: Schedule of Fees"], 1),
                ],
                "Clause",
            ),
            &[],
        ),
    );
    add("scanned-pages.pdf", build(&[Page::Scan, Page::Scan, Page::Blank, Page::Scan], &[]));

    let mut encrypted = build(&invoices(), &[]);
    let state = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V2 {
        document: &encrypted,
        owner_password: "fixture-owner",
        user_password: PASSWORD,
        key_length: 128,
        permissions: lopdf::Permissions::all(),
    })
    .unwrap();
    encrypted.encrypt(&state).unwrap();
    add("encrypted-invoices.pdf", encrypted);

    files.push(("corrupt.pdf", b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R\nthis file was cut off".to_vec()));
    files
}

/// A `pages`-page report with a bookmark every 100 pages, for the large-file tests (SRS 9.1).
pub fn large(pages: u32) -> Document {
    let content: Vec<Page> = (1..=pages)
        .map(|n| if n % 100 == 1 { page_with(&[&format!("Section {}", n / 100 + 1)], "Report", n) } else { page_with(&[], "Report", n) })
        .collect();
    let outline: Vec<Mark> = (0..pages.div_ceil(100)).map(|i| mark(format!("Section {}", i + 1), i * 100 + 1, vec![])).collect();
    build(&content, &outline)
}

/// `tests/fixtures/` in the repository.
pub fn folder() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("tests").join("fixtures")
}

#[cfg(test)]
mod tests {
    use std::ops::ControlFlow;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::error::SplitError;
    use crate::export::{self, ExportRequest};
    use crate::pdf::{self, tests::start_engine};
    use crate::preset_run::{self, PresetRun, RunOptions};
    use crate::presets;
    use crate::strategy::DocumentText;

    /// Writes the sample files again: `npm run fixtures`.
    #[test]
    #[ignore = "writes tests/fixtures; run with npm run fixtures"]
    fn write_fixtures() {
        let dir = folder();
        for (name, bytes) in files() {
            std::fs::write(dir.join(name), bytes).unwrap();
        }
        let generated = dir.join("generated");
        std::fs::create_dir_all(&generated).unwrap();
        large(1000).save(generated.join("large-1000-pages.pdf")).unwrap();
    }

    #[test]
    fn the_committed_files_match_the_generator() {
        let files = files();
        assert!(files.len() >= 10, "SRS 9.1 needs a varied set");
        for (name, bytes) in files {
            let committed = std::fs::read(folder().join(name)).unwrap_or_else(|e| panic!("{name}: {e}. Run npm run fixtures"));
            // Encryption is different on every run; its own test checks what it holds.
            if name != "encrypted-invoices.pdf" {
                assert!(committed == bytes, "{name} is out of date. Run npm run fixtures");
            }
        }
    }

    fn path(name: &str) -> String {
        folder().join(name).to_string_lossy().into_owned()
    }

    fn extract(path: &str, password: Option<&str>) -> DocumentText {
        start_engine();
        let (_, doc) = pdf::open_document(path, password).unwrap();
        pdf::extract_text(&doc, |_, _| ControlFlow::Continue(())).unwrap()
    }

    fn run_preset(path: &str, password: Option<&str>, preset: &str, options: &RunOptions) -> crate::error::Result<PresetRun> {
        let preset = presets::bundled().into_iter().find(|p| p.id == preset).unwrap();
        preset_run::run(&preset, &extract(path, password), options)
    }

    const KEEP: RunOptions = RunOptions { blank_threshold: 0, drop_separators: false };

    /// Opens, splits and exports `file` with `preset`, checks the parts, and checks that each
    /// exported file has its part's pages and that the original is unchanged (FR-04).
    fn split_and_export(file: &str, password: Option<&str>, preset: &str, options: &RunOptions, expected: &[(u32, u32, &str)]) {
        let source = path(file);
        let before = std::fs::read(&source).unwrap();
        let run = run_preset(&source, password, preset, options).unwrap();
        let PresetRun::Parts { ranges, .. } = run else { panic!("{file}: expected parts, got {run:?}") };
        let got: Vec<(u32, u32, &str)> = ranges.iter().map(|r| (r.start, r.end, r.title.as_str())).collect();
        assert_eq!(got, expected, "{file}");
        export_and_check(file, password, &ranges, &before);
    }

    /// Exports `ranges` of `file` and checks that each exported file has its part's pages and that
    /// the original still holds `before` (FR-04).
    fn export_and_check(file: &str, password: Option<&str>, ranges: &[crate::ranges::Range], before: &[u8]) {
        let source = path(file);
        let out = tempfile::tempdir().unwrap();
        let names = crate::naming::export_names(
            "{original} - {index} - {title}",
            file,
            &ranges.iter().map(|r| r.title.clone()).collect::<Vec<_>>(),
        );
        let request = ExportRequest {
            path: source.clone(),
            password: password.map(str::to_string),
            ranges: ranges.to_vec(),
            folder: out.path().to_string_lossy().into_owned(),
            names: names.parts,
        };
        let saved = export::export(&request).unwrap();
        assert_eq!(saved.paths.len(), ranges.len(), "{file}");
        let text = extract(&source, password);
        for (part, range) in saved.paths.iter().zip(ranges) {
            let exported = extract(part, None);
            assert_eq!(exported.page_count, range.end - range.start + 1, "{part}");
            let first = &text.pages[range.start as usize - 1];
            assert_eq!(exported.pages[0].trim(), first.trim(), "{part} should start with page {}", range.start);
        }
        assert!(std::fs::read(&source).unwrap() == before, "{file} was changed");
    }

    // Phase 1 exit (#12): the files below split correctly with rules only.

    #[test]
    fn a_textbook_splits_at_its_chapters() {
        split_and_export(
            "textbook-bookmarks.pdf",
            None,
            "books-by-chapter",
            &KEEP,
            &[(1, 2, "Preface"), (3, 6, "Chapter 1 Numbers"), (7, 10, "Chapter 2 Shapes"), (11, 14, "Chapter 3 Patterns")],
        );
    }

    #[test]
    fn course_notes_under_one_bookmark_split_by_week() {
        split_and_export(
            "course-notes-single-root.pdf",
            None,
            "academic-course-by-chapter",
            &KEEP,
            &[(1, 3, "Week 1 Introduction"), (4, 6, "Week 2 Practice"), (7, 9, "Week 3 Review")],
        );
    }

    #[test]
    fn a_guide_two_levels_under_one_bookmark_splits_by_topic() {
        split_and_export(
            "field-guide-nested.pdf",
            None,
            "books-by-chapter",
            &KEEP,
            &[(1, 2, "Birds"), (3, 4, "Trees"), (5, 5, "Insects"), (6, 7, "Rivers")],
        );
    }

    const INVOICES: &[(u32, u32, &str)] = &[
        (1, 2, "Invoice No: INV-1001"),
        (3, 3, "Invoice No: INV-1002"),
        (4, 6, "Invoice No: INV-1003"),
        (7, 7, "Invoice No: INV-1004"),
    ];

    #[test]
    fn an_invoice_batch_splits_by_invoice_number() {
        split_and_export("invoice-batch.pdf", None, "invoices-by-number", &KEEP, INVOICES);
    }

    #[test]
    fn an_encrypted_batch_needs_its_password_then_splits() {
        start_engine();
        let err = pdf::open_document(&path("encrypted-invoices.pdf"), None).err().unwrap();
        assert!(matches!(err, SplitError::PasswordRequired), "{err}");
        split_and_export("encrypted-invoices.pdf", Some(PASSWORD), "invoices-by-number", &KEEP, INVOICES);
    }

    #[test]
    fn statements_split_by_account() {
        split_and_export(
            "bank-statements.pdf",
            None,
            "statements-by-account",
            &KEEP,
            &[(1, 2, "Account Number: **** 0098"), (3, 4, "Account Number: **** 4417"), (5, 5, "Account Number: **** 7350")],
        );
    }

    #[test]
    fn staff_files_split_by_employee() {
        split_and_export(
            "employee-records.pdf",
            None,
            "hr-by-employee",
            &KEEP,
            &[(1, 1, "Employee ID: E-1001"), (2, 3, "Employee ID: E-1002"), (4, 4, "Employee ID: E-1003")],
        );
    }

    #[test]
    fn claims_split_by_claim_number() {
        split_and_export(
            "insurance-claims.pdf",
            None,
            "insurance-by-policy",
            &KEEP,
            &[(1, 2, "Claim No. CLM-2001"), (3, 3, "Claim No. CLM-2002"), (4, 5, "Claim No. CLM-2003")],
        );
    }

    #[test]
    fn shipping_papers_split_by_bill_of_lading() {
        split_and_export(
            "bills-of-lading.pdf",
            None,
            "shipping-by-shipment",
            &KEEP,
            &[(1, 1, "Bill of Lading No: BL-5001"), (2, 3, "Bill of Lading No: BL-5002")],
        );
    }

    #[test]
    fn letters_split_at_blank_pages_which_can_be_dropped() {
        let drop = RunOptions { blank_threshold: 0, drop_separators: true };
        split_and_export("blank-separators.pdf", None, "scanned-blank-pages", &drop, &[(1, 2, ""), (4, 4, ""), (6, 7, "")]);
        split_and_export(
            "blank-separators.pdf",
            None,
            "scanned-blank-pages",
            &KEEP,
            &[(1, 2, ""), (3, 3, ""), (4, 4, ""), (5, 5, ""), (6, 7, "")],
        );
    }

    // The files below need the AI or can't be split, and say so.

    #[test]
    fn without_bookmarks_or_with_an_ai_preset_the_ai_is_needed() {
        let run = run_preset(&path("textbook-no-bookmarks.pdf"), None, "books-by-chapter", &KEEP).unwrap();
        assert_eq!(run, PresetRun::NeedsAi { reason: Some("This PDF has no bookmarks".into()) });
        let run = run_preset(&path("contract-bundle.pdf"), None, "contracts-by-agreement", &KEEP).unwrap();
        assert_eq!(run, PresetRun::NeedsAi { reason: None });
    }

    /// Answers like a model would from the excerpts alone: a part starts on each page whose first
    /// line isn't "Continued", titled with that line.
    struct ReadsHeadings(std::cell::Cell<u32>);

    impl crate::ai::Provider for ReadsHeadings {
        async fn complete(&self, request: &crate::ai::AiRequest) -> crate::error::Result<String> {
            self.0.set(self.0.get() + 1);
            let mut parts: Vec<serde_json::Value> = Vec::new();
            for page in &request.pages {
                let heading = page.head.split(" / ").next().unwrap_or("");
                if heading.starts_with("Continued") && !parts.is_empty() {
                    parts.last_mut().unwrap()["end"] = page.page.into();
                } else {
                    parts.push(serde_json::json!({ "start": page.page, "end": page.page, "title": heading }));
                }
            }
            Ok(serde_json::Value::from(parts).to_string())
        }
    }

    /// #64: an AI preset, or a rule preset that falls back to the AI, splits and exports a sample
    /// file end to end with a scripted provider standing in for the AI service.
    #[test]
    fn ai_presets_split_and_export_with_a_mocked_provider() {
        use crate::strategy::{AiRun, AiStrategy};
        for (file, preset, expected) in [
            (
                "textbook-no-bookmarks.pdf",
                "books-by-chapter",
                &[(1, 2, "Preface"), (3, 6, "Chapter 1 Numbers"), (7, 10, "Chapter 2 Shapes"), (11, 14, "Chapter 3 Patterns")][..],
            ),
            (
                "contract-bundle.pdf",
                "contracts-by-agreement",
                &[(1, 3, "Services Agreement"), (4, 5, "Non-Disclosure Agreement"), (6, 6, "Exhibit A: Schedule of Fees")][..],
            ),
        ] {
            let source = path(file);
            let before = std::fs::read(&source).unwrap();
            assert!(matches!(run_preset(&source, None, preset, &KEEP).unwrap(), PresetRun::NeedsAi { .. }));
            let preset = presets::bundled().into_iter().find(|p| p.id == preset).unwrap();
            let provider = ReadsHeadings(std::cell::Cell::new(0));
            let strategy = AiStrategy { instruction: preset.prompt };
            let run = tauri::async_runtime::block_on(strategy.propose(&provider, &extract(&source, None), |_, _| {})).unwrap();
            let AiRun::Parts { ranges } = run else { panic!("{file}: expected parts, got {run:?}") };
            let got: Vec<(u32, u32, &str)> = ranges.iter().map(|r| (r.start, r.end, r.title.as_str())).collect();
            assert_eq!(got, expected, "{file}");
            assert_eq!(provider.0.get(), 1, "{file}: one chunk, answered first time");
            export_and_check(file, None, &ranges, &before);
        }
    }

    #[test]
    fn a_scan_without_text_is_not_split_by_its_text() {
        let err = run_preset(&path("scanned-pages.pdf"), None, "scanned-blank-pages", &KEEP).unwrap_err();
        assert!(matches!(err, SplitError::NoTextLayer), "{err}");
        assert!(err.to_string().contains("try turning on"), "{err}");
    }

    /// `file` as a scan: each page replaced by a greyscale picture of it, rendered by PDFium, with
    /// no text layer (FR-80). Saved as `scan.pdf` in `dir`.
    fn scanned(file: &str, dir: &tempfile::TempDir) -> String {
        start_engine();
        let (_, source) = pdf::open_document(&path(file), None).unwrap();
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let kids: Vec<Object> = (1..=source.pages().len() as u32)
            .map(|page| {
                let picture = image::DynamicImage::from(pdf::render_for_ocr(&source, page).unwrap()).to_luma8();
                let mut stream = Stream::new(
                    dictionary! {
                        "Type" => "XObject", "Subtype" => "Image", "Width" => picture.width(), "Height" => picture.height(),
                        "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8,
                    },
                    picture.into_raw(),
                );
                stream.compress().unwrap();
                let picture = doc.add_object(stream);
                let content = doc.add_object(Stream::new(dictionary! {}, b"q 612 0 0 792 0 0 cm /Im1 Do Q".to_vec()));
                doc.add_object(dictionary! {
                    "Type" => "Page", "Parent" => pages_id, "Contents" => content,
                    "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                    "Resources" => dictionary! { "XObject" => dictionary! { "Im1" => picture } },
                })
                .into()
            })
            .collect();
        let count = kids.len() as i64;
        doc.objects.insert(pages_id, Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }));
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        let out = dir.path().join("scan.pdf");
        doc.save(&out).unwrap();
        out.to_string_lossy().into_owned()
    }

    /// Phase 5 exit (#24): scans of the sample files split at the same pages as the files with
    /// text, once OCR has read them through the viewer, as the app does. OCR can misread a letter
    /// of a title (the user fixes it on the review screen), so titles are checked where it reads
    /// them reliably.
    #[test]
    fn scanned_files_split_correctly_with_ocr() {
        crate::ocr::tests::start_ocr();
        let separators = RunOptions { blank_threshold: 0, drop_separators: true };
        for (file, preset, options, expected, check_titles) in [
            ("invoice-batch.pdf", "invoices-by-number", KEEP, INVOICES, true),
            (
                "bills-of-lading.pdf",
                "shipping-by-shipment",
                KEEP,
                &[(1, 1, "Bill of Lading No: BL-5001"), (2, 3, "Bill of Lading No: BL-5002")][..],
                true,
            ),
            ("employee-records.pdf", "hr-by-employee", KEEP, &[(1, 1, ""), (2, 3, ""), (4, 4, "")][..], false),
            ("blank-separators.pdf", "scanned-blank-pages", separators, &[(1, 2, ""), (4, 4, ""), (6, 7, "")][..], true),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let scan = scanned(file, &dir);
            assert!(extract(&scan, None).pages.iter().all(|p| p.trim().is_empty()), "{file}: the scan has a text layer");

            let viewer = crate::viewer::Viewer::new(dir.path().join("cache"));
            viewer.open(&scan, None).unwrap();
            let mut ocr_steps = Vec::new();
            let text = viewer.extract_with_ocr(&scan, |_, _| (), |done, total| ocr_steps.push((done, total))).unwrap();
            assert!(!ocr_steps.is_empty(), "{file}: OCR didn't run");

            let preset = presets::bundled().into_iter().find(|p| p.id == preset).unwrap();
            let run = preset_run::run(&preset, &text, &options).unwrap();
            let PresetRun::Parts { ranges, .. } = run else { panic!("{file}: expected parts, got {run:?}") };
            let got: Vec<(u32, u32, &str)> =
                ranges.iter().map(|r| (r.start, r.end, if check_titles { r.title.as_str() } else { "" })).collect();
            assert_eq!(got, expected, "{file}");
            viewer.close();
        }
    }

    #[test]
    fn a_corrupt_file_gives_a_clear_error() {
        start_engine();
        let err = pdf::open_document(&path("corrupt.pdf"), None).err().unwrap();
        assert_eq!(err.to_string(), "corrupt.pdf is damaged or is not a PDF");
    }

    /// SRS 9.1: a 1,000-page file, built here rather than committed. NFR-01 asks for 300 pages in
    /// 15 s; 1,000 pages are read, split and exported well within that.
    #[test]
    fn a_1000_page_file_splits_and_exports_in_time() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("large.pdf");
        large(1000).save(&source).unwrap();
        let source = source.to_string_lossy().into_owned();

        let started = Instant::now();
        let run = run_preset(&source, None, "books-by-chapter", &KEEP).unwrap();
        let PresetRun::Parts { ranges, .. } = run else { panic!("expected parts") };
        assert_eq!(ranges.len(), 10);
        assert_eq!((ranges[9].start, ranges[9].end, ranges[9].title.as_str()), (901, 1000, "Section 10"));
        let out = tempfile::tempdir().unwrap();
        let request = ExportRequest {
            path: source,
            password: None,
            names: (1..=10).map(|n| format!("part {n}")).collect(),
            ranges,
            folder: out.path().to_string_lossy().into_owned(),
        };
        assert_eq!(export::export(&request).unwrap().paths.len(), 10);
        let took = started.elapsed();
        assert!(took < Duration::from_secs(15), "took {took:?}");
    }
}
