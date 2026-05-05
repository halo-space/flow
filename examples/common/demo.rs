#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rag::index::{BuildInput, ChunkerKind, ContentFormat};
use rag::query::{DefaultQueryParser, QueryEngine, QueryParser};
use rag::store::SearchRequest;
use rag::{
    BoxFuture, DefaultIndexBuilder, DefaultQueryEngine, Elastic, Embedder, Error, IndexBuilder,
    IndexBuilderConfig, Result, Store,
};
use serde_json::{Value, json};

pub const DEFAULT_DATA_DIR: &str = "examples/data";
pub const ES_URL: &str = "http://127.0.0.1:9200";
pub const DEMO_QUERY: &str = "怎么重置密码？";
pub const DEMO_KNOWLEDGE_BASE_ID: &str = "demo_knowledge_base";
pub const DOCUMENTS_INDEX: &str = "rag_demo_documents";
pub const CHUNKS_INDEX: &str = "rag_demo_chunks";
pub const EMBEDDING_DIM: usize = 1024;

#[derive(Debug, Clone, Default)]
pub struct DemoEmbedder;

impl Embedder for DemoEmbedder {
    fn embed<'a>(&'a self, text: &'a str) -> BoxFuture<'a, Result<Vec<f32>>> {
        Box::pin(async move {
            let mut vector = vec![0.0; EMBEDDING_DIM];
            let mut token_count = 0usize;

            for token in text.split_whitespace().filter(|token| !token.is_empty()) {
                let index = stable_hash(token) % EMBEDDING_DIM;
                vector[index] += 1.0;
                token_count += 1;
            }

            if token_count == 0 {
                for character in text.chars().filter(|character| !character.is_whitespace()) {
                    let index = stable_hash(&character.to_string()) % EMBEDDING_DIM;
                    vector[index] += 1.0;
                }
            }

            let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
            if norm > 0.0 {
                for value in &mut vector {
                    *value /= norm;
                }
            }

            Ok(vector)
        })
    }
}

fn stable_hash(text: &str) -> usize {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in text.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash as usize
}

pub fn create_store() -> Result<Arc<dyn Store>> {
    Ok(Arc::new(Elastic::new(ES_URL)?))
}

pub fn create_embedder() -> Arc<dyn Embedder> {
    Arc::new(DemoEmbedder)
}

pub fn create_index_builder(
    store: Arc<dyn Store>,
    embedder: Arc<dyn Embedder>,
) -> DefaultIndexBuilder {
    DefaultIndexBuilder::new(
        Some(store),
        Some(embedder),
        IndexBuilderConfig {
            documents_index: DOCUMENTS_INDEX.to_owned(),
            chunks_index: CHUNKS_INDEX.to_owned(),
            chunker: ChunkerKind::Fixed,
            chunk_size: 350,
            chunk_overlap: 80,
            delimiter: None,
            keyword_top: 3,
        },
    )
}

pub fn create_engine(store: Arc<dyn Store>) -> DefaultQueryEngine {
    DefaultQueryEngine::with_search_settings(store, None, 20, 0.0, 0.95)
}

pub fn data_files() -> Result<Vec<PathBuf>> {
    txt_files(Path::new(DEFAULT_DATA_DIR))
}

pub fn text_search_body(text_expression: &str) -> Value {
    json!({
        "size": 20,
        "query": {
            "bool": {
                "must": [{
                    "query_string": {
                        "query": text_expression,
                        "fields": [
                            "questions^8",
                            "keywords^6",
                            "title^4",
                            "content^1",
                            "question_tokens^3",
                            "keyword_tokens^3",
                            "title_tokens^2",
                            "content_tokens^1"
                        ],
                        "default_operator": "OR"
                    }
                }]
            }
        },
        "highlight": {
            "fields": {
                "content": {}
            }
        }
    })
}

pub fn vector_search_body(query_vector: Vec<f32>) -> Value {
    json!({
        "size": 20,
        "knn": {
            "field": "embedding",
            "query_vector": query_vector,
            "k": 20,
            "num_candidates": 100
        }
    })
}

pub fn hybrid_search_body(text_expression: &str, query_vector: Vec<f32>) -> Value {
    let bool_query = json!({
        "bool": {
            "must": [{
                "query_string": {
                    "query": text_expression,
                    "fields": [
                        "questions^8",
                        "keywords^6",
                        "title^4",
                        "content^1",
                        "question_tokens^3",
                        "keyword_tokens^3",
                        "title_tokens^2",
                        "content_tokens^1"
                    ],
                    "type": "best_fields",
                    "minimum_should_match": "30%"
                }
            }],
            "boost": 0.05
        }
    });

    json!({
        "size": 20,
        "query": bool_query.clone(),
        "knn": {
            "field": "embedding",
            "query_vector": query_vector,
            "k": 20,
            "num_candidates": 100,
            "boost": 0.95,
            "filter": bool_query
        },
        "highlight": {
            "fields": {
                "content": {}
            }
        }
    })
}

pub fn search_body(text_expression: &str, query_vector: Vec<f32>) -> Value {
    hybrid_search_body(text_expression, query_vector)
}

pub fn search_request(text_expression: &str, query_vector: Vec<f32>) -> SearchRequest {
    SearchRequest {
        index_name: CHUNKS_INDEX.to_owned(),
        body: search_body(text_expression, query_vector),
    }
}

