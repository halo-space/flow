pub mod default;
pub mod parse;
pub mod rrf;
pub mod scorer;
pub mod types;

pub use default::DefaultQueryEngine;
pub use parse::DefaultQueryParser;
pub use scorer::{LocalScorer, QueryScorer};
pub use types::{Hit, HitPage, ParseQuery, QueryLanguage, Scores};

use serde_json::Value;

use crate::{BoxFuture, Result};

pub trait QueryEngine: Send + Sync + std::fmt::Debug {
    fn search<'a>(
        &'a self,
        query: &'a str,
        index_name: &'a str,
        body: Value,
        page_num: Option<usize>,
        page_size: Option<usize>,
    ) -> BoxFuture<'a, Result<HitPage>>;
}

pub trait QueryParser: Send + Sync + std::fmt::Debug {
    fn parse(&self, query: &str) -> Result<ParseQuery>;
}

pub trait Reranker: Send + Sync + std::fmt::Debug {
    fn rerank<'a>(&'a self, query: &'a str, hits: &'a [Hit]) -> BoxFuture<'a, Result<Vec<f32>>>;
}
