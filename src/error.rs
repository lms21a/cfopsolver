use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid facelet state: {0}")]
    InvalidState(String),
    #[error("invalid move or algorithm: {0}")]
    InvalidAlgorithm(String),
    #[error("solver failed: {0}")]
    Solver(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
}
