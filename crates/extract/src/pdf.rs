use crate::ExtractionOutput;
use crate::extractor::{ExtractionError, Extractor};
use crate::normalize::normalize_text;
use camino::Utf8Path;
use lopdf::Document;
use mopi_config::ExtractionConfig;

pub struct PdfExtractor;

impl Extractor for PdfExtractor {
    fn extract(
        &self,
        path: &Utf8Path,
        config: &ExtractionConfig,
    ) -> Result<ExtractionOutput, ExtractionError> {
        let start = std::time::Instant::now();
        let doc = Document::load(path).map_err(|e| ExtractionError::Parse {
            path: path.to_string(),
            reason: format!("Failed to load PDF: {}", e),
        })?;

        let mut text = String::new();
        let pages = doc.get_pages();
        let mut warnings = Vec::new();

        for (page_count, (page_num, _page_id)) in pages.iter().enumerate() {
            if page_count >= config.max_pdf_pages as usize {
                warnings.push(format!("PDF truncated to {} pages", config.max_pdf_pages));
                break;
            }

            match doc.extract_text(&[*page_num]) {
                Ok(page_text) => {
                    text.push_str(&page_text);
                    text.push('\n');
                }
                Err(e) => {
                    warnings.push(format!(
                        "Failed to extract text from page {}: {}",
                        page_num, e
                    ));
                }
            }
            if text.chars().count() > config.max_extracted_chars {
                warnings.push(format!(
                    "Text truncated to {} characters",
                    config.max_extracted_chars
                ));
                break;
            }
        }

        let mut normalized = normalize_text(&text);
        if normalized.chars().count() > config.max_extracted_chars {
            normalized = normalized
                .chars()
                .take(config.max_extracted_chars)
                .collect::<String>();
        }

        Ok(ExtractionOutput {
            path: path.to_owned(),
            mime: "application/pdf".to_string(),
            text: normalized,
            warnings,
            metadata: crate::ExtractionMetadata {
                title: None,
                author: None,
                page_count: Some(pages.len() as u32),
                duration_ms: start.elapsed().as_millis() as u64,
            },
        })
    }
}
