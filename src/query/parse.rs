use std::collections::BTreeSet;

use crate::query::{ParseQuery, QueryLanguage, QueryParser};
use crate::utils::normalization::{add_space_between_ascii_and_non_ascii, is_weak_word};
use crate::{Error, Result};
use zhconv::{Variant, zhconv};

const MAX_KEYWORDS: usize = 32;
const MAX_EXPRESSION_TERMS: usize = 256;

#[derive(Debug)]
pub struct DefaultQueryParser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueryPath {
    Chinese,
    English,
}

#[derive(Debug, Clone, PartialEq)]
struct WeightedTerm {
    text: String,
    weight: f32,
}

impl QueryParser for DefaultQueryParser {
    fn parse(&self, query: &str) -> Result<ParseQuery> {
        if query.trim().is_empty() {
            return Err(Error::InvalidInput("query cannot be empty".to_owned()));
        }

        let spaced_query = self.add_language_boundary_spacing(query);
        let lowered_query = self.lowercase_english(&spaced_query);
        let halfwidth_query = self.fullwidth_to_halfwidth(&lowered_query);
        let simplified_query = self.traditional_to_simplified(&halfwidth_query);
        let safe_query = self.replace_search_syntax_with_space(&simplified_query);
        let normalized_query = self.remove_weak_semantic_words(&safe_query);
        let language = self.detect_language(&normalized_query);
        let path = self.query_path(language);
        let tokens = self.tokenize(&normalized_query, path);
        let weighted_terms = self.calculate_term_weights(&tokens, path);
        let synonym_terms = self.expand_synonyms(&weighted_terms);
        let fine_grained_terms = self.controlled_fine_grained_expansion(&weighted_terms, path);
        let keywords = self.build_keywords(&tokens, &synonym_terms, &fine_grained_terms);
        let text_expression =
            self.build_text_expression(&weighted_terms, &synonym_terms, &fine_grained_terms);

        Ok(ParseQuery {
            original_query: query.to_owned(),
            normalized_query,
            keywords,
            text_expression,
            language,
        })
    }
}

impl DefaultQueryParser {
    fn add_language_boundary_spacing(&self, query: &str) -> String {
        add_space_between_ascii_and_non_ascii(query)
    }

    fn lowercase_english(&self, query: &str) -> String {
        query.to_lowercase()
    }

    fn fullwidth_to_halfwidth(&self, query: &str) -> String {
        query
            .chars()
            .map(|ch| match ch {
                '\u{3000}' => ' ',
                '\u{ff01}'..='\u{ff5e}' => char::from_u32(ch as u32 - 0xfee0).unwrap_or(ch),
                _ => ch,
            })
            .collect()
    }

    fn traditional_to_simplified(&self, query: &str) -> String {
        zhconv(query, Variant::ZhHans)
    }

