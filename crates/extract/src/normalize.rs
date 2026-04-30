/// Normalizes text by converting line endings to `\n` and removing NUL bytes.
/// This ensures consistent indexing and prevents issues with some indexers.
pub fn normalize_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());

    for c in text.chars() {
        if c == '\0' {
            continue;
        }
        normalized.push(c);
    }

    // Normalize line endings (\r\n -> \n)
    normalized.replace("\r\n", "\n")
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
