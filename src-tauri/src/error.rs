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
    #[error("{0}")]
    Presets(String),
}

impl Serialize for SplitError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SplitError>;
