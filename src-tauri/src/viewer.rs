//! The open PDF and its page images (FR-10, FR-11).
//!
//! The document stays loaded while it's open, so each page renders without reloading the file.
//! Rendered pages are cached as JPEGs in a private, per-session folder under the user's cache
//! folder. Each opened PDF gets a fresh subfolder, so pages of one file are never shown for
//! another. The session folder is deleted on exit, and folders left by a crashed session are
//! deleted at the next start (NFR-07). Only the pages the interface asks for are rendered, so a
//! 1,000-page file never renders all at once (NFR-03).

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::Write;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, SystemTime};

use pdfium_render::prelude::PdfDocument;
use tempfile::TempDir;

use crate::error::{Result, SplitError};
use crate::pdf::{self, PdfInfo};
use crate::strategy::DocumentText;

/// Rendered widths are clamped to this range. Heights are capped by `pdf::MAX_RENDER_HEIGHT`, so
/// together they bound the memory one page can use.
pub const MIN_WIDTH: u32 = 32;
pub const MAX_WIDTH: u32 = 2400;

/// Session folders are named `session-<random>` and hold this lock file while the app runs.
const SESSION_PREFIX: &str = "session-";
const LOCK_FILE: &str = "lock";
/// A session folder without a lock file is only treated as stale after this long, so a session
/// that is starting right now is never deleted.
const UNLOCKED_GRACE: Duration = Duration::from_secs(60);

struct OpenPdf {
    path: String,
    page_count: u32,
    document: PdfDocument<'static>,
    /// This PDF's own cache folder. A new one is made for every open.
    cache_dir: PathBuf,
    /// The page text and bookmarks, read once for the rule strategies (#7).
    text: OnceLock<Arc<DocumentText>>,
}

/// The private cache folder of this app session.
struct Session {
    dir: TempDir,
    /// Held open and locked so other instances know this session is alive.
    lock_file: File,
    /// Counts opens, naming each opened PDF's cache folder.
    opened: u64,
}

pub struct Viewer {
    /// The lock is held only to swap or look up the open PDF, never while rendering, so an
    /// open or the exit cleanup never waits for a render to finish.
    current: Mutex<Option<Arc<OpenPdf>>>,
    session: Mutex<Option<Session>>,
    cache_root: PathBuf,
    /// Goes up by one on every cancel and every open. A text extraction stops when it changes.
    extract_generation: AtomicU64,
}

impl Viewer {
    /// A viewer whose session folders live in `cache_root`. The folder is created on first use.
    pub fn new(cache_root: PathBuf) -> Self {
        Self { current: Mutex::new(None), session: Mutex::new(None), cache_root, extract_generation: AtomicU64::new(0) }
    }

    /// Opens the PDF at `path` and makes it the one whose pages are shown (FR-01 to FR-05).
    /// On failure the previously open PDF stays open.
    pub fn open(&self, path: &str, password: Option<&str>) -> Result<PdfInfo> {
        let (info, document) = pdf::open_document(path, password)?;
        let cache_dir = self.new_cache_dir()?;
        let open = Arc::new(OpenPdf {
            path: info.path.clone(),
            page_count: info.page_count,
            document,
            cache_dir,
            text: OnceLock::new(),
        });
        let previous = {
            let mut current = lock(&self.current);
            // Text being read from the previous PDF is no longer needed. Cancelling under the
            // lock means an extraction of the new PDF never sees this cancel.
            self.cancel_extract();
            current.replace(open)
        };
        // Pages of the previous PDF, or of an earlier version of this file, are never read again.
        if let Some(previous) = previous {
            remove_dir(&previous.cache_dir);
        }
        Ok(info)
    }

    /// Page `page` (1-based) of the open PDF at `path`, as a JPEG `width` pixels wide.
    pub fn render(&self, path: &str, page: u32, width: u32) -> Result<Vec<u8>> {
        let width = width.clamp(MIN_WIDTH, MAX_WIDTH);
        let open = lock(&self.current).as_ref().filter(|o| o.path == path).cloned().ok_or(SplitError::NotOpen)?;
        if page == 0 || page > open.page_count {
            return Err(SplitError::PageOutOfRange(page, open.page_count));
        }
        let file = open.cache_dir.join(format!("{page}-{width}.jpg"));
        if let Ok(jpeg) = fs::read(&file) {
            return Ok(jpeg);
        }
        let jpeg = pdf::render_page(&open.document, page, width)?;
        // The cache only saves time, so a failed write still shows the page.
        let _ = write_atomically(&file, &jpeg);
        Ok(jpeg)
    }

