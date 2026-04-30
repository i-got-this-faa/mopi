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
                // look backwards up to self.overlap_chars for a newline
                let search_limit = start + self.overlap_chars;
                if let Some(pos) = chars[search_limit..end].iter().rposition(|&c| c == '\n') {
                    break_point = search_limit + pos + 1; // include the newline
                } else if let Some(pos) = chars[search_limit..end].iter().rposition(|&c| c == ' ') {
                    break_point = search_limit + pos + 1; // break at space
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

            // advance start, but include overlap
            start = break_point.saturating_sub(self.overlap_chars);
            // make sure we always advance
            if start == break_point {
                start += 1;
            }
        }

        chunks
    }
}
