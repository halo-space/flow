use serde_json::Value;

use crate::{BoxFuture, Result};

pub mod elastic;
pub mod types;

pub use elastic::Elastic;
pub use types::{Item, SearchHit, SearchRequest};

pub trait Store: Send + Sync + std::fmt::Debug {
    fn create_schema<'a>(&'a self, index_name: &'a str, schema: Value)
    -> BoxFuture<'a, Result<()>>;

    fn insert<'a>(&'a self, index_name: &'a str, item: Item) -> BoxFuture<'a, Result<()>>;

    fn batch_insert<'a>(
        &'a self,
        index_name: &'a str,
        items: Vec<Item>,
    ) -> BoxFuture<'a, Result<()>>;

    fn update<'a>(
        &'a self,
        index_name: &'a str,
        id: &'a str,
        fields: Value,
    ) -> BoxFuture<'a, Result<()>>;

    fn delete<'a>(&'a self, index_name: &'a str, query: Value) -> BoxFuture<'a, Result<()>>;

    fn get<'a>(&'a self, index_name: &'a str, id: &'a str) -> BoxFuture<'a, Result<Option<Value>>>;

    fn search(&self, request: SearchRequest) -> BoxFuture<'_, Result<Vec<SearchHit>>>;
}