pub fn document_schema() -> Value {
    json!({
        "mappings": {
            "properties": {
                "id": { "type": "keyword" },
                "knowledge_base_id": { "type": "keyword" },
                "title": { "type": "text" },
                "content": { "type": "text" },
                "created_at": { "type": "date" },
                "metadata": { "type": "object", "enabled": true },
                "type": { "type": "keyword" }
            }
        }
    })
}

pub fn chunk_schema() -> Value {
    json!({
        "mappings": {
            "properties": {
                "id": { "type": "keyword" },
                "doc_id": { "type": "keyword" },
                "knowledge_base_id": { "type": "keyword" },
                "title": { "type": "text" },
                "title_tokens": { "type": "text" },
                "content": { "type": "text" },
                "content_tokens": { "type": "text" },
                "keywords": { "type": "keyword" },
                "keyword_tokens": { "type": "text" },
                "questions": { "type": "text" },
                "question_tokens": { "type": "text" },
                "tags": { "type": "keyword" },
                "embedding": {
                    "type": "dense_vector",
                    "dims": EMBEDDING_DIM,
                    "index": true,
                    "similarity": "cosine"
                }
            }
        }
    })
}

pub async fn recreate_indexes(store: &dyn Store) -> Result<()> {
    let delete_all = json!({ "query": { "match_all": {} } });
    let _ = store.delete(DOCUMENTS_INDEX, delete_all.clone()).await;
    let _ = store.delete(CHUNKS_INDEX, delete_all).await;

    let _ = store
        .create_schema(DOCUMENTS_INDEX, document_schema())
        .await;
    let _ = store.create_schema(CHUNKS_INDEX, chunk_schema()).await;
    Ok(())
}

pub async fn ingest_all_texts() -> Result<usize> {
    let store = create_store()?;
    recreate_indexes(store.as_ref()).await?;

    let embedder = create_embedder();
    let builder = create_index_builder(store, embedder);
    let files = data_files()?;

    println!(
        "indexing {} txt files from {}",
        files.len(),
        DEFAULT_DATA_DIR
    );

    let mut chunk_count = 0;
    for file in files {
        let title = file
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("untitled")
            .to_owned();
        let content = fs::read_to_string(&file).map_err(|error| {
            Error::InvalidInput(format!("failed to read {}: {error}", file.display()))
        })?;
        let output = builder
            .index(BuildInput {
                content,
                title,
                kind: "text".to_owned(),
                format: ContentFormat::Text,
                tenant_id: None,
                user_id: None,
                knowledge_base_id: Some(DEMO_KNOWLEDGE_BASE_ID.to_owned()),
                metadata: json!({ "path": file.display().to_string() }),
                tags: vec!["demo".to_owned(), "faq".to_owned()],
                chunker: Some(ChunkerKind::Fixed),
                chunk_size: Some(350),
                chunk_overlap: Some(80),
                delimiter: None,
                keywords: Vec::new(),
                questions: Vec::new(),
            })
            .await?;
        chunk_count += output.chunks.len();
    }

    Ok(chunk_count)
}

pub async fn run_search() -> Result<()> {
    let store = create_store()?;
    let embedder = create_embedder();
    let engine = create_engine(store);
    let query = DEMO_QUERY.to_owned();
    let parsed_query = DefaultQueryParser.parse(&query)?;
    let query_vector = embedder.embed(&parsed_query.normalized_query).await?;
    let request = search_request(&parsed_query.text_expression, query_vector);
    let request_body = serde_json::to_string_pretty(&redacted_search_body(&request.body))?;
    println!("\nsearch index: {}", request.index_name);
    println!("search body:\n{request_body}");

    let page = engine.search(&query, request, Some(1), Some(5)).await?;

    println!("\nquery: {query}");
    println!("normalized query: {}", parsed_query.normalized_query);
    println!("text expression: {}", parsed_query.text_expression);
    println!("keywords: {:?}", parsed_query.keywords);
    println!("total hits after ranking: {}", page.total);
    for (index, hit) in page.hits.iter().enumerate() {
        let title = hit
            .source
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or("");
        let content = hit
            .source
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or("");
        println!(
            "\n#{} id={} score={:.4} title={}\n{}",
            index + 1,
            hit.id,
            hit.score,
            title,
            snippet(content, 180)
        );
        println!("scores: {:?}", hit.scores);
    }

    Ok(())
}

fn txt_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = fs::read_dir(dir)
        .map_err(|error| {
            Error::InvalidInput(format!(
                "failed to read data dir {}: {error}",
                dir.display()
            ))
        })?
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("txt"))
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn snippet(content: &str, max_chars: usize) -> String {
    let text = content.replace(['\r', '\n'], " ");
    let mut result = text.chars().take(max_chars).collect::<String>();
    if text.chars().count() > max_chars {
        result.push_str("...");
    }
    result
}

fn redacted_search_body(body: &Value) -> Value {
    let mut body = body.clone();
    if let Some(query_vector) = body.pointer_mut("/knn/query_vector") {
        let vector_len = query_vector.as_array().map(Vec::len).unwrap_or_default();
        *query_vector = json!({
            "redacted": true,
            "dims": vector_len
        });
    }
    body
}

pub async fn wait_for_refresh() {
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
}
