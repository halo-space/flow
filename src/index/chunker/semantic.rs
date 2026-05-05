use crate::Result;
use crate::index::chunker::Delimiter;
use crate::index::{ChunkInput, Chunker, Piece};

#[derive(Debug, Default, Clone, Copy)]
pub struct Semantic;

impl Chunker for Semantic {
    fn chunk(&self, input: ChunkInput<'_>) -> Result<Vec<Piece>> {
        Delimiter.chunk(ChunkInput {
            delimiter: Some("\n"),
            ..input
        })
    }
}
