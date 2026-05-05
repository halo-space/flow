use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::index::{
    BuildInput, BuildOutput, ChunkPipeline, ChunkPipelineInput, ChunkerKind, DefaultChunk,
    DefaultDocument, ExtractedDocument, Extractor, IndexBuilder, KeywordExtractionInput,
    KeywordExtractor, ParseInput, Parser, Piece, Tokenizer, chunker::Pipeline,
    parser::Pipeline as ParserPipeline,
};
use crate::store::{Item, Store};
use crate::{BoxFuture, Embedder, Error, Result};

const DEFAULT_CHUNKER: ChunkerKind = ChunkerKind::Fixed;
const DEFAULT_CHUNK_SIZE: usize = 800;
const DEFAULT_CHUNK_OVERLAP: usize = 100;
const DEFAULT_KEYWORD_TOP: usize = 3;

#[derive(Debug)]
pub struct DefaultIndexBuilder {
    store: Option<Arc<dyn Store>>,
    extractor: Arc<dyn Extractor>,
    parser: Arc<dyn Parser>,
    chunker: Arc<dyn ChunkPipeline>,
    tokenizer: Arc<dyn Tokenizer>,
    keyword_extractor: Option<Arc<dyn KeywordExtractor>>,
    embedder: Option<Arc<dyn Embedder>>,
    index_name: String,
    chunker_kind: ChunkerKind,
    chunk_size: usize,
    chunk_overlap: usize,
    delimiter: Option<String>,
    keyword_top: usize,
}

impl DefaultIndexBuilder {
    #[must_use]
    pub fn new(
        store: Option<Arc<dyn Store>>,
        embedder: Option<Arc<dyn Embedder>>,
        index_name: impl Into<String>,
    ) -> Self {
        Self::with_keyword_extractor(store, None, embedder, index_name)
    }

    #[must_use]
    pub fn with_keyword_extractor(
        store: Option<Arc<dyn Store>>,
        keyword_extractor: Option<Arc<dyn KeywordExtractor>>,
        embedder: Option<Arc<dyn Embedder>>,
        index_name: impl Into<String>,
    ) -> Self {
        Self {
            store,
            extractor: Arc::new(PlainTextExtractor),
            parser: Arc::new(ParserPipeline::default()),
            chunker: Arc::new(Pipeline::default()),
            tokenizer: Arc::new(SimpleTokenizer),
            keyword_extractor,
            embedder,
            index_name: index_name.into(),
            chunker_kind: DEFAULT_CHUNKER,
            chunk_size: DEFAULT_CHUNK_SIZE,
            chunk_overlap: DEFAULT_CHUNK_OVERLAP,
            delimiter: None,
            keyword_top: DEFAULT_KEYWORD_TOP,
        }
    }

    #[must_use]
    pub fn with_chunking(
        mut self,
        chunker_kind: ChunkerKind,
        chunk_size: usize,
        chunk_overlap: usize,
        delimiter: Option<String>,
    ) -> Self {
        self.chunker_kind = chunker_kind;
        self.chunk_size = chunk_size;
        self.chunk_overlap = chunk_overlap;
        self.delimiter = delimiter;
        self
    }

    #[must_use]
    pub fn with_keyword_top(mut self, keyword_top: usize) -> Self {
        self.keyword_top = keyword_top;
        self
    }

    fn build_document(&self, extracted: &ExtractedDocument, input: &BuildInput) -> DefaultDocument {
        DefaultDocument {
            id: Uuid::new_v4().to_string(),
            tenant_id: input.tenant_id.clone(),
            user_id: input.user_id.clone(),
            knowledge_base_id: input.knowledge_base_id.clone(),
            title: extracted.title.clone(),
            content: extracted.content.clone(),
            hash_id: None,
            size: extracted.size,
            created_at: Utc::now(),
            metadata: input.metadata.clone(),
            kind: extracted.kind.clone(),
        }
    }

    fn build_chunk(
        &self,
        piece: Piece,
        document: &DefaultDocument,
        input: &BuildInput,
    ) -> DefaultChunk {
        DefaultChunk {
            id: Uuid::new_v4().to_string(),
            tenant_id: document.tenant_id.clone(),
            user_id: document.user_id.clone(),
            knowledge_base_id: document.knowledge_base_id.clone(),
            doc_id: document.id.clone(),
            title: document.title.clone(),
            title_tokens: Vec::new(),
            content: piece.content,
            content_tokens: Vec::new(),
            keywords: input.keywords.clone(),
            keyword_tokens: Vec::new(),
            questions: if piece.questions.is_empty() {
                input.questions.clone()
            } else {
                piece.questions
            },
            question_tokens: Vec::new(),
            embedding: None,
            positions: piece.positions,
            tags: input.tags.clone(),
            images: Vec::new(),
        }
    }

