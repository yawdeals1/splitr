//! Saving parts to the chosen folder (FR-50, FR-52, FR-53, NFR-09).
//!
//! Every part is written to a temp file in the output folder first. Only when all of them are
//! written are they renamed to their final names, and a failure at any point removes whatever
//! this export created, so it never leaves partial files. A ZIP is written the same way: built in
//! a temp file, then renamed.

use std::collections::HashSet;
use std::fs;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tempfile::NamedTempFile;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::error::{Result, SplitError};
use crate::naming;
use crate::pdf;
use crate::ranges::{self, Range};

/// What to export: which parts of which PDF, where, and under which names (one per part, without
/// ".pdf"). Mirrored by `ExportRequest` in `src/types.ts`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub path: String,
    pub password: Option<String>,
    pub ranges: Vec<Range>,
    pub folder: String,
    pub names: Vec<String>,
}

/// Writes each part as its own file in the folder (FR-50). Names are sanitized and get a numeric
/// suffix rather than replace an existing file (FR-52). Returns the paths written, in part order.
pub fn export(request: &ExportRequest) -> Result<Vec<String>> {
    let (pdf, folder) = prepare(request)?;
    let targets = output_paths(folder, &request.names)?;

    let mut written = Vec::with_capacity(request.ranges.len());
    for range in &request.ranges {
        let mut temp = temp_file(folder)?;
        let mut out = BufWriter::new(temp.as_file_mut());
        pdf.write_part(range.start, range.end, &mut out)?;
        out.flush().map_err(write_failed)?;
        drop(out);
        temp.as_file().sync_all().map_err(write_failed)?;
        written.push(temp);
    }
    let saved = persist_all(written, targets)?;
    Ok(saved.into_iter().map(|p| p.to_string_lossy().into_owned()).collect())
}

/// Like [`export`], but bundles the parts into one ZIP in the folder, named after `zip_name`
/// (FR-53). Entries are stored, not compressed: PDF content is compressed already. Returns the
/// ZIP's path.
pub fn export_zip(request: &ExportRequest, zip_name: &str) -> Result<String> {
    let (pdf, folder) = prepare(request)?;
    let mut taken = taken_names(folder, "zip")?;
    let name = naming::dedupe(&naming::sanitize(naming::strip_pdf(zip_name)), &mut taken);
    let target = folder.join(format!("{name}.zip"));

    // The temp file deletes itself if anything below fails.
    let mut temp = temp_file(folder)?;
    let mut zip = ZipWriter::new(BufWriter::new(temp.as_file_mut()));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .large_file(true)
        .last_modified_time(now());
    let mut entries = HashSet::new();
    for (range, name) in request.ranges.iter().zip(&request.names) {
        let entry = format!("{}.pdf", naming::dedupe(&naming::sanitize(name), &mut entries));
        zip.start_file(entry, options).map_err(zip_failed)?;
        pdf.write_part(range.start, range.end, &mut zip)?;
    }
    zip.finish().map_err(zip_failed)?.flush().map_err(write_failed)?;
    temp.as_file().sync_all().map_err(write_failed)?;
    let saved = persist_all(vec![temp], vec![target])?;
    Ok(saved[0].to_string_lossy().into_owned())
}

/// Checks the request and loads the source, before anything is written.
fn prepare(request: &ExportRequest) -> Result<(pdf::Source, &Path)> {
    let (parts, names) = (&request.ranges, &request.names);
    if parts.is_empty() {
        return Err(SplitError::InvalidRanges("There are no parts to save".into()));
    }
    if names.len() != parts.len() {
        return Err(SplitError::InvalidRanges(format!(
            "{} parts were given {} file names",
            parts.len(),
            names.len()
        )));
    }
    let folder = Path::new(&request.folder);
    if !folder.is_dir() {
        return Err(SplitError::OutputFolderMissing(folder.display().to_string()));
    }
    let pdf = pdf::load_source(&request.path, request.password.as_deref())?;
    ranges::validate(parts, pdf.page_count(), true)?;
    Ok((pdf, folder))
}

