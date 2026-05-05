use crate::index::{ExtractedDocument, ParsedContent, Piece, Positions};

pub fn parse(extracted: &ExtractedDocument) -> ParsedContent {
    ParsedContent {
        pieces: vec![Piece {
            content: extracted.content.clone(),
            questions: Vec::new(),
            positions: Positions::default(),
        }],
    }
}
