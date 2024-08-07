use thiserror::Error;

#[derive(Debug, Error)]
pub enum InfernoError {
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    #[error("Parse error: {0}")]
    JsonParseError(#[from] serde_json::Error),
    #[error("Internal error: {0}")]
    InternalError(String),
    #[error("Out of memory")]
    OutOfMemory,
    #[error("SQL error: {0}")]
    SQLError(#[from] libsql::Error),
}
