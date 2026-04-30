#[cfg(test)]
mod tests {
    use crate::extractor::{ExtractionError, Extractor};
    use crate::pdf::PdfExtractor;
    use camino::Utf8PathBuf;
    use mopi_config::ExtractionConfig;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_malformed_pdf_document() {
        let dir = tempdir().expect("temp dir should exist");
        let path =
            Utf8PathBuf::from_path_buf(dir.path().join("malformed.pdf")).expect("valid path");
        // Create an invalid pdf file
        fs::write(&path, b"not a pdf file content").expect("write should succeed");

        let extractor = PdfExtractor;
        let config = ExtractionConfig::default();

        let result = extractor.extract(&path, &config);
        assert!(result.is_err());
        match result {
            Err(ExtractionError::Parse { reason, .. }) => {
                assert!(reason.contains("Failed to load PDF"));
            }
            _ => panic!("Expected Parse error"),
        }
    }
}
