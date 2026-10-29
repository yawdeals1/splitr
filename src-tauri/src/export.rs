//! Saving parts to the chosen folder (FR-50, FR-52, FR-53, NFR-09).
//!
//! Every part is written to a temp file in the output folder first. Only when all of them are
//! written are they renamed to their final names, and a failure at any point removes whatever
//! this export created, so it never leaves partial files. A ZIP is written the same way: built
//! in a temp file, then renamed.

use std::collections::HashSet;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::error::{Result, SplitError};
use crate::naming;
use crate::pdf;
use crate::ranges::{self, Range};

/// Writes each range of the PDF at `source` as its own file in `folder` (FR-50), named from
/// `names` (one per range, without ".pdf"). Names are sanitized and get a numeric suffix rather
/// than replace an existing file (FR-52). Returns the paths written, in range order.
pub fn export(source: &str, password: Option<&str>, parts: &[Range], folder: &str, names: &[String]) -> Result<Vec<String>> {
    let (pdf, folder) = prepare(source, password, parts, folder, names)?;
    let targets = output_paths(folder, names)?;

    let mut written = Vec::with_capacity(parts.len());
    for range in parts {
        let mut temp = temp_file(folder)?;
        let mut out = BufWriter::new(temp.as_file_mut());
        pdf.write_part(range.start, range.end, &mut out)?;
        out.flush().map_err(|e| SplitError::ExportFailed(e.to_string()))?;
        drop(out);
        temp.as_file().sync_all().map_err(|e| SplitError::ExportFailed(e.to_string()))?;
        written.push(temp);
    }

    // Unrenamed temp files delete themselves when dropped, so an early return cleans them up.
    let mut saved: Vec<PathBuf> = Vec::with_capacity(targets.len());
    for (temp, target) in written.into_iter().zip(targets) {
        match temp.persist_noclobber(&target) {
            Ok(_) => saved.push(target),
            Err(e) => {
                for path in &saved {
                    let _ = fs::remove_file(path);
                }
                return Err(SplitError::ExportFailed(format!("{} ({})", e.error, target.display())));
            }
        }
    }
    Ok(saved.into_iter().map(|p| p.to_string_lossy().into_owned()).collect())
}

/// Like [`export`], but bundles the parts into one ZIP in `folder` named after `zip_name` (FR-53).
/// Entries are stored, not compressed: PDF content is compressed already. Returns the ZIP's path.
pub fn export_zip(
    source: &str,
    password: Option<&str>,
    parts: &[Range],
    folder: &str,
    names: &[String],
    zip_name: &str,
) -> Result<String> {
    let (pdf, folder) = prepare(source, password, parts, folder, names)?;
    let target = zip_path(folder, zip_name)?;
    let fail = |e: &dyn std::fmt::Display| SplitError::ExportFailed(e.to_string());

    // The temp file deletes itself if anything below fails.
    let mut temp = temp_file(folder)?;
    let mut zip = ZipWriter::new(BufWriter::new(temp.as_file_mut()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored).large_file(true);
    let mut taken = HashSet::new();
    for (range, name) in parts.iter().zip(names) {
        let entry = format!("{}.pdf", naming::dedupe(&naming::sanitize(name), &mut taken));
        zip.start_file(entry, options).map_err(|e| fail(&e))?;
        pdf.write_part(range.start, range.end, &mut zip)?;
    }
    zip.finish().map_err(|e| fail(&e))?.flush().map_err(|e| fail(&e))?;
    temp.as_file().sync_all().map_err(|e| fail(&e))?;
    temp.persist_noclobber(&target)
        .map_err(|e| SplitError::ExportFailed(format!("{} ({})", e.error, target.display())))?;
    Ok(target.to_string_lossy().into_owned())
}

/// Checks the request and loads the source, before anything is written.
fn prepare<'a>(
    source: &str,
    password: Option<&str>,
    parts: &[Range],
    folder: &'a str,
    names: &[String],
) -> Result<(pdf::Source, &'a Path)> {
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
    let folder = Path::new(folder);
    if !folder.is_dir() {
        return Err(SplitError::OutputFolderMissing(folder.display().to_string()));
    }
    let pdf = pdf::load_source(source, password)?;
    ranges::validate(parts, pdf.page_count(), true)?;
    Ok((pdf, folder))
}

/// A temp file in `folder`, deleted when dropped unless it is renamed.
fn temp_file(folder: &Path) -> Result<tempfile::NamedTempFile> {
    tempfile::Builder::new()
        .prefix(".splitr-")
        .suffix(".tmp")
        .tempfile_in(folder)
        .map_err(|e| SplitError::ExportFailed(e.to_string()))
}

/// `<zip_name>.zip` in `folder`, with " (2)", " (3)"... if a ZIP of that name exists (FR-52).
fn zip_path(folder: &Path, zip_name: &str) -> Result<PathBuf> {
    let entries = fs::read_dir(folder).map_err(|e| SplitError::ExportFailed(e.to_string()))?;
    let mut taken: HashSet<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            name.strip_suffix(".zip").map(str::to_string)
        })
        .collect();
    let name = naming::dedupe(&naming::sanitize(naming::strip_pdf(zip_name)), &mut taken);
    Ok(folder.join(format!("{name}.zip")))
}

