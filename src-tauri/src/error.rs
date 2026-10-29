use serde::{Serialize, Serializer};

/// Errors returned to the interface. Messages are shown to the user, so keep them specific (PRD 8).
#[derive(Debug, thiserror::Error)]
pub enum SplitError {
    #[error("{0}")]
    InvalidRanges(String),
    #[error("Invalid pattern: {0}")]
    InvalidPattern(#[from] regex::Error),
    #[error("This PDF has no bookmarks")]
    NoOutline,
    #[error("No split points were found")]
    NothingFound,
    #[error("{0} is not implemented yet")]
    NotImplemented(&'static str),
    #[error("The PDF engine could not start: {0}")]
    PdfEngine(String),
    // Opening a PDF (FR-03, FR-05). The interface matches the two password messages to show a
    // password prompt; `src/types.ts` holds the same text, checked by a test in `pdf.rs`.
    #[error("This PDF is password protected. Enter its password to open it.")]
    PasswordRequired,
    #[error("The password is incorrect. Try again.")]
    WrongPassword,
    #[error("This PDF uses a type of protection Splitr can't open")]
    UnsupportedSecurity,
    #[error("The file {0} was not found. It may have been moved or deleted")]
    FileNotFound(String),
    #[error("Splitr can't read {0}. Check that you have permission to open it")]
    FileUnreadable(String),
    #[error("{0} is damaged or is not a PDF")]
    CorruptPdf(String),
    #[error("{0} has no pages")]
    EmptyPdf(String),
    #[error("{0} could not be opened: {1}")]
    OpenFailed(String, String),
    // Showing pages (FR-10, FR-11).
    #[error("This PDF is no longer open. Open it again to see its pages")]
    NotOpen,
    #[error("Page {0} does not exist. This PDF has {1} pages")]
    PageOutOfRange(u32, u32),
    #[error("Page {0} could not be shown: {1}")]
    RenderFailed(u32, String),
}

impl Serialize for SplitError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SplitError>;
