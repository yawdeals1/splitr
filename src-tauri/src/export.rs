//! Saving parts to the chosen folder (FR-50, FR-52, NFR-09).
//!
//! Every part is written to a temp file in the output folder first. Only when all of them are
//! written are they renamed to their final names, and a failure at any point removes whatever
//! this export created, so it never leaves partial files.

use std::collections::HashSet;
use std::fs;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use crate::error::{Result, SplitError};
use crate::naming;
use crate::pdf;
use crate::ranges::{self, Range};

/// Writes each range of the PDF at `source` as its own file in `folder` (FR-50), named from
/// `names` (one per range, without ".pdf"). Names are sanitized and get a numeric suffix rather
/// than replace an existing file (FR-52). Returns the paths written, in range order.
pub fn export(source: &str, password: Option<&str>, parts: &[Range], folder: &str, names: &[String]) -> Result<Vec<String>> {
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
    let targets = output_paths(folder, names)?;

    let mut written = Vec::with_capacity(parts.len());
    for range in parts {
        let mut temp = tempfile::Builder::new()
            .prefix(".splitr-")
            .suffix(".tmp")
            .tempfile_in(folder)
            .map_err(|e| SplitError::ExportFailed(e.to_string()))?;
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
