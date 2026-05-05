pub mod qa;
pub mod text;

pub use qa::Qa;
pub use text::PlainText;

use crate::Result;
use crate::index::{ContentFormat, ParseInput, ParsedContent, Parser};

#[derive(Debug, Default)]
pub struct Pipeline {
    plain_text: PlainText,
    qa: Qa,
}

impl Parser for Pipeline {
    fn parse(&self, input: ParseInput<'_>) -> Result<ParsedContent> {
        match input.format {
            ContentFormat::Text => self.plain_text.parse(input),
            ContentFormat::Qa => self.qa.parse(input),
        }
    }
}
