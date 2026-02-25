pub mod monitor;
pub mod runner;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApplicationError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] crate::services::DatabaseError),
    #[error("Extraction error: {0}")]
    ExtractionError(#[from] crate::services::ExtractionError),
    #[error("Upload error: {0}")]
    UploadError(#[from] crate::services::AreaUploadError),
    #[error("Storage error: {0}")]
    StorageError(#[from] crate::services::StorageError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub type ApplicationResult<T> = Result<T, ApplicationError>;

pub use runner::NodeRunner;
