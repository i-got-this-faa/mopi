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

        // Pre-compute char offsets: byte position for each character index.
        // This is a Vec<usize> (8 bytes/entry) vs Vec<char> (4 bytes/entry),
        // but avoids the re-encode cost when collecting chunk strings.
        let offsets: Vec<usize> = text.char_indices().map(|(i, _)| i).collect();
        let total_chars = offsets.len();
        let estimated_chunks = total_chars / self.max_chars + 1;
        let mut chunks = Vec::with_capacity(estimated_chunks);

        let mut start_char = 0;

        while start_char < total_chars {
            let end_char = (start_char + self.max_chars).min(total_chars);
            let mut break_char = end_char;
            if end_char < total_chars {
                let search_start = end_char.saturating_sub(self.overlap_chars).max(start_char + 1);
                for c in (search_start..end_char).rev() {
                    let byte_off = offsets[c];
                    let ch = text.as_bytes()[byte_off] as char;
                    if ch == '\n' {
                        break_char = c + 1;
                        break;
                    }
                    if ch == ' ' && break_char == end_char {
                        break_char = c + 1;
                    }
                }
            }

            let start_byte = offsets[start_char];
            let break_byte = if break_char == total_chars { text.len() } else { offsets[break_char] };
            let chunk_text = text[start_byte..break_byte].to_string();
            chunks.push(Chunk {
                text: chunk_text,
                start_char,
                end_char: break_char,
            });

            if break_char == total_chars {
                break;
            }

            let next_start = break_char.saturating_sub(self.overlap_chars);
            start_char = next_start.max(start_char + 1);
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
