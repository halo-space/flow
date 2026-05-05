use crate::index::chunker;
use crate::index::{ChunkerKind, ParsedContent, Piece};

pub fn chunk(
    parsed: ParsedContent,
    kind: ChunkerKind,
    chunk_size: usize,
    chunk_overlap: usize,
    delimiter: Option<&str>,
) -> Vec<Piece> {
    let mut pieces = Vec::new();

    for piece in parsed.pieces {
        if !piece.questions.is_empty() {
            pieces.push(piece);
            continue;
        }

        let mut split = match kind {
            ChunkerKind::Fixed if chunk_overlap == 0 => {
                chunker::fixed_size::chunk(&piece.content, chunk_size)
            }
            ChunkerKind::Fixed => {
                chunker::sliding_window::chunk(&piece.content, chunk_size, chunk_overlap)
            }
            ChunkerKind::Delimiter => {
                chunker::delimiter::chunk(&piece.content, delimiter.unwrap_or("\n\n"), chunk_size)
            }
            ChunkerKind::Semantic => chunker::semantic::chunk(&piece.content, chunk_size),
        };
        pieces.append(&mut split);
    }

    pieces
        .into_iter()
        .enumerate()
        .map(|(chunk_index, mut piece)| {
            piece.positions.chunk_index = chunk_index;
            piece
        })
        .collect()
}