    async fn extract_keywords(&self, chunk: &mut DefaultChunk) -> Result<()> {
        let Some(keyword_extractor) = &self.keyword_extractor else {
            return Ok(());
        };
        if !chunk.keywords.is_empty() {
            return Ok(());
        }

        chunk.keywords = keyword_extractor
            .extract_keywords(KeywordExtractionInput {
                title: &chunk.title,
                content: &chunk.content,
                top: self.keyword_top,
            })
            .await?
            .into_iter()
            .map(|keyword| keyword.trim().to_owned())
            .filter(|keyword| !keyword.is_empty())
            .collect();

        Ok(())
    }

    fn tokenize_chunk(&self, chunk: &mut DefaultChunk) {
        chunk.title_tokens = self.tokenizer.tokenize(&chunk.title);
        chunk.content_tokens = self.tokenizer.tokenize(&chunk.content);
        chunk.keyword_tokens = chunk
            .keywords
            .iter()
            .flat_map(|keyword| self.tokenizer.tokenize(keyword))
            .collect();
        chunk.question_tokens = chunk
            .questions
            .iter()
            .flat_map(|question| self.tokenizer.tokenize(question))
            .collect();
    }
}

impl IndexBuilder for DefaultIndexBuilder {
    fn build(&self, input: BuildInput) -> BoxFuture<'_, Result<BuildOutput>> {
        Box::pin(async move {
            tracing::info!(title = %input.title, kind = %input.kind, "index.build.start");

            let extracted = self.extractor.extract(&input).await?;
            let parsed = self.parser.parse(ParseInput {
                extracted: &extracted,
                format: input.format,
            })?;
            let pieces = self.chunker.chunk(ChunkPipelineInput {
                parsed,
                kind: input.chunker.unwrap_or(self.chunker_kind),
                chunk_size: input.chunk_size.unwrap_or(self.chunk_size),
                chunk_overlap: input.chunk_overlap.unwrap_or(self.chunk_overlap),
                delimiter: input.delimiter.as_deref().or(self.delimiter.as_deref()),
            })?;

            let document = self.build_document(&extracted, &input);
            let mut chunks: Vec<DefaultChunk> = pieces
                .into_iter()
                .map(|piece| self.build_chunk(piece, &document, &input))
                .collect();

            for chunk in &mut chunks {
                self.extract_keywords(chunk).await?;
                self.tokenize_chunk(chunk);
                if let Some(embedder) = &self.embedder {
                    chunk.embedding = Some(embedder.embed(&chunk.content).await?);
                }
            }

            let document_id = document.id.clone();
            let chunk_items = chunks
                .into_iter()
                .map(|chunk| {
                    Ok(Item {
                        id: chunk.id.clone(),
                        source: serde_json::to_value(chunk)?,
                    })
                })
                .collect::<Result<Vec<_>>>()?;

            tracing::info!(document_id = %document_id, chunk_count = chunk_items.len(), "index.build.done");
            Ok(BuildOutput {
                document: Item {
                    id: document_id,
                    source: serde_json::to_value(document)?,
                },
                chunks: chunk_items,
            })
        })
    }

    fn index(&self, input: BuildInput) -> BoxFuture<'_, Result<BuildOutput>> {
        Box::pin(async move {
            let output = self.build(input).await?;
            let store = self
                .store
                .as_ref()
                .ok_or_else(|| Error::InvalidInput("store is required for index()".to_owned()))?;
            let chunk_items = output.chunks.clone();

            store.batch_insert(&self.index_name, chunk_items).await?;

            tracing::info!(document_id = %output.document.id, chunk_count = output.chunks.len(), "index.store.done");
            Ok(output)
        })
    }
}

#[derive(Debug)]
pub struct PlainTextExtractor;

impl Extractor for PlainTextExtractor {
    fn extract<'a>(&'a self, input: &'a BuildInput) -> BoxFuture<'a, Result<ExtractedDocument>> {
        Box::pin(async move {
            Ok(ExtractedDocument {
                title: input.title.clone(),
                content: input.content.clone(),
                kind: input.kind.clone(),
                size: input.content.len(),
            })
        })
    }
}

#[derive(Debug)]
pub struct SimpleTokenizer;

