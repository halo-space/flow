pub fn add_space_between_ascii_and_non_ascii(text: &str) -> String {
    let mut result = String::with_capacity(text.len() + 8);
    let mut previous_ascii_alnum = false;

    for ch in text.chars() {
        let current_ascii_alnum = ch.is_ascii_alphanumeric();
        if !result.is_empty()
            && previous_ascii_alnum != current_ascii_alnum
            && (previous_ascii_alnum || current_ascii_alnum)
            && !ch.is_whitespace()
        {
            result.push(' ');
        }
        result.push(ch);
        previous_ascii_alnum = current_ascii_alnum;
    }

    result
}

pub fn is_weak_word(token: &str) -> bool {
    matches!(
        token,
        "请问"
            | "什么"
            | "如何"
            | "哪里"
            | "哪个"
            | "是否"
            | "有没有"
            | "吗"
            | "呢"
            | "the"
            | "a"
            | "an"
            | "is"
            | "are"
            | "do"
            | "does"
    )
}

#[cfg(test)]
mod tests {
    use super::add_space_between_ascii_and_non_ascii;

    #[test]
    fn spaces_mixed_text() {
        assert_eq!(
            add_space_between_ascii_and_non_ascii("如何使用GPT4模型"),
            "如何使用 GPT4 模型"
        );
    }
}
