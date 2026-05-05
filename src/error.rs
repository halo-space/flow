#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("unsupported: {0}")]
    Unsupported(String),

    #[error("store error: {0}")]
    Store(String),

    #[error("external service error: {0}")]
    External(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error(transparent)]
    Elastic(#[from] elasticsearch::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
