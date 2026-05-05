use crate::{BoxFuture, Result};

pub trait Embedder: Send + Sync + std::fmt::Debug {
    fn embed<'a>(&'a self, text: &'a str) -> BoxFuture<'a, Result<Vec<f32>>>;
}
