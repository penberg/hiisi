use thiserror::Error;

#[derive(Debug, Error)]
pub enum HiisiError {
    #[error("Protocol error: {0}")]
    ProtocolError(String),
    #[error("Parse error: {0}")]
    JsonParseError(#[from] serde_json::Error),
    #[error("Internal error: {0}")]
    InternalError(String),
    #[error("I/O error: {0}: {1}")]
    IOError(&'static str, std::io::Error),
    #[error("Out of memory")]
    OutOfMemory,
    #[error("SQL error: {0}")]
    SQLError(#[from] libsql::Error),
}
