//! The open PDF and its page images (FR-10, FR-11).
//!
//! The document stays loaded while it's open, so each page renders without reloading the file.
//! Rendered pages are cached as JPEGs in a per-session temp folder, emptied when another PDF
//! opens and deleted on exit (NFR-07). Only the pages the interface asks for are rendered, so a
//! 1,000-page file never renders all at once (NFR-03).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use pdfium_render::prelude::PdfDocument;

use crate::error::{Result, SplitError};
use crate::pdf::{self, PdfInfo};

/// Rendered widths are clamped to this range, which bounds the memory one page can use.
pub const MIN_WIDTH: u32 = 32;
pub const MAX_WIDTH: u32 = 2400;

struct OpenPdf {
    path: String,
    page_count: u32,
    document: PdfDocument<'static>,
}

pub struct Viewer {
    current: Mutex<Option<OpenPdf>>,
    cache_dir: PathBuf,
}

impl Viewer {
    /// A viewer whose page cache lives in `cache_dir`. The folder is created on first use.
    pub fn new(cache_dir: PathBuf) -> Self {
        Self { current: Mutex::new(None), cache_dir }
    }

    /// The cache folder for this app session: `splitr-thumbnails-<process id>` in the OS temp folder.
    pub fn session_cache_dir() -> PathBuf {
        std::env::temp_dir().join(format!("splitr-thumbnails-{}", std::process::id()))
    }

    /// Opens the PDF at `path` and makes it the one whose pages are shown (FR-01 to FR-05).
    /// On failure the previously open PDF stays open.
    pub fn open(&self, path: &str, password: Option<&str>) -> Result<PdfInfo> {
        let (info, document) = pdf::open_document(path, password)?;
        let mut current = self.lock();
        *current = Some(OpenPdf { path: info.path.clone(), page_count: info.page_count, document });
        // Pages of the previous PDF, or of an earlier version of this file, must not be shown.
        remove_dir(&self.cache_dir);
        Ok(info)
    }

    /// Page `page` (1-based) of the open PDF at `path`, as a JPEG `width` pixels wide.
    pub fn render(&self, path: &str, page: u32, width: u32) -> Result<Vec<u8>> {
        let width = width.clamp(MIN_WIDTH, MAX_WIDTH);
        let current = self.lock();
        let open = current.as_ref().filter(|o| o.path == path).ok_or(SplitError::NotOpen)?;
        if page == 0 || page > open.page_count {
            return Err(SplitError::PageOutOfRange(page, open.page_count));
        }
        let file = self.cache_dir.join(format!("{page}-{width}.jpg"));
        if let Ok(jpeg) = fs::read(&file) {
            return Ok(jpeg);
        }
        let jpeg = pdf::render_page(&open.document, page, width)?;
        // The cache only saves time, so a failed write still shows the page.
        let _ = write_atomically(&file, &jpeg);
        Ok(jpeg)
    }

    /// Closes the open PDF and deletes the cache folder (NFR-07). Call on exit.
    pub fn close(&self) {
        *self.lock() = None;
        remove_dir(&self.cache_dir);
    }

    /// A panic while rendering leaves no broken state behind, so a poisoned lock is still usable.
    fn lock(&self) -> MutexGuard<'_, Option<OpenPdf>> {
        self.current.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Writes to a temp file and renames it, so a crash never leaves a half-written page (NFR-09).
fn write_atomically(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = file.parent() {
        fs::create_dir_all(dir)?;
    }
    let partial = file.with_extension("part");
    fs::write(&partial, bytes)?;
    fs::rename(&partial, file)
}

fn remove_dir(dir: &Path) {
    if let Err(e) = fs::remove_dir_all(dir)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        eprintln!("Could not delete the thumbnail folder {}: {e}", dir.display());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::tests::{document, encrypted, save, start_engine};

    fn viewer(dir: &tempfile::TempDir) -> Viewer {
        start_engine();
        Viewer::new(dir.path().join("cache"))
    }

    fn is_jpeg(bytes: &[u8]) -> bool {
        bytes.starts_with(&[0xFF, 0xD8, 0xFF])
    }

    #[test]
    fn renders_pages_of_the_open_pdf_and_caches_them() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "report.pdf", document(3));
        assert_eq!(viewer.open(&path, None).unwrap().page_count, 3);

