use crate::ExtractionOutput;
use crate::extractor::{ExtractionError, Extractor};
use crate::normalize::normalize_text;
use camino::Utf8Path;
use mopi_config::ExtractionConfig;
use std::fs;
use std::io::Read;

pub struct TextExtractor;

impl Extractor for TextExtractor {
    fn extract(
        &self,
        path: &Utf8Path,
        config: &ExtractionConfig,
    ) -> Result<ExtractionOutput, ExtractionError> {
        let start = std::time::Instant::now();
        let mut file = fs::File::open(path).map_err(|source| ExtractionError::Io {
            path: path.to_string(),
            source,
        })?;

        let metadata = file.metadata().map_err(|source| ExtractionError::Io {
            path: path.to_string(),
            source,
        })?;

        let file_size = metadata.len();
        if file_size > config.max_file_bytes {
            return Ok(ExtractionOutput {
                path: path.to_owned(),
                mime: "text/plain".to_string(),
                text: String::new(),
                warnings: vec![format!(
                    "File size {} exceeds limit {}",
                    file_size, config.max_file_bytes
                )],
                metadata: crate::ExtractionMetadata {
                    duration_ms: start.elapsed().as_millis() as u64,
                    ..Default::default()
                },
            });
        }

        let mut buffer = Vec::with_capacity(file_size as usize);
        file.read_to_end(&mut buffer)
            .map_err(|source| ExtractionError::Io {
                path: path.to_string(),
                source,
            })?;

        let text = String::from_utf8_lossy(&buffer);
        let mut normalized = normalize_text(&text);

        let mut warnings = Vec::new();
        if normalized.chars().count() > config.max_extracted_chars {
            normalized = normalized
                .chars()
                .take(config.max_extracted_chars)
                .collect::<String>();
            warnings.push(format!(
                "Text truncated to {} characters",
                config.max_extracted_chars
            ));
        }

        Ok(ExtractionOutput {
            path: path.to_owned(),
            mime: "text/plain".to_string(),
            text: normalized,
            warnings,
            metadata: crate::ExtractionMetadata {
                duration_ms: start.elapsed().as_millis() as u64,
                ..Default::default()
            },
        })
    }
}
