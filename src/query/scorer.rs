use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::query::Hit;

pub trait QueryScorer: Send + Sync + std::fmt::Debug {
    fn term_score(&self, keywords: &[String], hit: &Hit) -> f32;

    fn vector_score(&self, query_vector: &[f32], hit: &Hit) -> Option<f32>;

    fn query_vector(&self, search_body: &Value) -> Option<Vec<f32>>;
}

#[derive(Debug, Clone)]
pub struct LocalScorer {
    token_weight: f32,
    phrase_weight: f32,
    title_weight: usize,
    keyword_weight: usize,
    question_weight: usize,
    vector_fields: Vec<String>,
}

impl Default for LocalScorer {
    fn default() -> Self {
        Self {
            // 单个 query token 的基础贡献。RAGFlow 本地 token_similarity 里 token 占 0.4。
            token_weight: 0.4,
            // 相邻 query token 拼成 phrase 后的额外贡献。RAGFlow 里相邻 phrase 占 0.6。
            phrase_weight: 0.6,
            // title_tokens 在 chunk 侧重复参与匹配的次数，提升标题命中的影响。
            title_weight: 2,
            // keywords / keyword_tokens 在 chunk 侧重复参与匹配的次数，强调入库阶段抽取的关键词。
            keyword_weight: 5,
            // questions / question_tokens 在 chunk 侧重复参与匹配的次数，候选问题最贴近用户问法，所以权重最高。
            question_weight: 6,
            // 默认读取 hit.source.embedding 作为 chunk 向量；也会自动优先尝试 q_{dim}_vec。
            vector_fields: vec!["embedding".to_owned()],
        }
    }
}

impl LocalScorer {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with_token_weights(mut self, token_weight: f32, phrase_weight: f32) -> Self {
        self.token_weight = token_weight;
        self.phrase_weight = phrase_weight;
        self
    }

    #[must_use]
    pub fn with_field_weights(
        mut self,
        title_weight: usize,
        keyword_weight: usize,
        question_weight: usize,
    ) -> Self {
        self.title_weight = title_weight;
        self.keyword_weight = keyword_weight;
        self.question_weight = question_weight;
        self
    }

    #[must_use]
    pub fn with_vector_fields<I, S>(mut self, vector_fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.vector_fields = vector_fields.into_iter().map(Into::into).collect();
        self
    }

    fn weighted_chunk_tokens(&self, source: &Value) -> Vec<String> {
        let mut tokens = Vec::new();

        tokens.extend(unique_preserve_order(expanded_field_terms(
            source,
            "content_tokens",
        )));
        if tokens.is_empty() {
            tokens.extend(unique_preserve_order(expanded_field_terms(
                source, "content",
            )));
        }

        repeat_extend(
            &mut tokens,
            expanded_field_terms(source, "title_tokens"),
            self.title_weight,
        );
        if !source_has_terms(source, "title_tokens") {
            repeat_extend(
                &mut tokens,
                expanded_field_terms(source, "title"),
                self.title_weight,
            );
        }

        repeat_extend(
            &mut tokens,
            expanded_field_terms(source, "keywords"),
            self.keyword_weight,
        );
        repeat_extend(
            &mut tokens,
            expanded_field_terms(source, "keyword_tokens"),
            self.keyword_weight,
        );
        repeat_extend(
            &mut tokens,
            expanded_field_terms(source, "questions"),
            self.question_weight,
        );
        repeat_extend(
            &mut tokens,
            expanded_field_terms(source, "question_tokens"),
            self.question_weight,
        );

        tokens
    }

    fn token_weight_map(&self, tokens: &[String]) -> BTreeMap<String, f32> {
        let weighted_terms = normalized_term_weights(tokens);
        let mut weights = BTreeMap::new();

        for (index, (term, weight)) in weighted_terms.iter().enumerate() {
            *weights.entry(term.clone()).or_default() += weight * self.token_weight;
            if let Some((next_term, next_weight)) = weighted_terms.get(index + 1) {
                let phrase = format!("{term}{next_term}");
                *weights.entry(phrase).or_default() +=
                    weight.max(*next_weight) * self.phrase_weight;
            }
        }

        weights
    }
}

impl QueryScorer for LocalScorer {
    fn term_score(&self, keywords: &[String], hit: &Hit) -> f32 {
        if keywords.is_empty() {
            return hit.scores.get("hybrid_score").copied().unwrap_or_default();
        }

        let query_weights = self.token_weight_map(keywords);
        if query_weights.is_empty() {
            return hit.scores.get("hybrid_score").copied().unwrap_or_default();
        }

        let chunk_tokens = self.weighted_chunk_tokens(&hit.source);
        if chunk_tokens.is_empty() {
            return 0.0;
        }

        let chunk_weights = self.token_weight_map(&chunk_tokens);
        weighted_token_similarity(&query_weights, &chunk_weights)
    }

