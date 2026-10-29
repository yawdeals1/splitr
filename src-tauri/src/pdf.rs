//! PDF engine (SRS 3): open, render thumbnails, extract text and outline, write parts.
//!
//! PDFium (via pdfium-render) renders, extracts text and reads the outline; lopdf writes parts.
//! The PDFium library ships as a Tauri resource in `pdfium/` (SRS 2.5, 5.3, NFR-12), fetched by
//! `scripts/fetch-pdfium.mjs`. The original file is never modified (FR-04).

use std::io::{Cursor, ErrorKind};
use std::path::Path;
use std::sync::OnceLock;

use image::ImageFormat;
use pdfium_render::prelude::{PdfDocument, PdfRenderConfig, Pdfium, PdfiumError, PdfiumInternalError};
use serde::Serialize;

use crate::error::{Result, SplitError};

/// The PDFium instance loaded at startup, or why it failed to load.
static ENGINE: OnceLock<std::result::Result<Pdfium, String>> = OnceLock::new();

/// Loads the PDFium library from `dir` (the `pdfium` resource folder). Call once at startup.
/// A failure is kept, so every later PDF call reports it to the user.
pub fn init(dir: &Path) -> Result<()> {
    ENGINE
        .get_or_init(|| load(dir))
        .as_ref()
        .map(|_| ())
        .map_err(|e| SplitError::PdfEngine(e.clone()))
}

fn load(dir: &Path) -> std::result::Result<Pdfium, String> {
    let path = Pdfium::pdfium_platform_library_name_at_path(dir);
    if !path.is_file() {
        return Err(format!(
            "the PDFium library is missing ({}). Reinstall Splitr; developers run `npm run fetch-pdfium`",
            path.display()
        ));
    }
    let bindings = Pdfium::bind_to_library(&path)
        .map_err(|e| format!("the PDFium library at {} could not be loaded ({e})", path.display()))?;
    Ok(Pdfium::new(bindings))
}

/// The loaded PDFium instance, or a user-facing error if it failed to load.
pub fn engine() -> Result<&'static Pdfium> {
    match ENGINE.get() {
        Some(Ok(pdfium)) => Ok(pdfium),
        Some(Err(e)) => Err(SplitError::PdfEngine(e.clone())),
        None => Err(SplitError::PdfEngine("the PDFium library was not loaded".into())),
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PdfInfo {
    pub path: String,
    pub file_name: String,
    pub page_count: u32,
    pub size_bytes: u64,
}

/// Opens the PDF at `path` read-only and returns its name, page count and size (FR-01, FR-02).
/// `password` unlocks an encrypted file (FR-03). Every failure is a specific, user-facing error,
/// so the app stays usable (FR-05, NFR-08). The file is only read, never written (FR-04).
/// The loaded document is returned too, so its pages can be rendered (FR-10); the password
/// unlocks it in memory and is not kept.
pub fn open_document(path: &str, password: Option<&str>) -> Result<(PdfInfo, PdfDocument<'static>)> {
    let file_path = Path::new(path);
    let file_name = file_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string());

    let metadata = std::fs::metadata(file_path).map_err(|e| io_error(&e, &file_name))?;
    if !metadata.is_file() {
        return Err(SplitError::CorruptPdf(file_name));
    }

    let password = password.filter(|p| !p.is_empty());
    let document = engine()?
        .load_pdf_from_file(file_path, password)
        .map_err(|e| load_error(e, &file_name, password.is_some()))?;

    let page_count = u32::try_from(document.pages().len()).unwrap_or(0);
    if page_count == 0 {
        return Err(SplitError::EmptyPdf(file_name));
    }

    let info = PdfInfo { path: path.to_string(), file_name, page_count, size_bytes: metadata.len() };
    Ok((info, document))
}

