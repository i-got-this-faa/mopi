use crate::ExtractionOutput;
use camino::Utf8Path;
use lss_config::ExtractionConfig;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    Pdf,
    Zip,
    Xml,
    Office,
    Text,
}

impl std::fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pdf => write!(f, "pdf"),
            Self::Zip => write!(f, "zip"),
            Self::Xml => write!(f, "xml"),
            Self::Office => write!(f, "office"),
            Self::Text => write!(f, "text"),
        }
    }
}

#[derive(Debug, Error)]
pub enum ExtractionError {
    #[error("IO error during extraction from {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Unsupported format for file at {path}")]
    UnsupportedFormat { path: String },
    #[error("Extraction from {path} timed out during {stage}")]
    Timeout { path: String, stage: String },
    #[error("Extraction cap exceeded for {path}: {cap} ({limit})")]
    CapExceeded { path: String, cap: String, limit: u64 },
    #[error("Failed to parse {kind} at {path}: {reason}")]
    Parse {
        path: String,
        kind: ParseErrorKind,
        reason: String,
    },
    #[error("Missing dependency for {path}: {dependency}")]
    MissingDependency { path: String, dependency: String },
}

pub trait Extractor: Send + Sync {
    fn extract(
        &self,
        path: &Utf8Path,
        config: &ExtractionConfig,
    ) -> Result<ExtractionOutput, ExtractionError>;
}
