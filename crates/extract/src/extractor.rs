use crate::ExtractionOutput;
use camino::Utf8Path;
use lss_config::ExtractionConfig;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExtractionError {
    #[error("IO error during extraction from {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("Failed to parse document at {path}: {reason}")]
    Parse { path: String, reason: String },
    #[error("Extraction from {path} timed out")]
    Timeout { path: String },
    #[error("Unsupported format for file at {path}")]
    UnsupportedFormat { path: String },
}

pub trait Extractor: Send + Sync {
    fn extract(
        &self,
        path: &Utf8Path,
        config: &ExtractionConfig,
    ) -> Result<ExtractionOutput, ExtractionError>;
}
