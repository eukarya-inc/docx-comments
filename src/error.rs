use std::fmt::{Display, Formatter};
use std::io;

#[derive(Debug)]
pub enum AppError {
    Io(io::Error),
    Zip(zip::result::ZipError),
    Json(serde_json::Error),
    MissingRequiredPart(&'static str),
    MalformedXml { part: &'static str, message: String },
    Cli(String),
}

impl Display for AppError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::Zip(error) => write!(f, "zip error: {error}"),
            Self::Json(error) => write!(f, "JSON error: {error}"),
            Self::MissingRequiredPart(part) => write!(f, "missing required part: {part}"),
            Self::MalformedXml { part, message } => {
                write!(f, "malformed XML in {part}: {message}")
            }
            Self::Cli(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<io::Error> for AppError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<zip::result::ZipError> for AppError {
    fn from(value: zip::result::ZipError) -> Self {
        Self::Zip(value)
    }
}

impl From<serde_json::Error> for AppError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}
