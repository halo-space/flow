use crate::index::{Piece, Positions};

pub fn chunk(content: &str, delimiter: &str, chunk_size: usize) -> Vec<Piece> {
    let delimiter = if delimiter.is_empty() {
        "\n\n"
    } else {
        delimiter
    };
    let mut pieces = Vec::new();
    let mut current = String::new();

    for part in content
        .split(delimiter)
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        if !current.is_empty() && current.len() + part.len() > chunk_size {
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

    pieces
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
