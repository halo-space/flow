use serde::{Deserialize, Serialize};
use serde_json::Value;

use chrono::{DateTime, Utc};

use crate::store::Item;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildInput {
    pub content: String,
    pub title: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub format: ContentFormat,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub knowledge_base_id: Option<String>,
    pub metadata: Value,
    pub tags: Vec<String>,
    pub chunker: Option<ChunkerKind>,
    pub chunk_size: Option<usize>,
    pub chunk_overlap: Option<usize>,
    pub delimiter: Option<String>,
    pub keywords: Vec<String>,
    pub questions: Vec<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContentFormat {
    #[default]
    Text,
    Qa,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum ChunkerKind {
    #[default]
    Fixed,
    Delimiter,
    Semantic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BuildOutput {
    pub document: Item,
    pub chunks: Vec<Item>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DefaultDocument {
    pub id: String,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub knowledge_base_id: Option<String>,
    pub title: String,
    pub content: String,
    pub hash_id: Option<String>,
    pub size: usize,
    pub created_at: DateTime<Utc>,
    pub metadata: Value,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DefaultChunk {
    pub id: String,
    pub tenant_id: Option<String>,
    pub user_id: Option<String>,
    pub knowledge_base_id: Option<String>,
    pub doc_id: String,
    pub title: String,
    pub title_tokens: Vec<String>,
    pub content: String,
    pub content_tokens: Vec<String>,
    pub keywords: Vec<String>,
    pub keyword_tokens: Vec<String>,
    pub questions: Vec<String>,
    pub question_tokens: Vec<String>,
    pub embedding: Option<Vec<f32>>,
    pub positions: Positions,
    pub tags: Vec<String>,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Positions {
    pub chunk_index: usize,
    pub start_offset: Option<usize>,
    pub end_offset: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractedDocument {
    pub title: String,
    pub content: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ParsedContent {
    pub pieces: Vec<Piece>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Piece {
    pub content: String,
    pub questions: Vec<String>,
    pub positions: Positions,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeywordExtractionInput<'a> {
    pub title: &'a str,
    pub content: &'a str,
    pub top: usize,
}
