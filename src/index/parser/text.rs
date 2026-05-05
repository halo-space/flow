use crate::Result;
use crate::index::{ParseInput, ParsedContent, Parser, Piece, Positions};

#[derive(Debug, Default, Clone, Copy)]
pub struct PlainText;

impl Parser for PlainText {
    fn parse(&self, input: ParseInput<'_>) -> Result<ParsedContent> {
        Ok(ParsedContent {
            pieces: vec![Piece {
                content: input.extracted.content.clone(),
                questions: Vec::new(),
                positions: Positions::default(),
            }],
        })
    }
}