/// Renders page `page` (1-based) of `document` as a JPEG `width` pixels wide (FR-10, FR-11).
pub fn render_page(document: &PdfDocument, page: u32, width: u32) -> Result<Vec<u8>> {
    let failed = |e: String| SplitError::RenderFailed(page, e);
    let index = i32::try_from(page.saturating_sub(1)).map_err(|e| failed(e.to_string()))?;
    let width = i32::try_from(width).map_err(|e| failed(e.to_string()))?;
    let pdf_page = document.pages().get(index).map_err(|e| failed(e.to_string()))?;
    let image = pdf_page
        .render_with_config(&PdfRenderConfig::new().set_target_width(width))
        .and_then(|bitmap| bitmap.as_image())
        .map_err(|e| failed(e.to_string()))?;
    let mut jpeg = Vec::new();
    image
        .to_rgb8()
        .write_to(&mut Cursor::new(&mut jpeg), ImageFormat::Jpeg)
        .map_err(|e| failed(e.to_string()))?;
    Ok(jpeg)
}

fn io_error(e: &std::io::Error, file_name: &str) -> SplitError {
    match e.kind() {
        ErrorKind::NotFound => SplitError::FileNotFound(file_name.to_string()),
        _ => SplitError::FileUnreadable(file_name.to_string()),
    }
}