    /// The page text and top-level bookmarks of the open PDF at `path` (FR-30 to FR-32). They are
    /// read on the first call and kept while the PDF stays open. `progress` gets the pages done
    /// and the total. Ends with `SplitError::ExtractCancelled` when `cancel_extract` is called
    /// or another PDF is opened.
    pub fn extract(&self, path: &str, mut progress: impl FnMut(u32, u32)) -> Result<Arc<DocumentText>> {
        let (open, generation) = {
            let current = lock(&self.current);
            let open = current.as_ref().filter(|o| o.path == path).cloned().ok_or(SplitError::NotOpen)?;
            (open, self.extract_generation.load(Ordering::SeqCst))
        };
        if let Some(text) = open.text.get() {
            return Ok(text.clone());
        }
        let text = pdf::extract_text(&open.document, |done, total| {
            progress(done, total);
            if self.extract_generation.load(Ordering::SeqCst) == generation {
                ControlFlow::Continue(())
            } else {
                ControlFlow::Break(())
            }
        })?;
        Ok(open.text.get_or_init(|| Arc::new(text)).clone())
    }

    /// Stops every text extraction running now. Later ones are not affected.
    pub fn cancel_extract(&self) {
        self.extract_generation.fetch_add(1, Ordering::SeqCst);
    }

    /// Closes the open PDF and deletes this session's cache folder (NFR-07). Call on exit.
    pub fn close(&self) {
        *lock(&self.current) = None;
        if let Some(session) = lock(&self.session).take() {
            let dir = session.dir.path().to_path_buf();
            drop(session.lock_file);
            if let Err(e) = session.dir.close() {
                eprintln!("Could not delete the thumbnail folder {}: {e}", dir.display());
            }
        }
    }

    /// Deletes session folders in `cache_root` left behind by a crashed or killed app, which
    /// never got to delete its own (NFR-07). Folders of other running instances are kept,
    /// because they still hold their lock. Call at startup.
    pub fn remove_stale_sessions(cache_root: &Path) {
        let Ok(entries) = fs::read_dir(cache_root) else { return };
        for entry in entries.flatten() {
            let dir = entry.path();
            let is_session = entry.file_name().to_string_lossy().starts_with(SESSION_PREFIX);
            if is_session && entry.file_type().is_ok_and(|t| t.is_dir()) && is_stale(&dir) {
                remove_dir(&dir);
            }
        }
    }

    /// Makes an empty cache folder for a newly opened PDF, starting the session if needed.
    fn new_cache_dir(&self) -> Result<PathBuf> {
        let failed = |e: std::io::Error| SplitError::OpenFailed("The page cache".into(), e.to_string());
        let mut guard = lock(&self.session);
        let mut session = match guard.take() {
            Some(session) => session,
            None => start_session(&self.cache_root).map_err(failed)?,
        };
        session.opened += 1;
        let dir = session.dir.path().join(session.opened.to_string());
        let created = fs::create_dir(&dir);
        *guard = Some(session);
        created.map_err(failed)?;
        Ok(dir)
    }
}

/// A panic while rendering leaves no broken state behind, so a poisoned lock is still usable.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// Creates a session folder with a random name that only this user can read (mode 0700 on
/// Unix), and locks its lock file for as long as the app runs.
fn start_session(cache_root: &Path) -> std::io::Result<Session> {
    fs::create_dir_all(cache_root)?;
    let mut builder = tempfile::Builder::new();
    builder.prefix(SESSION_PREFIX);
    // Created with these permissions, so the folder is never readable by others, even briefly.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(fs::Permissions::from_mode(0o700));
    }
    let dir = builder.tempdir_in(cache_root)?;
    let lock = File::create(dir.path().join(LOCK_FILE))?;
    lock.try_lock()?;
    Ok(Session { dir, lock_file: lock, opened: 0 })
}

