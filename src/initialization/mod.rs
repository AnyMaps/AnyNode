pub mod database_init;
pub mod directories_init;
pub mod download_init;
pub mod init;
pub mod tools_init;
pub mod validation_init;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum InitializationError {
    #[error("Configuration error: {0}")]
    ConfigError(#[from] crate::config::ConfigError),
    #[error("Database error: {0}")]
    DatabaseError(#[from] crate::services::DatabaseError),
    #[error("Storage error: {0}")]
    StorageError(#[from] crate::services::StorageError),
    #[error("Extraction error: {0}")]
    ExtractionError(#[from] crate::services::ExtractionError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Directory not found: {0}")]
    DirectoryNotFound(String),
    #[error("Download error: {0}")]
    DownloadError(#[from] crate::utils::FileError),
    #[error("Command error: {0}")]
    CmdError(#[from] crate::utils::CmdError),
    #[error("Database is missing and download is disabled")]
    DatabaseMissing,
}

pub type InitializationResult<T> = Result<T, InitializationError>;

pub use database_init::{initialize_cid_db, initialize_whosonfirst_db};
pub use directories_init::ensure_directories;
pub use download_init::ensure_database_is_present;
pub use init::{
    initialize_country_service, initialize_extraction_service, initialize_locality_upload_service,
    initialize_storage_service, print_final_stats, print_startup_info,
};
pub use tools_init::ensure_required_tools;
pub use validation_init::validate_config;
