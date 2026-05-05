use crate::Result;
use crate::index::{ChunkInput, Chunker, Piece, Positions};

#[derive(Debug, Default, Clone, Copy)]
pub struct Fixed;

impl Chunker for Fixed {
    fn chunk(&self, input: ChunkInput<'_>) -> Result<Vec<Piece>> {
        if input.content.is_empty() {
            return Ok(Vec::new());
        }

        let chars: Vec<char> = input.content.chars().collect();
        let size = input.chunk_size.max(1);
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
            start = end;
        }

        Ok(pieces)
    }
}

#[cfg(test)]
mod tests {
    use crate::index::{ChunkInput, Chunker};

    use super::Fixed;

    #[test]
    fn fixed_size_has_no_overlap() {
        let pieces = Fixed
            .chunk(ChunkInput {
                content: "abcdef",
                chunk_size: 2,
                chunk_overlap: 0,
                delimiter: None,
            })
            .unwrap();

        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0].content, "ab");
        assert_eq!(pieces[1].content, "cd");
        assert_eq!(pieces[2].content, "ef");
    }
}
