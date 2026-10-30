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
    #[error("This PDF has no text layer (it may be scanned), so Splitr can't split it by its text yet")]
    NoTextLayer,
    #[error("{0} is not implemented yet")]
    NotImplemented(&'static str),
    #[error("The PDF engine could not start: {0}")]
    PdfEngine(String),
    #[error("{0}")]
    Presets(String),
    #[error("Could not load settings: {0}")]
    SettingsLoad(String),
    #[error("Could not save settings: {0}")]
    SettingsSave(String),
    // API keys (FR-60, NFR-05). Messages never include the key.
    #[error("{0}")]
    ApiKey(String),
    // Calls to the AI provider (#15). Messages never include the key.
    #[error("{0}")]
    Ai(String),
    #[error("The AI request was cancelled")]
    AiCancelled,
    // An AI answer still unusable after one retry (FR-36): pages, then the problem.
    #[error("The AI's answer for pages {0} to {1} could not be used: {2}. Set the parts by hand instead")]
    AiReply(u32, u32, String),
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
    // Reading page text and bookmarks for the rule strategies (#7, NFR-02).
    #[error("The text of page {0} could not be read: {1}")]
    TextFailed(u32, String),
    #[error("Splitr could not read the PDF's text: {0}")]
    ExtractFailed(String),
    #[error("Reading the PDF's text was cancelled")]
    ExtractCancelled,
    // Saving parts (FR-50, NFR-09). A failed export removes everything it wrote.
    #[error("Splitr can't write parts of {0}: {1}")]
    CantSplit(String, String),
    #[error("The folder {0} was not found. Choose another folder")]
    OutputFolderMissing(String),
    #[error("The parts could not be saved, so no files were written: {0}")]
    ExportFailed(String),
}

impl Serialize for SplitError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, SplitError>;