/// A temp file in `folder`, deleted when dropped unless it is renamed.
fn temp_file(folder: &Path) -> Result<NamedTempFile> {
    tempfile::Builder::new().prefix(".splitr-").suffix(".tmp").tempfile_in(folder).map_err(write_failed)
}

/// Renames each temp file to its target, never replacing a file. If one rename fails, the files
/// already renamed are removed and the remaining temp files delete themselves (NFR-09).
fn persist_all(written: Vec<NamedTempFile>, targets: Vec<PathBuf>) -> Result<Vec<PathBuf>> {
    let mut saved: Vec<PathBuf> = Vec::with_capacity(targets.len());
    for (temp, target) in written.into_iter().zip(targets) {
        match temp.persist_noclobber(&target) {
            Ok(_) => saved.push(target),
            Err(e) => {
                for path in &saved {
                    let _ = fs::remove_file(path);
                }
                let file = target.file_name().unwrap_or_default().to_string_lossy();
                return Err(SplitError::ExportFailed(format!("{} ({file})", reason(&e.error))));
            }
        }
    }
    Ok(saved)
}

/// The final path of each part: the sanitized name plus ".pdf", with " (2)", " (3)"... added when
/// another part or a PDF already in the folder has that name (FR-52).
fn output_paths(folder: &Path, names: &[String]) -> Result<Vec<PathBuf>> {
    let mut taken = taken_names(folder, "pdf")?;
    Ok(names
        .iter()
        .map(|name| folder.join(format!("{}.pdf", naming::dedupe(&naming::sanitize(name), &mut taken))))
        .collect())
}

/// The names, in lower case and without the extension, of the files in `folder` that end in
/// `.<extension>` in any letter case.
fn taken_names(folder: &Path, extension: &str) -> Result<HashSet<String>> {
    let suffix = format!(".{extension}");
    let entries = fs::read_dir(folder).map_err(write_failed)?;
    Ok(entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            name.strip_suffix(&suffix).map(str::to_string)
        })
        .collect())
}

/// The time stamped on ZIP entries. ZIP stores local time; 1980-01-01 if it can't be stored.
fn now() -> zip::DateTime {
    zip::DateTime::try_from(chrono::Local::now().naive_local()).unwrap_or_default()
}

fn write_failed(e: io::Error) -> SplitError {
    SplitError::ExportFailed(reason(&e))
}

fn zip_failed(e: zip::result::ZipError) -> SplitError {
    match e {
        zip::result::ZipError::Io(e) => write_failed(e),
        other => SplitError::ExportFailed(format!("the ZIP file could not be built ({other})")),
    }
}