        let first = viewer.render(&path, 3, 120).unwrap();
        assert!(is_jpeg(&first));
        let cached = dir.path().join("cache").join("3-120.jpg");
        assert_eq!(fs::read(&cached).unwrap(), first);

        // A second request is served from the cache.
        fs::write(&cached, b"\xFF\xD8\xFFcached").unwrap();
        assert_eq!(viewer.render(&path, 3, 120).unwrap(), b"\xFF\xD8\xFFcached");
    }

    #[test]
    fn widths_are_clamped() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "a.pdf", document(1));
        viewer.open(&path, None).unwrap();

        viewer.render(&path, 1, 0).unwrap();
        viewer.render(&path, 1, 100_000).unwrap();
        let cache = dir.path().join("cache");
        assert!(cache.join(format!("1-{MIN_WIDTH}.jpg")).is_file());
        assert!(cache.join(format!("1-{MAX_WIDTH}.jpg")).is_file());
    }

    #[test]
    fn pages_outside_the_document_give_clear_errors() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "a.pdf", document(2));
        viewer.open(&path, None).unwrap();

        for page in [0, 3] {
            let err = viewer.render(&path, page, 100).unwrap_err();
            assert!(matches!(err, SplitError::PageOutOfRange(p, 2) if p == page), "{err}");
        }
        assert_eq!(viewer.render(&path, 3, 100).unwrap_err().to_string(), "Page 3 does not exist. This PDF has 2 pages");
    }

    #[test]
    fn only_the_open_pdf_can_be_rendered() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let first = save(&dir, "first.pdf", document(1));
        let second = save(&dir, "second.pdf", document(1));

        assert!(matches!(viewer.render(&first, 1, 100), Err(SplitError::NotOpen)));
        viewer.open(&first, None).unwrap();
        viewer.render(&first, 1, 100).unwrap();
        viewer.open(&second, None).unwrap();
        assert!(matches!(viewer.render(&first, 1, 100), Err(SplitError::NotOpen)));
        viewer.render(&second, 1, 100).unwrap();
    }

    #[test]
    fn opening_another_pdf_empties_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "a.pdf", document(1));
        viewer.open(&path, None).unwrap();
        viewer.render(&path, 1, 100).unwrap();
        let stale = dir.path().join("cache").join("1-100.jpg");
        assert!(stale.is_file());

        viewer.open(&path, None).unwrap();
        assert!(!stale.exists());
    }

    #[test]
    fn a_failed_open_keeps_the_current_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "a.pdf", document(1));
        viewer.open(&path, None).unwrap();

        assert!(viewer.open(&dir.path().join("gone.pdf").to_string_lossy(), None).is_err());
        viewer.render(&path, 1, 100).unwrap();
    }

    #[test]
    fn an_unlocked_pdf_renders_without_its_password_again() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "locked.pdf", encrypted(2, "secret"));
        viewer.open(&path, Some("secret")).unwrap();
        assert!(is_jpeg(&viewer.render(&path, 2, 100).unwrap()));
    }

    #[test]
    fn close_deletes_the_cache_folder() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "a.pdf", document(1));
        viewer.open(&path, None).unwrap();
        viewer.render(&path, 1, 100).unwrap();
        assert!(dir.path().join("cache").is_dir());

        viewer.close();
        assert!(!dir.path().join("cache").exists());
        assert!(matches!(viewer.render(&path, 1, 100), Err(SplitError::NotOpen)));
    }
}
