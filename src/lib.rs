pub mod error;
pub mod executor;
pub mod manager;
pub mod proto;
pub mod server;

pub use error::InfernoError;

pub type Result<T> = std::result::Result<T, error::InfernoError>;
