use crate::index::{ParsedContent, Piece, Positions};
use crate::{Error, Result};

pub fn parse(content: &str) -> Result<ParsedContent> {
    let mut pieces = Vec::new();
    let blocks = content.split("\n\n");

    for (chunk_index, block) in blocks.enumerate() {
        let mut question = None;
        let mut answer = Vec::new();

        for line in block.lines().map(str::trim).filter(|line| !line.is_empty()) {
            if let Some(rest) = line.strip_prefix("Q:").or_else(|| line.strip_prefix("问:")) {
                question = Some(rest.trim().to_owned());
            } else if let Some(rest) = line.strip_prefix("A:").or_else(|| line.strip_prefix("答:"))
            {
                answer.push(rest.trim().to_owned());
            } else {
                answer.push(line.to_owned());
            }
        }

        if let Some(question) = question
            && !answer.is_empty()
        {
            pieces.push(Piece {
                content: answer.join("\n"),
                questions: vec![question],
                positions: Positions {
                    chunk_index,
                    start_offset: None,
                    end_offset: None,
                },
            });
        }
    }

    if pieces.is_empty() {
        return Err(Error::InvalidInput(
            "qa content has no valid Q/A pairs".to_owned(),
        ));
    }

    Ok(ParsedContent { pieces })
}