/// Maps a PDFium load error to a message the user can act on.
fn load_error(e: PdfiumError, file_name: &str, had_password: bool) -> SplitError {
    match e {
        PdfiumError::PdfiumLibraryInternalError(internal) => match internal {
            PdfiumInternalError::PasswordError if had_password => SplitError::WrongPassword,
            PdfiumInternalError::PasswordError => SplitError::PasswordRequired,
            PdfiumInternalError::SecurityError => SplitError::UnsupportedSecurity,
            PdfiumInternalError::FormatError => SplitError::CorruptPdf(file_name.to_string()),
            PdfiumInternalError::FileError => SplitError::FileUnreadable(file_name.to_string()),
            other => SplitError::OpenFailed(file_name.to_string(), format!("{other:?}")),
        },
        PdfiumError::IoError(io) => io_error(&io, file_name),
        other => SplitError::OpenFailed(file_name.to_string(), other.to_string()),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use lopdf::{Document, Object, Stream, dictionary};
    use std::path::PathBuf;

    #[test]
    fn missing_library_gives_a_clear_error() {
        let err = load(Path::new("does-not-exist")).unwrap_err();
        assert!(err.contains("PDFium library is missing"), "{err}");
        assert!(err.contains("npm run fetch-pdfium"), "{err}");
    }

    /// Needs `npm run fetch-pdfium` first; CI runs it before `cargo test`.
    #[test]
    fn loads_the_bundled_library() {
        start_engine();
        let doc = engine().unwrap().create_new_pdf().unwrap();
        assert_eq!(doc.pages().len(), 0);
    }

    fn open(path: &str, password: Option<&str>) -> Result<PdfInfo> {
        open_document(path, password).map(|(info, _)| info)
    }

    pub(crate) fn start_engine() {
        init(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("pdfium")).unwrap();
    }

    /// A PDF with `pages` text pages. The trailer `/ID` is there because encryption needs it.
    pub(crate) fn document(pages: usize) -> Document {
        let mut doc = Document::with_version("1.5");
        doc.trailer.set(
            "ID",
            Object::Array(vec![
                Object::string_literal(b"0123456789abcdef"),
                Object::string_literal(b"fedcba9876543210"),
            ]),
        );
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica",
        });
        let kids: Vec<Object> = (1..=pages)
            .map(|n| {
                let content = format!("BT /F1 12 Tf 100 700 Td (Page {n}) Tj ET");
                let contents_id = doc.add_object(Stream::new(dictionary! {}, content.into_bytes()));
                doc.add_object(dictionary! {
                    "Type" => "Page",
                    "Parent" => pages_id,
                    "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                    "Resources" => dictionary! { "Font" => dictionary! { "F1" => font_id } },
                    "Contents" => contents_id,
                })
                .into()
            })
            .collect();
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => pages as i64 }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);
        doc
    }

    pub(crate) fn encrypted(pages: usize, user_password: &str) -> Document {
        let mut doc = document(pages);
        let state = lopdf::EncryptionState::try_from(lopdf::EncryptionVersion::V2 {
            document: &doc,
            owner_password: "owner-secret",
            user_password,
            key_length: 128,
            permissions: lopdf::Permissions::all(),
        })
        .unwrap();
        doc.encrypt(&state).unwrap();
        doc
    }

    pub(crate) fn save(dir: &tempfile::TempDir, name: &str, mut doc: Document) -> String {
        let path = dir.path().join(name);
        doc.save(&path).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn write(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> String {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path.to_string_lossy().into_owned()
    }

    #[test]
    fn opens_a_normal_pdf() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let path = save(&dir, "report.pdf", document(3));

        let info = open(&path, None).unwrap();
        assert_eq!(info.file_name, "report.pdf");
        assert_eq!(info.page_count, 3);
        assert_eq!(info.size_bytes, std::fs::metadata(&path).unwrap().len());
        assert_eq!(info.path, path);
    }

    #[test]
    fn opening_leaves_the_original_byte_for_byte_unchanged() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let path = save(&dir, "original.pdf", document(2));
        let locked = save(&dir, "locked.pdf", encrypted(2, "secret"));
        let before = (std::fs::read(&path).unwrap(), std::fs::read(&locked).unwrap());

        open(&path, None).unwrap();
        open(&locked, Some("secret")).unwrap();
        let _ = open(&locked, Some("wrong"));

        assert_eq!(before, (std::fs::read(&path).unwrap(), std::fs::read(&locked).unwrap()));
    }

    #[test]
    fn encrypted_pdf_asks_for_a_password_then_opens_with_it() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let path = save(&dir, "locked.pdf", encrypted(4, "secret"));

        assert!(matches!(open(&path, None), Err(SplitError::PasswordRequired)));
        assert!(matches!(open(&path, Some("")), Err(SplitError::PasswordRequired)));
        assert!(matches!(open(&path, Some("wrong")), Err(SplitError::WrongPassword)));
        assert_eq!(open(&path, Some("secret")).unwrap().page_count, 4);
    }

    #[test]
    fn encrypted_pdf_without_a_user_password_opens_directly() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let path = save(&dir, "restricted.pdf", encrypted(2, ""));
        assert_eq!(open(&path, None).unwrap().page_count, 2);
    }

    #[test]
    fn corrupt_and_non_pdf_files_give_clear_errors() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let mut bytes = Vec::new();
        document(3).save_to(&mut bytes).unwrap();
        let truncated = write(&dir, "truncated.pdf", &bytes[..bytes.len() / 3]);
        let text = write(&dir, "notes.pdf", b"just some text, not a PDF");
        let empty = write(&dir, "empty.pdf", b"");

        for path in [&truncated, &text, &empty] {
            let err = open(path, None).unwrap_err();
            assert!(matches!(err, SplitError::CorruptPdf(_)), "{path}: {err}");
            assert!(err.to_string().ends_with("is damaged or is not a PDF"), "{err}");
        }
    }

    #[test]
    fn missing_file_and_folder_give_clear_errors() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("gone.pdf");
        let err = open(&missing.to_string_lossy(), None).unwrap_err();
        assert_eq!(err.to_string(), "The file gone.pdf was not found. It may have been moved or deleted");

        let folder = open(&dir.path().to_string_lossy(), None).unwrap_err();
        assert!(matches!(folder, SplitError::CorruptPdf(_)), "{folder}");
    }

    #[test]
    fn renders_a_page_as_a_jpeg_of_the_requested_width() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let path = save(&dir, "report.pdf", document(2));
        let (_, doc) = open_document(&path, None).unwrap();

        let jpeg = render_page(&doc, 2, 200).unwrap();
        let image = image::load_from_memory_with_format(&jpeg, ImageFormat::Jpeg).unwrap();
        assert_eq!(image.width(), 200);
        // US Letter is 612 x 792 points, so the height keeps the page's aspect ratio.
        assert_eq!(image.height(), 259);
    }

    #[test]
    fn rendering_a_missing_page_gives_a_clear_error() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let (_, doc) = open_document(&save(&dir, "a.pdf", document(1)), None).unwrap();
        let err = render_page(&doc, 5, 100).unwrap_err();
        assert!(err.to_string().starts_with("Page 5 could not be shown"), "{err}");
    }

    /// The interface shows a password prompt by matching these messages.
    #[test]
    fn password_messages_match_the_frontend() {
        let types = include_str!("../../src/types.ts");
        for err in [SplitError::PasswordRequired, SplitError::WrongPassword] {
            assert!(types.contains(&format!("\"{err}\"")), "src/types.ts is missing \"{err}\"");
        }
    }
}