    fn vector_score(&self, query_vector: &[f32], hit: &Hit) -> Option<f32> {
        let dynamic_field_name = format!("q_{}_vec", query_vector.len());
        let chunk_vector = number_array(hit.source.get(&dynamic_field_name)).or_else(|| {
            self.vector_fields
                .iter()
                .find_map(|field_name| number_array(hit.source.get(field_name)))
        })?;

        cosine_similarity(query_vector, &chunk_vector)
    }

    fn query_vector(&self, search_body: &Value) -> Option<Vec<f32>> {
        search_body
            .pointer("/knn/query_vector")
            .and_then(|value| number_array(Some(value)))
            .or_else(|| {
                search_body
                    .get("knn")
                    .and_then(Value::as_array)
                    .and_then(|items| items.first())
                    .and_then(|item| item.get("query_vector"))
                    .and_then(|value| number_array(Some(value)))
            })
    }
}

fn normalized_term_weights(tokens: &[String]) -> Vec<(String, f32)> {
    let weighted = tokens
        .iter()
        .map(|token| normalize_token(token))
        .filter(|token| !token.is_empty())
        .map(|token| {
            let weight = term_weight(&token);
            (token, weight)
        })
        .collect::<Vec<_>>();
    let total = weighted.iter().map(|(_, weight)| weight).sum::<f32>();

    if total <= f32::EPSILON {
        return Vec::new();
    }

    weighted
        .into_iter()
        .map(|(token, weight)| (token, weight / total))
        .collect()
}

fn weighted_token_similarity(
    query_weights: &BTreeMap<String, f32>,
    chunk_weights: &BTreeMap<String, f32>,
) -> f32 {
    let mut matched = 1e-9_f32;
    let mut total = 1e-9_f32;

    for (token, weight) in query_weights {
        if chunk_weights.contains_key(token) {
            matched += weight;
        }
        total += weight;
    }

    matched / total
}

fn expanded_field_terms(source: &Value, field: &str) -> Vec<String> {
    field_terms(source, field)
        .into_iter()
        .flat_map(|token| expanded_matching_terms(&token))
        .collect()
}

fn field_terms(source: &Value, field: &str) -> Vec<String> {
    match source.get(field) {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .flat_map(split_term_text)
            .collect(),
        Some(Value::String(text)) => split_term_text(text),
        _ => Vec::new(),
    }
}

fn split_term_text(text: &str) -> Vec<String> {
    text.chars()
        .map(|ch| {
            if ch.is_ascii_punctuation() || ch.is_whitespace() || is_cjk_punctuation(ch) {
                ' '
            } else {
                ch
            }
        })
        .collect::<String>()
        .split_whitespace()
        .map(normalize_token)
        .filter(|token| !token.is_empty())
        .collect()
}

fn expanded_matching_terms(token: &str) -> Vec<String> {
    let token = normalize_token(token);
    if token.is_empty() {
        return Vec::new();
    }

    let mut terms = vec![token.clone()];
    if token.chars().any(is_cjk) && token.chars().count() >= 3 {
        let chars = token.chars().collect::<Vec<_>>();
        terms.extend(
            chars
                .windows(2)
                .map(|window| window.iter().collect::<String>()),
        );
    }

    unique_preserve_order(terms)
}

fn repeat_extend(tokens: &mut Vec<String>, terms: Vec<String>, times: usize) {
    for _ in 0..times {
        tokens.extend(terms.iter().cloned());
    }
}

fn unique_preserve_order(tokens: Vec<String>) -> Vec<String> {
    let mut seen = BTreeSet::new();
    tokens
        .into_iter()
        .filter(|token| seen.insert(token.clone()))
        .collect()
}

