use crate::index::{Piece, chunker};

pub fn chunk(content: &str, chunk_size: usize) -> Vec<Piece> {
    chunker::delimiter::chunk(content, "\n", chunk_size)
}
