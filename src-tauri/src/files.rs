//! Saving the small files the app keeps in its config folder (SRS 6.4).

use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Writes `bytes` to a new temp file next to `path` and renames it over `path`, so a failed write
/// keeps the old file. Each write has its own temp file, so writes that overlap can't mix. Creates
/// the folder if it's missing.
pub fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().filter(|dir| !dir.as_os_str().is_empty()).unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let mut temp = tempfile::Builder::new().prefix(".splitr-").suffix(".tmp").tempfile_in(dir)?;
    temp.write_all(bytes)?;
    temp.persist(path).map_err(|e| e.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_the_file_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("a.json");
        write_atomically(&path, b"one").unwrap();
        write_atomically(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn a_failed_write_keeps_what_was_there() {
        let dir = tempfile::tempdir().unwrap();
        // A folder can't be renamed over, like a file another program holds on to.
        let path = dir.path().join("a.json");
        fs::create_dir(&path).unwrap();
        assert!(write_atomically(&path, b"new").is_err());
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
