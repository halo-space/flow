use crate::Result;
use crate::index::{ChunkInput, Chunker, Piece, Positions};

#[derive(Debug, Default, Clone, Copy)]
pub struct SlidingWindow;

impl Chunker for SlidingWindow {
    fn chunk(&self, input: ChunkInput<'_>) -> Result<Vec<Piece>> {
        if input.content.is_empty() {
            return Ok(Vec::new());
        }

        let chars: Vec<char> = input.content.chars().collect();
        let size = input.chunk_size.max(1);
        let overlap = input.chunk_overlap.min(size.saturating_sub(1));
        let mut pieces = Vec::new();
        let mut start = 0;

        while start < chars.len() {
            let end = (start + size).min(chars.len());
            let text: String = chars[start..end].iter().collect();
            pieces.push(Piece {
                content: text,
                questions: Vec::new(),
                positions: Positions {
                    chunk_index: pieces.len(),
                    start_offset: Some(start),
                    end_offset: Some(end),
                },
            });
            if end == chars.len() {
                break;
            }
            start = end.saturating_sub(overlap);
        }

        Ok(pieces)
    }
}

#[cfg(test)]
mod tests {
    use crate::index::{ChunkInput, Chunker};

    use super::SlidingWindow;

    #[test]
    fn sliding_window_keeps_overlap() {
        let pieces = SlidingWindow
            .chunk(ChunkInput {
                content: "abcdef",
                chunk_size: 3,
                chunk_overlap: 1,
                delimiter: None,
            })
            .unwrap();

        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0].content, "abc");
        assert_eq!(pieces[1].content, "cde");
        assert_eq!(pieces[2].content, "ef");
    }
}
