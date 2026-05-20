use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chunk {
    pub text: String,
    pub start_char: usize,
    pub end_char: usize,
}

pub struct Chunker {
    pub max_chars: usize,
    pub overlap_chars: usize,
}

impl Default for Chunker {
    fn default() -> Self {
        Self {
            max_chars: 1000,
            overlap_chars: 200,
        }
    }
}

impl Chunker {
    pub fn chunk(&self, text: &str) -> Vec<Chunk> {
        if text.is_empty() {
            return Vec::new();
        }

        let mut chunks = Vec::new();
        let chars: Vec<char> = text.chars().collect();
        let mut start = 0;

        while start < chars.len() {
            let end = (start + self.max_chars).min(chars.len());

            // Try to find a nice boundary like a newline if we are not at the end
            let mut break_point = end;
            if end < chars.len() {
                // look backwards from end up to overlap_chars for a newline or space
                let search_start = end.saturating_sub(self.overlap_chars).max(start + 1);
                if let Some(pos) = chars[search_start..end].iter().rposition(|&c| c == '\n') {
                    break_point = search_start + pos + 1; // include the newline
                } else if let Some(pos) = chars[search_start..end].iter().rposition(|&c| c == ' ') {
                    break_point = search_start + pos + 1; // break at space
                }
            }

            let chunk_text: String = chars[start..break_point].iter().collect();
            chunks.push(Chunk {
                text: chunk_text,
                start_char: start,
                end_char: break_point,
            });

            if break_point == chars.len() {
                break;
            }

            // advance start with overlap, but always move forward
            let next_start = break_point.saturating_sub(self.overlap_chars);
            start = next_start.max(start + 1);
        }

        chunks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_returns_no_chunks() {
        let chunker = Chunker::default();
        assert!(chunker.chunk("").is_empty());
    }

    #[test]
    fn short_text_fits_in_single_chunk() {
        let chunker = Chunker {
            max_chars: 100,
            overlap_chars: 20,
        };
        let chunks = chunker.chunk("hello world");
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "hello world");
        assert_eq!(chunks[0].start_char, 0);
        assert_eq!(chunks[0].end_char, 11);
    }

    #[test]
    fn long_text_splits_at_max_chars() {
        let chunker = Chunker {
            max_chars: 10,
            overlap_chars: 0,
        };
        let text = "abcdefghijKLMNOPQRST";
        let chunks = chunker.chunk(text);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].text, "abcdefghij");
        assert_eq!(chunks[1].text, "KLMNOPQRST");
    }

    #[test]
    fn splits_at_newline_boundary() {
        let chunker = Chunker {
            max_chars: 20,
            overlap_chars: 5,
        };
        let text = "first line\nsecond line\nthird line";
        let chunks = chunker.chunk(text);
        assert!(chunks.len() >= 2);
        assert!(chunks[0].text.len() <= 20);
        assert!(chunks[0].text.contains('\n'));
    }

    #[test]
    fn splits_at_space_boundary_when_no_newline() {
        let chunker = Chunker {
            max_chars: 15,
            overlap_chars: 5,
        };
        let text = "hello world foo bar";
        let chunks = chunker.chunk(text);
        assert!(chunks.len() >= 2);
        assert!(chunks[0].text.ends_with(' '));
        assert_eq!(chunks[0].text, "hello world ");
    }

    #[test]
    fn overlap_preserves_context() {
        let chunker = Chunker {
            max_chars: 10,
            overlap_chars: 3,
        };
        let text = "abcdefghijKLMNOPQRST";
        let chunks = chunker.chunk(text);
        assert!(chunks.len() >= 2);
        let first_end = chunks[0].end_char;
        let second_start = chunks[1].start_char;
        assert!(second_start < first_end, "overlap should exist");
        assert_eq!(first_end - second_start, 3);
    }

    #[test]
    fn long_single_word_forces_hard_break() {
        let chunker = Chunker {
            max_chars: 10,
            overlap_chars: 2,
        };
        let text = "superlongwordwithoutspaces";
        let chunks = chunker.chunk(text);
        assert!(!chunks.is_empty());
        for chunk in &chunks {
            assert!(!chunk.text.is_empty());
            assert!(chunk.text.len() <= 12);
        }
        let last = chunks.last().expect("should have at least one chunk");
        assert!(text.ends_with(&last.text[last.text.len().saturating_sub(10)..]));
    }

    #[test]
    fn chunks_cover_full_text_without_gaps() {
        let chunker = Chunker {
            max_chars: 50,
            overlap_chars: 10,
        };
        let text = "The quick brown fox jumps over the lazy dog. Another sentence here for testing purposes.";
        let chunks = chunker.chunk(text);
        assert!(!chunks.is_empty());
        assert_eq!(chunks[0].start_char, 0);
        let last = chunks.last().expect("should have at least one chunk");
        assert_eq!(last.end_char, text.len());
    }

    #[test]
    fn always_advance_guard_prevents_infinite_loop() {
        let chunker = Chunker {
            max_chars: 5,
            overlap_chars: 10,
        };
        let text = "abc";
        let chunks = chunker.chunk(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "abc");
    }

    #[test]
    fn exact_max_chars_no_boundary_search() {
        let chunker = Chunker {
            max_chars: 11,
            overlap_chars: 3,
        };
        let text = "hello world";
        let chunks = chunker.chunk(text);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "hello world");
    }
}
