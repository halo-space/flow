use std::sync::Arc;

use chrono::Utc;
use uuid::Uuid;

use crate::index::{
    BuildInput, BuildOutput, Chunker, ChunkerKind, ContentFormat, DefaultChunk, DefaultDocument,
    ExtractedDocument, Extractor, IndexBuilder, KeywordExtractionInput, KeywordExtractor,
    ParsedContent, Parser, Piece, Tokenizer, chunker, parser,
};
use crate::store::{Item, Store};
use crate::{BoxFuture, Embedder, Error, Result};

#[derive(Debug, Clone)]
pub struct IndexBuilderConfig {
    pub documents_index: String,
    pub chunks_index: String,
    pub chunker: ChunkerKind,
    pub chunk_size: usize,
    pub chunk_overlap: usize,
    pub delimiter: Option<String>,
    pub keyword_top: usize,
}

impl Default for IndexBuilderConfig {
    fn default() -> Self {
        Self {
            documents_index: "documents".to_owned(),
            chunks_index: "chunks".to_owned(),
            chunker: ChunkerKind::Fixed,
            chunk_size: 800,
            chunk_overlap: 100,
            delimiter: None,
            keyword_top: 3,
        }
    }
}

#[derive(Debug)]
pub struct DefaultIndexBuilder {
    store: Option<Arc<dyn Store>>,
    extractor: Arc<dyn Extractor>,
    parser: Arc<dyn Parser>,
    chunker: Arc<dyn Chunker>,
    tokenizer: Arc<dyn Tokenizer>,
    keyword_extractor: Option<Arc<dyn KeywordExtractor>>,
    embedder: Option<Arc<dyn Embedder>>,
    config: IndexBuilderConfig,
}

impl DefaultIndexBuilder {
    #[must_use]
    pub fn new(
        store: Option<Arc<dyn Store>>,
        embedder: Option<Arc<dyn Embedder>>,
        config: IndexBuilderConfig,
    ) -> Self {
        Self::with_keyword_extractor(store, None, embedder, config)
    }

    #[must_use]
    pub fn with_keyword_extractor(
        store: Option<Arc<dyn Store>>,
        keyword_extractor: Option<Arc<dyn KeywordExtractor>>,
        embedder: Option<Arc<dyn Embedder>>,
        config: IndexBuilderConfig,
    ) -> Self {
        Self {
            store,
            extractor: Arc::new(PlainTextExtractor),
            parser: Arc::new(DefaultParser),
            chunker: Arc::new(DefaultChunker),
            tokenizer: Arc::new(SimpleTokenizer),
            keyword_extractor,
            embedder,
            config,
        }
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
                top: self.config.keyword_top,
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
            let parsed = self.parser.parse(&extracted, input.format)?;
            let pieces = self.chunker.chunk(
                parsed,
                input.chunker.unwrap_or(self.config.chunker),
                input.chunk_size.unwrap_or(self.config.chunk_size),
                input.chunk_overlap.unwrap_or(self.config.chunk_overlap),
                input
                    .delimiter
                    .as_deref()
                    .or(self.config.delimiter.as_deref()),
            )?;

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
            let document_id = output.document.id.clone();
            let chunk_items = output.chunks.clone();

            store
                .insert(
                    &self.config.documents_index,
                    Item {
                        id: document_id,
                        source: output.document.source.clone(),
                    },
                )
                .await?;
            store
                .batch_insert(&self.config.chunks_index, chunk_items)
                .await?;

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
pub struct DefaultParser;

impl Parser for DefaultParser {
    fn parse(&self, extracted: &ExtractedDocument, format: ContentFormat) -> Result<ParsedContent> {
        match format {
            ContentFormat::Text => Ok(parser::text::parse(extracted)),
            ContentFormat::Qa => parser::qa::parse(&extracted.content),
        }
    }
}

#[derive(Debug)]
pub struct DefaultChunker;

impl Chunker for DefaultChunker {
    fn chunk(
        &self,
        parsed: ParsedContent,
        kind: ChunkerKind,
        chunk_size: usize,
        chunk_overlap: usize,
        delimiter: Option<&str>,
    ) -> Result<Vec<Piece>> {
        Ok(chunker::default::chunk(
            parsed,
            kind,
            chunk_size,
            chunk_overlap,
            delimiter,
        ))
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
            IndexBuilderConfig::default(),
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

    #[test]
    fn qa_parser_extracts_pairs() {
        let parsed = parser::qa::parse(
            "Q: 如何重置密码？\nA: 点击忘记密码。\n\nQ: 支持什么文件？\nA: txt。",
        )
        .unwrap();
        assert_eq!(parsed.pieces.len(), 2);
        assert_eq!(parsed.pieces[0].questions[0], "如何重置密码？");
    }
}