/// A readable reason for a file error, without the OS error code (the user sees it).
fn reason(e: &io::Error) -> String {
    match e.kind() {
        io::ErrorKind::PermissionDenied => "Splitr isn't allowed to write to this folder. Choose another folder".into(),
        io::ErrorKind::StorageFull => "the disk is full. Free some space or choose another folder".into(),
        io::ErrorKind::NotFound => "the folder was moved or deleted while saving".into(),
        io::ErrorKind::AlreadyExists => "a file with the same name appeared while saving. Try again".into(),
        io::ErrorKind::ReadOnlyFilesystem => "this drive is read-only. Choose another folder".into(),
        _ => {
            let text = e.to_string();
            match text.find(" (os error ") {
                Some(at) => text[..at].to_string(),
                None => text,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SplitError;
    use crate::pdf::tests::{encrypted, outlined, save, start_engine};
    use std::io::Read;

    fn request(source: &str, password: Option<&str>, parts: &[Range], folder: &str, names: &[&str]) -> ExportRequest {
        ExportRequest {
            path: source.into(),
            password: password.map(String::from),
            ranges: parts.to_vec(),
            folder: folder.into(),
            names: names.iter().map(|n| n.to_string()).collect(),
        }
    }

    fn listing(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        names
    }

    #[test]
    fn saves_each_part_and_leaves_the_original_unchanged() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let source = save(&dir, "book.pdf", outlined());
        let before = fs::read(&source).unwrap();
        let folder = dir.path().to_string_lossy().into_owned();
        let parts = [Range::new(1, 2), Range::new(3, 4)];

        let paths = export(&request(&source, None, &parts, &folder, &["book", "book - 02"])).unwrap();

        assert_eq!(listing(dir.path()), ["book (2).pdf", "book - 02.pdf", "book.pdf"]);
        assert_eq!(paths, [dir.path().join("book (2).pdf"), dir.path().join("book - 02.pdf")].map(|p| p.to_string_lossy().into_owned()));
        assert_eq!(fs::read(&source).unwrap(), before);
        for path in &paths {
            assert_eq!(pdf::open_document(path, None).unwrap().0.page_count, 2);
        }
    }

    #[test]
    fn a_failed_export_leaves_no_files() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let source = save(&dir, "locked.pdf", encrypted(3, "secret"));
        let out = tempfile::tempdir().unwrap();
        let folder = out.path().to_string_lossy().into_owned();
        let names = ["a", "b"];

        let past_the_end = [Range::new(1, 2), Range::new(3, 9)];
        let err = export(&request(&source, Some("secret"), &past_the_end, &folder, &names)).unwrap_err();
        assert!(matches!(err, SplitError::InvalidRanges(_)), "{err}");
        let parts = [Range::new(1, 1), Range::new(2, 3)];
        let wrong = export(&request(&source, Some("nope"), &parts, &folder, &names)).unwrap_err();
        assert!(matches!(wrong, SplitError::WrongPassword), "{wrong}");
        assert!(listing(out.path()).is_empty());

        export(&request(&source, Some("secret"), &parts, &folder, &names)).unwrap();
        assert_eq!(listing(out.path()), ["a.pdf", "b.pdf"]);
    }

    #[test]
    fn names_avoid_existing_files_and_each_other() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Book - 01.PDF"), b"").unwrap();
        fs::write(dir.path().join("notes.txt"), b"").unwrap();
        let names = ["Book - 01", "Book - 02", "book - 02", "a/b", "notes"].map(String::from);

        let paths = output_paths(dir.path(), &names).unwrap();
        let files: Vec<_> = paths.iter().map(|p| p.file_name().unwrap().to_string_lossy().into_owned()).collect();
        assert_eq!(files, ["Book - 01 (2).pdf", "Book - 02.pdf", "book - 02 (2).pdf", "a_b.pdf", "notes.pdf"]);
        assert!(paths.iter().all(|p| p.parent() == Some(dir.path())));
    }

    #[test]
    fn rejects_bad_input_before_reading_the_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().to_string_lossy().into_owned();
        let one = [Range::new(1, 1)];

        assert!(matches!(export(&request("x.pdf", None, &[], &folder, &[])), Err(SplitError::InvalidRanges(_))));
        assert!(matches!(export(&request("x.pdf", None, &one, &folder, &[])), Err(SplitError::InvalidRanges(_))));
        let missing = dir.path().join("gone").to_string_lossy().into_owned();
        let err = export(&request("x.pdf", None, &one, &missing, &["a"])).unwrap_err();
        assert!(matches!(err, SplitError::OutputFolderMissing(_)), "{err}");
    }

    /// The entry names in the ZIP at `path`, and the page count of each entry.
    fn zip_contents(path: &str) -> Vec<(String, u32)> {
        let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        let unpacked = tempfile::tempdir().unwrap();
        (0..archive.len())
            .map(|i| {
                let mut entry = archive.by_index(i).unwrap();
                assert!(entry.last_modified().unwrap().year() > 1980, "entries carry the export time");
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).unwrap();
                let file = unpacked.path().join(format!("{i}.pdf"));
                fs::write(&file, bytes).unwrap();
                let pages = pdf::open_document(&file.to_string_lossy(), None).unwrap().0.page_count;
                (entry.name().unwrap().to_string(), pages)
            })
            .collect()
    }

    #[test]
    fn zips_the_parts_into_one_file() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let source = save(&dir, "book.pdf", outlined());
        let before = fs::read(&source).unwrap();
        let out = tempfile::tempdir().unwrap();
        let folder = out.path().to_string_lossy().into_owned();
        let job = request(&source, None, &[Range::new(1, 1), Range::new(2, 4)], &folder, &["book - 01", "book - 01"]);

        let zip = export_zip(&job, "book.pdf").unwrap();
        assert_eq!(zip, out.path().join("book.zip").to_string_lossy());
        assert_eq!(listing(out.path()), ["book.zip"]);
        assert_eq!(zip_contents(&zip), [("book - 01.pdf".to_string(), 1), ("book - 01 (2).pdf".to_string(), 3)]);
        assert_eq!(fs::read(&source).unwrap(), before);

        // A second export never replaces the first ZIP, whatever its letter case.
        fs::rename(&zip, out.path().join("Book.ZIP")).unwrap();
        let again = export_zip(&job, "book.pdf").unwrap();
        assert_eq!(again, out.path().join("book (2).zip").to_string_lossy());
    }

    #[test]
    fn a_failed_zip_leaves_no_files() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let source = save(&dir, "locked.pdf", encrypted(3, "secret"));
        let out = tempfile::tempdir().unwrap();
        let folder = out.path().to_string_lossy().into_owned();
        let names = ["a", "b"];

        let parts = [Range::new(1, 1), Range::new(2, 3)];
        let err = export_zip(&request(&source, Some("nope"), &parts, &folder, &names), "a").unwrap_err();
        assert!(matches!(err, SplitError::WrongPassword), "{err}");
        let past_the_end = [Range::new(1, 1), Range::new(2, 9)];
        let err = export_zip(&request(&source, Some("secret"), &past_the_end, &folder, &names), "a").unwrap_err();
        assert!(matches!(err, SplitError::InvalidRanges(_)), "{err}");
        assert!(listing(out.path()).is_empty());
    }

    #[test]
    fn a_failed_rename_removes_the_files_already_saved() {
        let dir = tempfile::tempdir().unwrap();
        let temps: Vec<_> = (0..3).map(|_| temp_file(dir.path()).unwrap()).collect();
        // The second target appears after the names were chosen, so its rename fails.
        fs::write(dir.path().join("b.pdf"), b"theirs").unwrap();
        let targets = ["a.pdf", "b.pdf", "c.pdf"].map(|n| dir.path().join(n)).to_vec();

        let err = persist_all(temps, targets).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("(b.pdf)") && !message.contains("os error"), "{message}");
        assert_eq!(listing(dir.path()), ["b.pdf"]);
        assert_eq!(fs::read(dir.path().join("b.pdf")).unwrap(), b"theirs");
    }

    #[test]
    fn zip_names_are_safe_and_unique() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let source = save(&dir, "src.pdf", outlined());
        let out = tempfile::tempdir().unwrap();
        let folder = out.path().to_string_lossy().into_owned();
        fs::write(out.path().join("Report.ZIP"), b"").unwrap();
        let job = request(&source, None, &[Range::new(1, 4)], &folder, &["x"]);

        assert_eq!(export_zip(&job, "report.pdf").unwrap(), out.path().join("report (2).zip").to_string_lossy());
        assert_eq!(export_zip(&job, "a:b").unwrap(), out.path().join("a_b.zip").to_string_lossy());
    }

    #[test]
    fn file_errors_read_without_os_codes() {
        let denied = io::Error::from_raw_os_error(if cfg!(windows) { 5 } else { 13 });
        assert_eq!(reason(&denied), "Splitr isn't allowed to write to this folder. Choose another folder");
        assert_eq!(reason(&io::Error::other("disk on fire")), "disk on fire");
        let coded = io::Error::from_raw_os_error(if cfg!(windows) { 1117 } else { 5 });
        assert!(!reason(&coded).contains("os error"), "{}", reason(&coded));
    }
}