impl Tokenizer for SimpleTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        let normalized = text
            .chars()
            .map(|ch| {
                if ch.is_ascii_punctuation() || ch.is_whitespace() {
                    ' '
                } else {
                    ch
                }
            })
            .collect::<String>()
            .to_lowercase();

        normalized
            .split_whitespace()
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    use crate::index::{ContentFormat, parser::Qa};
    use crate::store::SearchHit;

    #[test]
    fn tokenizer_keeps_words() {
        let tokenizer = SimpleTokenizer;
        assert_eq!(
            tokenizer.tokenize("RAGFlow 支持 API!"),
            vec!["ragflow", "支持", "api"]
        );
    }

    #[derive(Debug)]
    struct StaticKeywordExtractor;

    impl KeywordExtractor for StaticKeywordExtractor {
        fn extract_keywords<'a>(
            &'a self,
            _input: KeywordExtractionInput<'a>,
        ) -> BoxFuture<'a, Result<Vec<String>>> {
            Box::pin(async move { Ok(vec!["自动关键词".to_owned(), "提取".to_owned()]) })
        }
    }

    #[tokio::test]
    async fn builder_fills_keywords_when_extractor_exists() {
        let builder = DefaultIndexBuilder::with_keyword_extractor(
            None,
            Some(Arc::new(StaticKeywordExtractor)),
            None,
            "chunks",
        );

        let output = builder
            .build(BuildInput {
                content: "这是测试内容".to_owned(),
                title: "测试标题".to_owned(),
                kind: "text".to_owned(),
                format: ContentFormat::Text,
                tenant_id: None,
                user_id: None,
                knowledge_base_id: None,
                metadata: serde_json::json!({}),
                tags: vec![],
                chunker: None,
                chunk_size: None,
                chunk_overlap: None,
                delimiter: None,
                keywords: vec![],
                questions: vec![],
            })
            .await
            .expect("build should succeed");

        let chunk = output.chunks[0].source.clone();
        assert_eq!(chunk["keywords"], serde_json::json!(["自动关键词", "提取"]));
        assert_eq!(
            chunk["keyword_tokens"],
            serde_json::json!(["自动关键词", "提取"])
        );
    }

    #[derive(Debug, Default)]
    struct RecordingStore {
        batch_insert_indexes: Mutex<Vec<String>>,
    }

    impl Store for RecordingStore {
        fn create_schema<'a>(
            &'a self,
            _index_name: &'a str,
            _schema: serde_json::Value,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn insert<'a>(&'a self, index_name: &'a str, _item: Item) -> BoxFuture<'a, Result<()>> {
            Box::pin(async move {
                Err(Error::External(format!(
                    "unexpected document insert into {index_name}"
                )))
            })
        }

        fn batch_insert<'a>(
            &'a self,
            index_name: &'a str,
            _items: Vec<Item>,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async move {
                self.batch_insert_indexes
                    .lock()
                    .unwrap()
                    .push(index_name.to_owned());
                Ok(())
            })
        }

        fn update<'a>(
            &'a self,
            _index_name: &'a str,
            _id: &'a str,
            _fields: serde_json::Value,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn delete<'a>(
            &'a self,
            _index_name: &'a str,
            _query: serde_json::Value,
        ) -> BoxFuture<'a, Result<()>> {
            Box::pin(async { Ok(()) })
        }

        fn get<'a>(
            &'a self,
            _index_name: &'a str,
            _id: &'a str,
        ) -> BoxFuture<'a, Result<Option<serde_json::Value>>> {
            Box::pin(async { Ok(None) })
        }

        fn search<'a>(
            &'a self,
            _index_name: &'a str,
            _body: serde_json::Value,
        ) -> BoxFuture<'a, Result<Vec<SearchHit>>> {
            Box::pin(async { Ok(Vec::new()) })
        }
    }

    #[tokio::test]
    async fn index_writes_only_chunks() {
        let store = Arc::new(RecordingStore::default());
        let builder = DefaultIndexBuilder::new(Some(store.clone()), None, "chunks_only");

        builder
            .index(BuildInput {
                content: "只写 chunk，不写 document。".to_owned(),
                title: "chunk only".to_owned(),
                kind: "text".to_owned(),
                format: ContentFormat::Text,
                tenant_id: None,
                user_id: None,
                knowledge_base_id: None,
                metadata: serde_json::json!({}),
                tags: vec![],
                chunker: None,
                chunk_size: None,
                chunk_overlap: None,
                delimiter: None,
                keywords: vec![],
                questions: vec![],
            })
            .await
            .expect("index should succeed");

        assert_eq!(
            *store.batch_insert_indexes.lock().unwrap(),
            vec!["chunks_only".to_owned()]
        );
    }

    #[test]
    fn qa_parser_extracts_pairs() {
        let extracted = ExtractedDocument {
            title: "QA".to_owned(),
            content: "Q: 如何重置密码？\nA: 点击忘记密码。\n\nQ: 支持什么文件？\nA: txt。"
                .to_owned(),
            kind: "qa".to_owned(),
            size: 0,
        };
        let parsed = Qa
            .parse(ParseInput {
                extracted: &extracted,
                format: ContentFormat::Qa,
            })
            .unwrap();
        assert_eq!(parsed.pieces.len(), 2);
        assert_eq!(parsed.pieces[0].questions[0], "如何重置密码？");
    }
}