/// True when no running app holds the session folder's lock.
fn is_stale(dir: &Path) -> bool {
    match OpenOptions::new().write(true).open(dir.join(LOCK_FILE)) {
        Ok(file) => match file.try_lock() {
            Ok(()) => true,
            Err(TryLockError::WouldBlock) => false,
            Err(TryLockError::Error(_)) => false,
        },
        // No lock file: either the session is being created right now, or it crashed before
        // making one. Only an old folder is stale.
        Err(_) => fs::metadata(dir)
            .and_then(|m| m.modified())
            .is_ok_and(|modified| SystemTime::now().duration_since(modified).unwrap_or_default() > UNLOCKED_GRACE),
    }
}

/// Writes to a new temp file and renames it, so a crash never leaves a half-written page
/// (NFR-09). If the same page is being written by another request, this one is skipped.
fn write_atomically(file: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let partial = file.with_extension("part");
    let mut out = OpenOptions::new().write(true).create_new(true).open(&partial)?;
    let written = out.write_all(bytes).and_then(|()| out.sync_all());
    drop(out);
    match written.and_then(|()| fs::rename(&partial, file)) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = fs::remove_file(&partial);
            Err(e)
        }
    }
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
    use crate::pdf::tests::{document, encrypted, outlined, save, start_engine};

    fn viewer(dir: &tempfile::TempDir) -> Viewer {
        start_engine();
        Viewer::new(dir.path().join("cache"))
    }

    fn is_jpeg(bytes: &[u8]) -> bool {
        bytes.starts_with(&[0xFF, 0xD8, 0xFF])
    }

    /// The cache folder of the PDF that is open now.
    fn cache_dir(viewer: &Viewer) -> PathBuf {
        lock(&viewer.current).as_ref().unwrap().cache_dir.clone()
    }

    fn session_dir(viewer: &Viewer) -> PathBuf {
        lock(&viewer.session).as_ref().unwrap().dir.path().to_path_buf()
    }

    #[test]
    fn renders_pages_of_the_open_pdf_and_caches_them() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "report.pdf", document(3));
        assert_eq!(viewer.open(&path, None).unwrap().page_count, 3);

        let first = viewer.render(&path, 3, 120).unwrap();
        assert!(is_jpeg(&first));
        let cached = cache_dir(&viewer).join("3-120.jpg");
        assert_eq!(fs::read(&cached).unwrap(), first);
        assert!(!cached.with_extension("part").exists());

        // A second request is served from the cache.
        fs::write(&cached, b"\xFF\xD8\xFFcached").unwrap();
        assert_eq!(viewer.render(&path, 3, 120).unwrap(), b"\xFF\xD8\xFFcached");
    }

    #[test]
    fn the_session_folder_is_private_and_randomly_named() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "a.pdf", document(1));
        viewer.open(&path, None).unwrap();

        let session = session_dir(&viewer);
        assert_eq!(session.parent().unwrap(), dir.path().join("cache"));
        let name = session.file_name().unwrap().to_string_lossy().into_owned();
        assert!(name.starts_with(SESSION_PREFIX) && name.len() > SESSION_PREFIX.len(), "{name}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(&session).unwrap().permissions().mode() & 0o777, 0o700);
        }
    }

    #[test]
    fn widths_are_clamped() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "a.pdf", document(1));
        viewer.open(&path, None).unwrap();

        viewer.render(&path, 1, 0).unwrap();
        viewer.render(&path, 1, 100_000).unwrap();
        let cache = cache_dir(&viewer);
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
    fn opening_another_pdf_uses_a_fresh_cache_folder() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "a.pdf", document(1));
        viewer.open(&path, None).unwrap();
        viewer.render(&path, 1, 100).unwrap();
        let old = cache_dir(&viewer);
        assert!(old.join("1-100.jpg").is_file());

        viewer.open(&path, None).unwrap();
        let new = cache_dir(&viewer);
        assert_ne!(old, new);
        assert!(!old.exists());
        assert!(!new.join("1-100.jpg").exists());
    }

    #[test]
    fn a_page_left_in_an_old_folder_is_never_shown() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "a.pdf", document(1));
        viewer.open(&path, None).unwrap();
        let old = cache_dir(&viewer);
        viewer.open(&path, None).unwrap();
        // Pretend deleting the old folder failed and it kept a page.
        fs::create_dir_all(&old).unwrap();
        fs::write(old.join("1-100.jpg"), b"stale").unwrap();

        assert!(is_jpeg(&viewer.render(&path, 1, 100).unwrap()));
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
    fn close_deletes_the_session_folder() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "a.pdf", document(1));
        viewer.open(&path, None).unwrap();
        viewer.render(&path, 1, 100).unwrap();
        let session = session_dir(&viewer);
        assert!(session.is_dir());

        viewer.close();
        assert!(!session.exists());
        assert!(matches!(viewer.render(&path, 1, 100), Err(SplitError::NotOpen)));
    }

    #[test]
    fn stale_sessions_are_removed_but_live_ones_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("cache");
        let live = start_session(&root).unwrap();
        let crashed = start_session(&root).unwrap();
        let crashed_dir = crashed.dir.keep();
        drop(crashed.lock_file);
        let other = root.join("not-a-session");
        fs::create_dir_all(&other).unwrap();

        Viewer::remove_stale_sessions(&root);
        assert!(live.dir.path().is_dir());
        assert!(!crashed_dir.exists());
        assert!(other.is_dir());
    }

    #[test]
    fn a_new_session_without_a_lock_file_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("cache");
        let starting = root.join(format!("{SESSION_PREFIX}starting"));
        fs::create_dir_all(&starting).unwrap();

        Viewer::remove_stale_sessions(&root);
        assert!(starting.is_dir());
    }

    #[test]
    fn extracts_the_text_once_and_keeps_it_while_open() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "a.pdf", outlined());
        viewer.open(&path, None).unwrap();

        let mut progress = Vec::new();
        let first = viewer.extract(&path, |done, total| progress.push((done, total))).unwrap();
        assert_eq!(progress, [(1, 4), (2, 4), (3, 4), (4, 4)]);
        assert_eq!(first.pages[2].trim(), "Page 3");
        assert_eq!(first.outline.len(), 2);

        let mut again = 0;
        let second = viewer.extract(&path, |_, _| again += 1).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(again, 0, "the cached text should be reused");
    }

    #[test]
    fn only_the_open_pdf_can_be_extracted() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "a.pdf", document(1));
        assert!(matches!(viewer.extract(&path, |_, _| ()), Err(SplitError::NotOpen)));
        viewer.open(&path, None).unwrap();
        let other = save(&dir, "b.pdf", document(1));
        assert!(matches!(viewer.extract(&other, |_, _| ()), Err(SplitError::NotOpen)));
    }

    #[test]
    fn a_cancelled_extraction_stops_and_a_later_one_runs() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let path = save(&dir, "a.pdf", document(5));
        viewer.open(&path, None).unwrap();

        let result = viewer.extract(&path, |done, _| {
            if done == 2 {
                viewer.cancel_extract();
            }
        });
        assert!(matches!(result, Err(SplitError::ExtractCancelled)));
        assert_eq!(viewer.extract(&path, |_, _| ()).unwrap().pages.len(), 5);
    }

    #[test]
    fn opening_another_pdf_cancels_the_extraction() {
        let dir = tempfile::tempdir().unwrap();
        let viewer = viewer(&dir);
        let first = save(&dir, "a.pdf", document(3));
        let second = save(&dir, "b.pdf", document(2));
        viewer.open(&first, None).unwrap();

        let result = viewer.extract(&first, |done, _| {
            if done == 1 {
                viewer.open(&second, None).unwrap();
            }
        });
        assert!(matches!(result, Err(SplitError::ExtractCancelled)));
        assert_eq!(viewer.extract(&second, |_, _| ()).unwrap().page_count, 2);
    }

    #[test]
    fn a_page_being_written_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("1-100.jpg");
        fs::write(file.with_extension("part"), b"in progress").unwrap();
        assert!(write_atomically(&file, b"jpeg").is_err());
        assert!(!file.exists());

        fs::remove_file(file.with_extension("part")).unwrap();
        write_atomically(&file, b"jpeg").unwrap();
        assert_eq!(fs::read(&file).unwrap(), b"jpeg");
    }
}
