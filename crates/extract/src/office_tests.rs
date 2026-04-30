#[cfg(test)]
mod tests {
    use crate::extractor::{ExtractionError, Extractor};
    use crate::office::OfficeExtractor;
    use camino::Utf8PathBuf;
    use mopi_config::ExtractionConfig;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_malformed_office_document() {
        let dir = tempdir().expect("temp dir should exist");
        let path =
            Utf8PathBuf::from_path_buf(dir.path().join("malformed.docx")).expect("valid path");
        // Create an invalid zip file
        fs::write(&path, b"not a zip file content").expect("write should succeed");

        let extractor = OfficeExtractor;
        let config = ExtractionConfig::default();

        let result = extractor.extract(&path, &config);
        assert!(result.is_err());
        match result {
            Err(ExtractionError::Parse { reason, .. }) => {
                assert!(reason.contains("Failed to open zip archive"));
            }
            _ => panic!("Expected Parse error"),
        }
    }
}
