/// Normalizes text by converting line endings to `\n` and removing NUL bytes.
/// This ensures consistent indexing and prevents issues with some indexers.
pub fn normalize_text(text: &str) -> String {
    // Fast path: no NUL bytes and no \r\n sequences
    if !text.contains('\0') && !text.contains("\r\n") {
        return text.to_string();
    }

    // Remove NUL bytes first, then normalize \r\n -> \n in a single pass
    let filtered: String = text.chars().filter(|&c| c != '\0').collect();
    if filtered.contains("\r\n") {
        filtered.replace("\r\n", "\n")
    } else {
        filtered
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_line_endings() {
        assert_eq!(normalize_text("line1\r\nline2"), "line1\nline2");
        assert_eq!(normalize_text("line1\nline2"), "line1\nline2");
    }

    #[test]
    fn test_remove_nul_bytes() {
        assert_eq!(normalize_text("hello\0world"), "helloworld");
    }

    #[test]
    fn test_combined_normalization() {
        assert_eq!(normalize_text("hello\0\r\nworld"), "hello\nworld");
    }
}
