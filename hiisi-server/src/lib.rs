pub mod error;
pub mod executor;
pub mod io;
pub mod manager;
pub mod proto;
pub mod server;

pub type Result<T> = std::result::Result<T, error::HiisiError>;

pub use error::HiisiError;
pub use manager::ResourceManager;
pub use server::{serve, Context, IO};
