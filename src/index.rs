use crate::{BoxFuture, Result};

pub mod builder;
pub mod chunker;
pub mod parser;
pub mod types;

pub use builder::DefaultIndexBuilder;
pub use types::{
    BuildInput, BuildOutput, ChunkerKind, ContentFormat, DefaultChunk, DefaultDocument,
    ExtractedDocument, KeywordExtractionInput, ParsedContent, Piece, Positions,
};

pub trait IndexBuilder: Send + Sync + std::fmt::Debug {
    fn build(&self, input: BuildInput) -> BoxFuture<'_, Result<BuildOutput>>;

    fn index(&self, input: BuildInput) -> BoxFuture<'_, Result<BuildOutput>>;
}

pub trait Tokenizer: Send + Sync + std::fmt::Debug {
    fn tokenize(&self, text: &str) -> Vec<String>;
}

pub trait KeywordExtractor: Send + Sync + std::fmt::Debug {
    fn extract_keywords<'a>(
        &'a self,
        input: KeywordExtractionInput<'a>,
    ) -> BoxFuture<'a, Result<Vec<String>>>;
}

pub trait Extractor: Send + Sync + std::fmt::Debug {
    fn extract<'a>(&'a self, input: &'a BuildInput) -> BoxFuture<'a, Result<ExtractedDocument>>;
}

pub trait Parser: Send + Sync + std::fmt::Debug {
    fn parse(&self, extracted: &ExtractedDocument, format: ContentFormat) -> Result<ParsedContent>;
}

pub trait Chunker: Send + Sync + std::fmt::Debug {
    fn chunk(
        &self,
        parsed: ParsedContent,
        kind: ChunkerKind,
        chunk_size: usize,
        chunk_overlap: usize,
        delimiter: Option<&str>,
    ) -> Result<Vec<Piece>>;
}
