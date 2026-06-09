pub mod extractor;
pub mod normalize;
pub mod text;

use crate::extractor::{ExtractionError, Extractor};
use crate::text::TextExtractor;
use camino::{Utf8Path, Utf8PathBuf};
use lss_config::ExtractionConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExtractionWarningCode {
    FileTooLarge,
    TruncatedBytes,
    TruncatedPages,
    TruncatedChars,
    PartialPageFailure,
    ArchiveEntrySkipped,
    InvalidXmlRecovered,
    OcrAttempted,
    OcrUnavailable,
    OcrTimedOut,
    OcrLowConfidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionWarning {
    pub code: ExtractionWarningCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractionOutput {
    pub path: Utf8PathBuf,
    pub mime: String,
    pub text: String,
    pub warnings: Vec<ExtractionWarning>,
    pub metadata: ExtractionMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ExtractionMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub page_count: Option<u32>,
    pub duration_ms: u64,
}

impl ExtractionOutput {
    #[must_use]
    pub fn empty(path: Utf8PathBuf, mime: impl Into<String>) -> Self {
        Self {
            path,
            mime: mime.into(),
            text: String::new(),
            warnings: Vec::new(),
            metadata: ExtractionMetadata::default(),
        }
    }
}

pub struct Dispatcher {
    text: TextExtractor,
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}

impl Dispatcher {
    #[must_use]
    pub fn new() -> Self {
        Self {
            text: TextExtractor,
        }
    }

    pub fn extract(
        &self,
        path: &Utf8Path,
        config: &ExtractionConfig,
    ) -> Result<ExtractionOutput, ExtractionError> {
        let extension = path.extension().unwrap_or("").to_lowercase();

        match extension.as_str() {
            _ => {
                // Fallback to text if it's a known config format or if sniffing suggests text
                if is_text_format(&extension, config) {
                    self.text.extract(path, config)
                } else {
                    // Try to sniff the mime type
                    let mime = mime_guess::from_path(path).first_or_octet_stream();
                    if mime.type_() == "text" {
                        self.text.extract(path, config)
                    } else {
                        Err(ExtractionError::UnsupportedFormat {
                            path: path.to_string(),
                        })
                    }
                }
            }
        }
    }
}

fn is_text_format(extension: &str, config: &ExtractionConfig) -> bool {
    if !config.enable_plain_text && !config.enable_configs {
        return false;
    }

    let config_formats = [
        "toml", "yaml", "yml", "json", "jsonc", "ini", "env", "xml", "md",
    ];
    let text_formats = [
        "txt", "rs", "py", "js", "ts", "c", "cpp", "h", "hpp", "go", "java",
    ];

    (config.enable_configs && config_formats.contains(&extension))
        || (config.enable_plain_text && (extension.is_empty() || text_formats.contains(&extension)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lss_config::AppConfig;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_dispatch_text_file() {
        let dir = tempdir().expect("temp dir should be created");
        let path = Utf8PathBuf::from_path_buf(dir.path().join("test.txt"))
            .expect("temp path should be valid UTF-8");
        fs::write(&path, "hello world").expect("text fixture should be written");

        let config = AppConfig::default().extraction;
        let dispatcher = Dispatcher::new();
        let output = dispatcher
            .extract(&path, &config)
            .expect("text extraction should succeed");

        assert_eq!(output.mime, "text/plain");
        assert_eq!(output.text, "hello world");
    }

    #[test]
    fn test_dispatch_json_file() {
        let dir = tempdir().expect("temp dir should be created");
        let path = Utf8PathBuf::from_path_buf(dir.path().join("test.json"))
            .expect("temp path should be valid UTF-8");
        fs::write(&path, "{\"a\": 1}").expect("json fixture should be written");

        let config = AppConfig::default().extraction;
        let dispatcher = Dispatcher::new();
        let output = dispatcher
            .extract(&path, &config)
            .expect("json extraction should succeed");

        assert_eq!(output.mime, "text/plain");
        assert_eq!(output.text, "{\"a\": 1}");
    }

    #[test]
    fn test_unsupported_format() {
        let dir = tempdir().expect("temp dir should be created");
        let path = Utf8PathBuf::from_path_buf(dir.path().join("test.exe"))
            .expect("temp path should be valid UTF-8");
        fs::write(&path, "binary").expect("binary fixture should be written");

        let config = AppConfig::default().extraction;
        let dispatcher = Dispatcher::new();
        let result = dispatcher.extract(&path, &config);

        assert!(matches!(
            result,
            Err(ExtractionError::UnsupportedFormat { .. })
        ));
    }
}