/// The final path of each part: the sanitized name plus ".pdf", with " (2)", " (3)"... added when
/// another part or a PDF already in the folder has that name (FR-52).
fn output_paths(folder: &Path, names: &[String]) -> Result<Vec<PathBuf>> {
    let entries = fs::read_dir(folder).map_err(|e| SplitError::ExportFailed(e.to_string()))?;
    let mut taken: HashSet<String> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_lowercase();
            name.strip_suffix(".pdf").map(str::to_string)
        })
        .collect();
    Ok(names
        .iter()
        .map(|name| folder.join(format!("{}.pdf", naming::dedupe(&naming::sanitize(name), &mut taken))))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SplitError;
    use crate::pdf::tests::{encrypted, outlined, save, start_engine};
    use std::io::Read;

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

        let paths = export(&source, None, &parts, &folder, &["book".into(), "book - 02".into()]).unwrap();

        assert_eq!(listing(dir.path()), ["book (2).pdf", "book - 02.pdf", "book.pdf"]);
        assert_eq!(paths, [dir.path().join("book (2).pdf"), dir.path().join("book - 02.pdf")].map(|p| p.to_string_lossy().into_owned()));
        assert_eq!(fs::read(&source).unwrap(), before);
        for path in &paths {
            assert_eq!(pdf::open(path, None).unwrap().page_count, 2);
        }
    }

    #[test]
    fn a_failed_export_leaves_no_files() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let source = save(&dir, "locked.pdf", encrypted(3, "secret"));
        let out = tempfile::tempdir().unwrap();
        let folder = out.path().to_string_lossy().into_owned();
        let names: Vec<String> = vec!["a".into(), "b".into()];

        let past_the_end = [Range::new(1, 2), Range::new(3, 9)];
        let err = export(&source, Some("secret"), &past_the_end, &folder, &names).unwrap_err();
        assert!(matches!(err, SplitError::InvalidRanges(_)), "{err}");
        let wrong = export(&source, Some("nope"), &[Range::new(1, 1), Range::new(2, 3)], &folder, &names).unwrap_err();
        assert!(matches!(wrong, SplitError::WrongPassword), "{wrong}");
        assert!(listing(out.path()).is_empty());

        export(&source, Some("secret"), &[Range::new(1, 1), Range::new(2, 3)], &folder, &names).unwrap();
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

    /// The entry names in the ZIP at `path`, and the page count of each entry.
    fn zip_contents(path: &str) -> Vec<(String, u32)> {
        let mut archive = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        let unpacked = tempfile::tempdir().unwrap();
        (0..archive.len())
            .map(|i| {
                let mut entry = archive.by_index(i).unwrap();
                let mut bytes = Vec::new();
                entry.read_to_end(&mut bytes).unwrap();
                let file = unpacked.path().join(format!("{i}.pdf"));
                fs::write(&file, bytes).unwrap();
                (entry.name().unwrap().to_string(), pdf::open(&file.to_string_lossy(), None).unwrap().page_count)
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
        let parts = [Range::new(1, 1), Range::new(2, 4)];
        let names = ["book - 01".to_string(), "book - 01".to_string()];

        let zip = export_zip(&source, None, &parts, &folder, &names, "book.pdf").unwrap();
        assert_eq!(zip, out.path().join("book.zip").to_string_lossy());
        assert_eq!(listing(out.path()), ["book.zip"]);
        assert_eq!(zip_contents(&zip), [("book - 01.pdf".to_string(), 1), ("book - 01 (2).pdf".to_string(), 3)]);
        assert_eq!(fs::read(&source).unwrap(), before);

        // A second export never replaces the first ZIP.
        let again = export_zip(&source, None, &parts, &folder, &names, "book.pdf").unwrap();
        assert_eq!(again, out.path().join("book (2).zip").to_string_lossy());
    }

    #[test]
    fn a_failed_zip_leaves_no_files() {
        start_engine();
        let dir = tempfile::tempdir().unwrap();
        let source = save(&dir, "locked.pdf", encrypted(3, "secret"));
        let out = tempfile::tempdir().unwrap();
        let folder = out.path().to_string_lossy().into_owned();
        let names = ["a".to_string(), "b".to_string()];

        let err = export_zip(&source, Some("nope"), &[Range::new(1, 1), Range::new(2, 3)], &folder, &names, "a")
            .unwrap_err();
        assert!(matches!(err, SplitError::WrongPassword), "{err}");
        let err = export_zip(&source, Some("secret"), &[Range::new(1, 1), Range::new(2, 9)], &folder, &names, "a")
            .unwrap_err();
        assert!(matches!(err, SplitError::InvalidRanges(_)), "{err}");
        assert!(listing(out.path()).is_empty());
    }

    #[test]
    fn zip_names_are_safe_and_unique() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Report.ZIP"), b"").unwrap();
        assert_eq!(zip_path(dir.path(), "report.pdf").unwrap(), dir.path().join("report (2).zip"));
        assert_eq!(zip_path(dir.path(), "a:b").unwrap(), dir.path().join("a_b.zip"));
    }

    #[test]
    fn rejects_bad_input_before_reading_the_pdf() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().to_string_lossy().into_owned();
        let one = [Range::new(1, 1)];

        assert!(matches!(export("x.pdf", None, &[], &folder, &[]), Err(SplitError::InvalidRanges(_))));
        assert!(matches!(export("x.pdf", None, &one, &folder, &[]), Err(SplitError::InvalidRanges(_))));
        let missing = dir.path().join("gone").to_string_lossy().into_owned();
        let err = export("x.pdf", None, &one, &missing, &["a".into()]).unwrap_err();
        assert!(matches!(err, SplitError::OutputFolderMissing(_)), "{err}");
    }
}
