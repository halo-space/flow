use crate::Result;
use crate::index::chunker::{Delimiter, Fixed, Semantic, SlidingWindow};
use crate::index::{ChunkInput, ChunkPipeline, ChunkPipelineInput, Chunker, ChunkerKind, Piece};

#[derive(Debug, Default)]
pub struct Pipeline {
    fixed: Fixed,
    sliding_window: SlidingWindow,
    delimiter: Delimiter,
    semantic: Semantic,
}

impl ChunkPipeline for Pipeline {
    fn chunk(&self, input: ChunkPipelineInput<'_>) -> Result<Vec<Piece>> {
        let mut pieces = Vec::new();

        for piece in input.parsed.pieces {
            if !piece.questions.is_empty() {
                pieces.push(piece);
                continue;
            }

            let chunk_input = ChunkInput {
                content: &piece.content,
                chunk_size: input.chunk_size,
                chunk_overlap: input.chunk_overlap,
                delimiter: input.delimiter,
            };

            let mut split = match input.kind {
                ChunkerKind::Fixed if input.chunk_overlap == 0 => self.fixed.chunk(chunk_input)?,
                ChunkerKind::Fixed => self.sliding_window.chunk(chunk_input)?,
                ChunkerKind::Delimiter => self.delimiter.chunk(chunk_input)?,
                ChunkerKind::Semantic => self.semantic.chunk(chunk_input)?,
            };
            pieces.append(&mut split);
        }

        Ok(pieces
            .into_iter()
            .enumerate()
            .map(|(chunk_index, mut piece)| {
                piece.positions.chunk_index = chunk_index;
                piece
            })
            .collect())
    }
}

impl Pipeline {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_chunkers(
        fixed: Fixed,
        sliding_window: SlidingWindow,
        delimiter: Delimiter,
        semantic: Semantic,
    ) -> Self {
        Self {
            fixed,
            sliding_window,
            delimiter,
            semantic,
        }
    }
}
