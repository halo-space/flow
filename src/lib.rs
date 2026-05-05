#![deny(unsafe_code)]
#![warn(missing_debug_implementations)]

pub mod embedding;
pub mod error;
pub mod index;
pub mod query;
pub mod store;
pub mod utils;

pub use crate::embedding::Embedder;
pub use crate::error::{Error, Result};
pub use crate::index::{DefaultIndexBuilder, IndexBuilder, IndexBuilderConfig, KeywordExtractor};
pub use crate::query::{DefaultQueryEngine, LocalScorer, QueryEngine, QueryScorer};
pub use crate::store::{Elastic, Store};

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;