    fn replace_search_syntax_with_space(&self, query: &str) -> String {
        query
            .chars()
            .map(|ch| {
                if is_search_syntax_char(ch) || ch.is_whitespace() {
                    ' '
                } else {
                    ch
                }
            })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn remove_weak_semantic_words(&self, query: &str) -> String {
        let mut cleaned = query.to_owned();
        for weak_word in WEAK_PHRASES {
            cleaned = cleaned.replace(weak_word, " ");
        }

        let tokens = cleaned
            .split_whitespace()
            .flat_map(strip_weak_chinese_edges)
            .filter(|token| !is_weak_word(token))
            .filter(|token| !WEAK_PHRASES.contains(token))
            .collect::<Vec<_>>();

        if tokens.is_empty() {
            query.to_owned()
        } else {
            tokens.join(" ")
        }
    }

    fn detect_language(&self, query: &str) -> QueryLanguage {
        let has_ascii = query.chars().any(|ch| ch.is_ascii_alphabetic());
        let has_non_ascii = query
            .chars()
            .any(|ch| !ch.is_ascii() && !ch.is_whitespace());

        match (has_ascii, has_non_ascii) {
            (true, true) => QueryLanguage::Mixed,
            (true, false) => QueryLanguage::English,
            (false, true) => QueryLanguage::Chinese,
            (false, false) => QueryLanguage::Unknown,
        }
    }

    fn query_path(&self, language: QueryLanguage) -> QueryPath {
        match language {
            QueryLanguage::English => QueryPath::English,
            QueryLanguage::Chinese | QueryLanguage::Mixed | QueryLanguage::Unknown => {
                QueryPath::Chinese
            }
        }
    }

    fn tokenize(&self, query: &str, path: QueryPath) -> Vec<String> {
        match path {
            QueryPath::English => query
                .split_whitespace()
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .map(ToOwned::to_owned)
                .collect(),
            QueryPath::Chinese => query
                .split_whitespace()
                .flat_map(split_mixed_token)
                .filter(|token| !token.is_empty())
                .collect(),
        }
    }

    fn calculate_term_weights(&self, tokens: &[String], path: QueryPath) -> Vec<WeightedTerm> {
        tokens
            .iter()
            .take(MAX_EXPRESSION_TERMS)
            .map(|token| WeightedTerm {
                text: token.clone(),
                weight: term_weight(token, path),
            })
            .collect()
    }

    fn expand_synonyms(&self, weighted_terms: &[WeightedTerm]) -> Vec<WeightedTerm> {
        weighted_terms
            .iter()
            .flat_map(|term| {
                synonyms(&term.text)
                    .into_iter()
                    .map(|synonym| WeightedTerm {
                        text: synonym,
                        weight: term.weight * 0.2,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn controlled_fine_grained_expansion(
        &self,
        weighted_terms: &[WeightedTerm],
        path: QueryPath,
    ) -> Vec<WeightedTerm> {
        if path == QueryPath::English {
            return Vec::new();
        }

        weighted_terms
            .iter()
            .flat_map(|term| {
                controlled_segments(&term.text)
                    .into_iter()
                    .map(|segment| WeightedTerm {
                        text: segment,
                        weight: term.weight * 0.5,
                    })
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    fn build_keywords(
        &self,
        tokens: &[String],
        synonym_terms: &[WeightedTerm],
        fine_grained_terms: &[WeightedTerm],
    ) -> Vec<String> {
        let mut seen = BTreeSet::new();
        let mut keywords = Vec::new();

        for keyword in tokens
            .iter()
            .cloned()
            .chain(synonym_terms.iter().map(|term| term.text.clone()))
            .chain(fine_grained_terms.iter().map(|term| term.text.clone()))
        {
            if keyword.is_empty() || !seen.insert(keyword.clone()) {
                continue;
            }
            keywords.push(keyword);
            if keywords.len() >= MAX_KEYWORDS {
                break;
            }
        }

        keywords
    }

    fn build_text_expression(
        &self,
        weighted_terms: &[WeightedTerm],
        synonym_terms: &[WeightedTerm],
        fine_grained_terms: &[WeightedTerm],
    ) -> String {
        let base_terms = weighted_terms
            .iter()
            .map(|term| weighted_expression(&term.text, term.weight));
        let synonyms = synonym_terms
            .iter()
            .map(|term| weighted_expression(&term.text, term.weight));
        let fine_grained = fine_grained_terms
            .iter()
            .map(|term| weighted_expression(&term.text, term.weight));

        base_terms
            .chain(synonyms)
            .chain(fine_grained)
            .take(MAX_EXPRESSION_TERMS)
            .collect::<Vec<_>>()
            .join(" OR ")
    }
}

const WEAK_PHRASES: &[&str] = &[
    "怎么办",
    "什么样的",
    "哪家",
    "一下",
    "那家",
    "请问",
    "啥样",
    "咋样了",
    "什么时候",
    "何时",
    "何地",
    "何人",
    "是否",
    "是不是",
    "多少",
    "哪里",
    "怎么",
    "哪儿",
    "怎么样",
    "如何",
    "哪些",
    "是啥",
    "啥是",
    "有没有",
    "哪位",
    "哪个",
    "什么",
    "在",
    "于",
    "和",
    "与",
    "的",
    "了",
    "是",
    "啊",
    "吧",
    "呀",
    "谁",
    "who",
    "what",
    "how",
    "which",
    "where",
    "why",
    "is",
    "are",
    "were",
    "was",
    "do",
    "does",
    "did",
    "has",
    "have",
    "be",
    "there",
    "you",
    "me",
    "your",
    "my",
    "mine",
    "please",
    "just",
    "may",
    "should",
    "would",
    "will",
    "go",
    "for",
    "with",
    "so",
    "the",
    "a",
    "an",
    "by",
    "as",
    "on",
    "in",
    "at",
    "up",
    "out",
    "down",
    "of",
    "to",
    "or",
    "and",
    "if",
];

fn strip_weak_chinese_edges(token: &str) -> Vec<&str> {
    let stripped_prefix = WEAK_CHINESE_PREFIXES
        .iter()
        .find_map(|prefix| token.strip_prefix(prefix))
        .filter(|remaining| !remaining.is_empty())
        .unwrap_or(token);

    let stripped = WEAK_CHINESE_SUFFIXES
        .iter()
        .find_map(|suffix| stripped_prefix.strip_suffix(suffix))
        .filter(|remaining| !remaining.is_empty())
        .unwrap_or(stripped_prefix);

    if stripped.is_empty() {
        Vec::new()
    } else {
        vec![stripped]
    }
}

const WEAK_CHINESE_PREFIXES: &[&str] = &["在", "于", "是"];
const WEAK_CHINESE_SUFFIXES: &[&str] = &["吗", "呢", "吧", "啊", "呀", "的", "了"];

fn is_search_syntax_char(ch: char) -> bool {
    matches!(
        ch,
        ' ' | ':'
            | '|'
            | '\r'
            | '\n'
            | '\t'
            | ','
            | '，'
            | '。'
            | '？'
            | '?'
            | '/'
            | '`'
            | '!'
            | '！'
            | '&'
            | '^'
            | '%'
            | '('
            | ')'
            | '['
            | ']'
            | '{'
            | '}'
            | '<'
            | '>'
            | '*'
            | '~'
            | '\''
            | '"'
            | '\\'
    )
}

fn split_mixed_token(token: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut current_ascii = None;

    for ch in token.chars() {
        let ascii = ch.is_ascii_alphanumeric();
        match current_ascii {
            Some(previous_ascii) if previous_ascii != ascii => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
                current_ascii = Some(ascii);
            }
            None => current_ascii = Some(ascii),
            _ => {}
        }
        current.push(ch);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

fn term_weight(token: &str, path: QueryPath) -> f32 {
    if token.chars().all(|ch| ch.is_ascii_digit()) {
        return 2.0;
    }
    if token.chars().all(|ch| ch.is_ascii_alphabetic()) && token.len() <= 2 {
        return 0.3;
    }

    let char_count = token.chars().count();
    let base = match path {
        QueryPath::English => 1.0,
        QueryPath::Chinese => 1.2,
    };
    let length_boost = (char_count.min(8) as f32) * 0.08;
    base + length_boost
}

fn synonyms(token: &str) -> Vec<String> {
    match token {
        "密码" => vec!["口令".to_owned()],
        "重置" => vec!["找回".to_owned(), "恢复".to_owned()],
        "举办" => vec!["举行".to_owned()],
        "查询" => vec!["检索".to_owned(), "搜索".to_owned()],
        "search" => vec!["query".to_owned(), "lookup".to_owned()],
        "reset" => vec!["recover".to_owned(), "restore".to_owned()],
        _ => Vec::new(),
    }
}

fn controlled_segments(token: &str) -> Vec<String> {
    if token.chars().any(|ch| ch.is_ascii_alphanumeric()) || token.chars().count() < 3 {
        return Vec::new();
    }

    let chars = token.chars().collect::<Vec<_>>();
    chars
        .windows(2)
        .map(|window| window.iter().collect::<String>())
        .filter(|segment| segment != token)
        .collect()
}

fn weighted_expression(token: &str, weight: f32) -> String {
    let escaped = escape_expression_token(token);
    if escaped.contains(' ') {
        format!("\"{}\"^{:.3}", escaped, weight)
    } else {
        format!("{}^{:.3}", escaped, weight)
    }
}

fn escape_expression_token(token: &str) -> String {
    token
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\'', "")
}

#[cfg(test)]
mod tests {
    use crate::query::{DefaultQueryParser, QueryLanguage, QueryParser};

    #[test]
    fn default_query_parser_follows_full_flow() {
        let parsed = DefaultQueryParser
            .parse("ＦＥＮ (超) 在哪裡舉辦？")
            .expect("query should parse");

        assert_eq!(parsed.original_query, "ＦＥＮ (超) 在哪裡舉辦？");
        assert_eq!(parsed.normalized_query, "fen 超 举办");
        assert_eq!(parsed.language, QueryLanguage::Mixed);
        assert!(parsed.keywords.contains(&"fen".to_owned()));
        assert!(parsed.keywords.contains(&"超".to_owned()));
        assert!(parsed.keywords.contains(&"举办".to_owned()));
        assert!(parsed.keywords.contains(&"举行".to_owned()));
        assert!(!parsed.keywords.contains(&"哪里".to_owned()));
        assert!(parsed.text_expression.contains("fen^"));
        assert!(parsed.text_expression.contains("举办^"));
    }

    #[test]
    fn default_query_parser_removes_weak_english_words() {
        let parsed = DefaultQueryParser
            .parse("Please search reset password")
            .expect("query should parse");

        assert_eq!(parsed.language, QueryLanguage::English);
        assert!(!parsed.keywords.contains(&"please".to_owned()));
        assert!(parsed.keywords.contains(&"reset".to_owned()));
        assert!(parsed.keywords.contains(&"recover".to_owned()));
    }
}
