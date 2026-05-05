use crate::index::{Piece, Positions};

pub fn chunk(content: &str, chunk_size: usize, chunk_overlap: usize) -> Vec<Piece> {
    if content.is_empty() {
        return Vec::new();
    }

    let chars: Vec<char> = content.chars().collect();
    let size = chunk_size.max(1);
    let overlap = chunk_overlap.min(size.saturating_sub(1));
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
        if end == chars.len() {
            break;
        }
        start = end.saturating_sub(overlap);
    }

    pieces
}

#[cfg(test)]
mod tests {
    use super::chunk;

    #[test]
    fn sliding_window_keeps_overlap() {
        let pieces = chunk("abcdef", 3, 1);

        assert_eq!(pieces.len(), 3);
        assert_eq!(pieces[0].content, "abc");
        assert_eq!(pieces[1].content, "cde");
        assert_eq!(pieces[2].content, "ef");
    }
}
