use crate::Result;
use crate::index::{ChunkInput, Chunker, Piece, Positions};

#[derive(Debug, Default, Clone, Copy)]
pub struct Delimiter;

impl Chunker for Delimiter {
    fn chunk(&self, input: ChunkInput<'_>) -> Result<Vec<Piece>> {
        let delimiter = match input.delimiter {
            Some(delimiter) if !delimiter.is_empty() => delimiter,
            _ => "\n\n",
        };
        let mut pieces = Vec::new();
        let mut current = String::new();

        for part in input
            .content
            .split(delimiter)
            .map(str::trim)
            .filter(|part| !part.is_empty())
        {
            if !current.is_empty() && current.len() + part.len() > input.chunk_size {
                pieces.push(piece(std::mem::take(&mut current), pieces.len()));
            }
            if !current.is_empty() {
                current.push_str(delimiter);
            }
            current.push_str(part);
        }

        if !current.is_empty() {
            pieces.push(piece(current, pieces.len()));
        }

        Ok(pieces)
    }
}

fn piece(content: String, chunk_index: usize) -> Piece {
    Piece {
        content,
        questions: Vec::new(),
        positions: Positions {
            chunk_index,
            start_offset: None,
            end_offset: None,
        },
    }
}