fn source_has_terms(source: &Value, field: &str) -> bool {
    !field_terms(source, field).is_empty()
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> Option<f32> {
    if left.len() != right.len() || left.is_empty() {
        return None;
    }

    let mut dot = 0.0_f32;
    let mut left_norm = 0.0_f32;
    let mut right_norm = 0.0_f32;

    for (left_value, right_value) in left.iter().zip(right) {
        dot += left_value * right_value;
        left_norm += left_value * left_value;
        right_norm += right_value * right_value;
    }

    if left_norm <= f32::EPSILON || right_norm <= f32::EPSILON {
        return None;
    }

    Some(dot / left_norm.sqrt() / right_norm.sqrt())
}

fn number_array(value: Option<&Value>) -> Option<Vec<f32>> {
    let values = value?.as_array()?;
    let vector = values
        .iter()
        .map(|value| value.as_f64().map(|number| number as f32))
        .collect::<Option<Vec<_>>>()?;

    if vector.is_empty() {
        None
    } else {
        Some(vector)
    }
}

fn term_weight(token: &str) -> f32 {
    if token.chars().all(|ch| ch.is_ascii_digit()) {
        return 2.0;
    }
    if token.chars().all(|ch| ch.is_ascii_alphabetic()) && token.chars().count() <= 2 {
        return 0.01;
    }
    if token.chars().count() == 1 {
        return 0.2;
    }
    if token.chars().count() >= 4 {
        return 1.4;
    }
    1.0
}

fn normalize_token(token: &str) -> String {
    token.trim().to_lowercase()
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch,
        '\u{4e00}'..='\u{9fff}'
            | '\u{3400}'..='\u{4dbf}'
            | '\u{20000}'..='\u{2a6df}'
            | '\u{2a700}'..='\u{2b73f}'
            | '\u{2b740}'..='\u{2b81f}'
            | '\u{2b820}'..='\u{2ceaf}'
    )
}

fn is_cjk_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '，' | '。'
            | '？'
            | '！'
            | '；'
            | '：'
            | '、'
            | '“'
            | '”'
            | '‘'
            | '’'
            | '（'
            | '）'
            | '【'
            | '】'
            | '《'
            | '》'
            | '「'
            | '」'
            | '『'
            | '』'
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use crate::query::{Hit, LocalScorer, QueryScorer};

    #[test]
    fn local_scorer_matches_token_overlap() {
        let hit = Hit {
            id: "chunk_1".to_owned(),
            source: json!({
                "title": "密码重置指南",
                "content": "用户可以通过忘记密码入口重置密码。",
                "title_tokens": ["密码", "重置", "指南"],
                "content_tokens": ["忘记密码", "重置密码"],
                "keywords": ["密码重置"],
                "questions": []
            }),
            score: 4.0,
            scores: BTreeMap::new(),
            highlight: None,
        };

        let keywords = vec!["密码".to_owned(), "重置".to_owned(), "邮箱".to_owned()];
        let score = LocalScorer::default().term_score(&keywords, &hit);

        assert!(score > 0.0);
        assert!(score < 1.0);
    }

    #[test]
    fn local_scorer_uses_keyword_and_question_terms() {
        let keyword_hit = Hit {
            id: "chunk_1".to_owned(),
            source: json!({
                "content_tokens": ["普通内容"],
                "keywords": ["重置密码"],
                "questions": ["如何重置密码"]
            }),
            score: 1.0,
            scores: BTreeMap::new(),
            highlight: None,
        };
        let content_hit = Hit {
            id: "chunk_2".to_owned(),
            source: json!({
                "content_tokens": ["普通内容"],
                "keywords": [],
                "questions": []
            }),
            score: 1.0,
            scores: BTreeMap::new(),
            highlight: None,
        };
        let keywords = vec!["重置".to_owned(), "密码".to_owned()];
        let scorer = LocalScorer::default();

        assert!(
            scorer.term_score(&keywords, &keyword_hit) > scorer.term_score(&keywords, &content_hit)
        );
    }

    #[test]
    fn local_scorer_allows_field_weight_override() {
        let scorer = LocalScorer::default().with_field_weights(1, 1, 1);
        let hit = Hit {
            id: "chunk_1".to_owned(),
            source: json!({
                "content_tokens": ["普通内容"],
                "questions": ["重置密码"]
            }),
            score: 1.0,
            scores: BTreeMap::new(),
            highlight: None,
        };

        assert!(scorer.term_score(&["重置".to_owned(), "密码".to_owned()], &hit) > 0.0);
    }

    #[test]
    fn local_scorer_uses_query_vector_and_chunk_embedding() {
        let hit = Hit {
            id: "chunk_1".to_owned(),
            source: json!({
                "embedding": [1.0, 0.0, 0.0]
            }),
            score: 1.0,
            scores: BTreeMap::new(),
            highlight: None,
        };

        assert_eq!(
            LocalScorer::default().vector_score(&[1.0, 0.0, 0.0], &hit),
            Some(1.0)
        );
    }

    #[test]
    fn local_scorer_reads_elastic_knn_query_vector() {
        let body = json!({
            "query": { "match_all": {} },
            "knn": {
                "field": "embedding",
                "query_vector": [0.1, 0.2],
                "k": 10
            }
        });

        assert_eq!(
            LocalScorer::default().query_vector(&body),
            Some(vec![0.1_f32, 0.2_f32])
        );
    }
}
