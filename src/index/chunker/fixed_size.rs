use crate::index::{Piece, Positions};

pub fn chunk(content: &str, chunk_size: usize) -> Vec<Piece> {
    if content.is_empty() {
        return Vec::new();
    }

    let chars: Vec<char> = content.chars().collect();
    let size = chunk_size.max(1);
    let mut pieces = Vec::new();
    let mut start = 0;

    while start < chars.len() {
        let end = (start + size).min(chars.len());
        let text: String = chars[start..end].iter().collect();
        pieces.push(Piece {
            content: text,
            questions: Vec::new(),
            positions: Positions {
                chunk_index: pieces.len(),
                start_offset: Some(start),
                end_offset: Some(end),
            },
        });
        start = end;
    }

    pieces
}

#[cfg(test)]
mod tests {
    use super::chunk;

    #[test]
    fn fixed_size_has_no_overlap() {
        let pieces = chunk("abcdef", 2);

        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0].content, "ab");
        assert_eq!(pieces[1].content, "cd");
        assert_eq!(pieces[2].content, "ef");
    }
}
