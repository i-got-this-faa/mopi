use crate::ExtractionOutput;
use crate::extractor::{ExtractionError, Extractor};
use crate::normalize::normalize_text;
use camino::Utf8Path;
use mopi_config::ExtractionConfig;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use std::fs;
use std::io::Read;
use zip::ZipArchive;

pub struct OfficeExtractor;

impl Extractor for OfficeExtractor {
    fn extract(
        &self,
        path: &Utf8Path,
        config: &ExtractionConfig,
    ) -> Result<ExtractionOutput, ExtractionError> {
        let start = std::time::Instant::now();
        let file = fs::File::open(path).map_err(|source| ExtractionError::Io {
            path: path.to_string(),
            source,
        })?;

        let mut archive = ZipArchive::new(file).map_err(|e| ExtractionError::Parse {
            path: path.to_string(),
            reason: format!("Failed to open zip archive: {}", e),
        })?;

        let extension = path.extension().unwrap_or("");
        let (xml_path, mime) = match extension {
            "docx" => (
                "word/document.xml",
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            ),
            "odt" => ("content.xml", "application/vnd.oasis.opendocument.text"),
            _ => {
                return Err(ExtractionError::UnsupportedFormat {
                    path: path.to_string(),
                });
            }
        };

        let mut xml_file = archive
            .by_name(xml_path)
            .map_err(|e| ExtractionError::Parse {
                path: path.to_string(),
                reason: format!("Failed to find {} in archive: {}", xml_path, e),
            })?;

        let mut xml_content = Vec::new();
        xml_file
            .read_to_end(&mut xml_content)
            .map_err(|source| ExtractionError::Io {
                path: path.to_string(),
                source,
            })?;

        let text = if extension == "docx" {
            extract_text_docx(&xml_content)?
        } else {
            extract_text_odt(&xml_content)?
        };

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
            mime: mime.to_string(),
            text: normalized,
            warnings,
            metadata: crate::ExtractionMetadata {
                title: None,
                author: None,
                page_count: None,
                duration_ms: start.elapsed().as_millis() as u64,
            },
        })
    }
}

fn extract_text_docx(xml_content: &[u8]) -> Result<String, ExtractionError> {
    let mut reader = Reader::from_reader(xml_content);
    let mut text = String::new();
    let mut in_text = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"w:t" => in_text = true,
            Ok(Event::End(e)) if e.name().as_ref() == b"w:t" => in_text = false,
            Ok(Event::Start(e)) if e.name().as_ref() == b"w:p" => text.push('\n'),
            Ok(Event::Text(e)) if in_text => {
                text.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ExtractionError::Parse {
                    path: String::from("docx_xml"),
                    reason: format!("XML error: {}", e),
                });
            }
            _ => (),
        }
    }

    Ok(text)
}

fn extract_text_odt(xml_content: &[u8]) -> Result<String, ExtractionError> {
    let mut reader = Reader::from_reader(xml_content);
    let mut text = String::new();
    let mut in_p = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.name().as_ref() == b"text:p" => {
                in_p = true;
                text.push('\n');
            }
            Ok(Event::End(e)) if e.name().as_ref() == b"text:p" => in_p = false,
            Ok(Event::Text(e)) if in_p => {
                text.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ExtractionError::Parse {
                    path: String::from("odt_xml"),
                    reason: format!("XML error: {}", e),
                });
            }
            _ => (),
        }
    }

    Ok(text)
}
