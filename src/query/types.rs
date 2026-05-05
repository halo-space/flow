use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Hit {
    pub id: String,
    pub source: Value,
    pub score: f32,
    pub scores: Scores,
    pub highlight: Option<String>,
}

pub type Scores = BTreeMap<String, f32>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HitPage {
    pub total: usize,
    pub hits: Vec<Hit>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum QueryLanguage {
    Chinese,
    English,
    Mixed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParseQuery {
    pub original_query: String,
    pub normalized_query: String,
    pub keywords: Vec<String>,
    pub text_expression: String,
    pub language: QueryLanguage,
}
