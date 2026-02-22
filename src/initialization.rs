use crate::config::Config;
use crate::services::{
    CountryService, DatabaseService, ExtractionService, LocalityUploadService, StorageService,
};
use crate::types::UploadStats;
use crate::utils::{download_file_with_progress, run_command};
use std::io::{self, Write};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use thiserror::Error;
use tracing::{info, warn};

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

pub async fn initialize_whosonfirst_db(
    config: &Config,
) -> InitializationResult<Arc<DatabaseService>> {
    info!("Initializing WhosOnFirst database at {:?}", config.whosonfirst_db_path);

    let db = DatabaseService::new(
        config.whosonfirst_db_path.to_str().unwrap(),
        false, // Don't create CID tables for WhosOnFirst DB
    )
    .await?;

    info!("WhosOnFirst database initialized successfully");
    Ok(Arc::new(db))
}

pub async fn initialize_cid_db(
    config: &Config,
) -> InitializationResult<Arc<DatabaseService>> {
    info!("Initializing CID mappings database at {:?}", config.cid_db_path);

    if let Some(parent) = config.cid_db_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let db = DatabaseService::new(
        config.cid_db_path.to_str().unwrap(),
        true,
    )
    .await?;

    info!("CID mappings database initialized successfully");
    Ok(Arc::new(db))
}

pub fn initialize_country_service() -> CountryService {
    info!("Initializing country service");
    let country_service = CountryService::new();
    info!("Country service initialized successfully");
    country_service
}

pub async fn initialize_storage_service(
    config: &Config,
    port_override: Option<u16>,
    data_dir_override: Option<PathBuf>,
    bootstrap_nodes: Vec<String>,
) -> InitializationResult<Arc<StorageService>> {
    info!("Initializing storage service");

    let port = port_override.unwrap_or(config.discovery_port);
    let data_dir = data_dir_override.unwrap_or_else(|| config.storage_data_dir.clone());

    tokio::fs::create_dir_all(&data_dir).await?;

    if !bootstrap_nodes.is_empty() {
        info!("Using {} bootstrap node(s)", bootstrap_nodes.len());
    }

    let storage_service = StorageService::new(
        &data_dir,
        config.storage_quota,
        port,
        config.max_peers,
        bootstrap_nodes,
    )
    .await?;

    info!("Storage service initialized successfully");
    Ok(Arc::new(storage_service))
}

pub fn initialize_extraction_service(
    config: &Arc<Config>,
    whosonfirst_db: Arc<DatabaseService>,
) -> InitializationResult<ExtractionService> {
    info!("Initializing extraction service");

    let extraction_service = ExtractionService::new(config.clone(), whosonfirst_db);

    info!("Extraction service initialized successfully");
    Ok(extraction_service)
}

pub fn initialize_locality_upload_service(
    cid_db: Arc<DatabaseService>,
    whosonfirst_db: Arc<DatabaseService>,
    storage: Arc<StorageService>,
    config: &Config,
) -> InitializationResult<LocalityUploadService> {
    info!("Initializing locality upload service");

    let upload_service = LocalityUploadService::new(
        cid_db,
        whosonfirst_db,
        storage,
        config.localities_dir.clone(),
    );

    info!("Locality upload service initialized successfully");
    Ok(upload_service)
}

pub async fn ensure_directories(config: &Config) -> InitializationResult<()> {
    info!("Ensuring required directories exist");

    if !config.localities_dir.exists() {
        tokio::fs::create_dir_all(&config.localities_dir).await?;
        info!("Created localities directory: {:?}", config.localities_dir);
    }

    // Only create directory if planet_pmtiles_location is a local file path
    if let Some(planet_location) = &config.planet_pmtiles_location {
        if !planet_location.starts_with("http://") && !planet_location.starts_with("https://") {
            if let Some(parent) = PathBuf::from(planet_location).parent() {
                if !parent.exists() {
                    tokio::fs::create_dir_all(parent).await?;
                    info!("Created planet file directory: {:?}", parent);
                }
            }
        }
    }

    if let Some(parent) = config.cid_db_path.parent() {
        if !parent.exists() {
            tokio::fs::create_dir_all(parent).await?;
            info!("Created CID database directory: {:?}", parent);
        }
    }

    Ok(())
}

pub fn validate_config(config: &Config) -> InitializationResult<()> {
    info!("Validating configuration");

    if !config.whosonfirst_db_path.exists() {
        return Err(InitializationError::DirectoryNotFound(format!(
            "WhosOnFirst database not found: {:?}",
            config.whosonfirst_db_path
        )));
    }

    info!("Configuration validated successfully");
    Ok(())
}

pub async fn ensure_required_tools(config: &Config) -> InitializationResult<()> {
    info!("Ensuring required tools are present");
    crate::utils::ensure_tools_are_present(&[&config.bzip2_cmd, &config.pmtiles_cmd]).await?;
    info!("All required tools are present");
    Ok(())
}

pub async fn ensure_database_is_present(
    config: &Config,
    cli: &crate::cli::Cli,
) -> InitializationResult<()> {
    let database_path = &config.whosonfirst_db_path;
    let compressed_path = format!("{}.bz2", database_path.display());

    if database_path.exists() {
        info!("WhosOnFirst database already present.");
        return Ok(());
    }

    if Path::new(&compressed_path).exists() {
        info!("Compressed database found, decompressing...");
        decompress_database(&config.bzip2_cmd, &compressed_path).await?;
        return Ok(());
    }

    info!("WhosOnFirst database not found.");

    if !cli.should_skip_download() {
        info!("Auto-downloading WhosOnFirst database...");
        download_and_decompress_database(config, &compressed_path).await?;
        return Ok(());
    }

    if !cli.is_non_interactive() {
        print!("Do you want to download the WhosOnFirst database? This may take a while. (y/n) ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() == "y" {
            download_and_decompress_database(config, &compressed_path).await?;
            return Ok(());
        }
    }

    info!("Database download skipped.");
    Err(InitializationError::DatabaseMissing)
}

async fn download_and_decompress_database(
    config: &Config,
    compressed_path: &str,
) -> InitializationResult<()> {
    if let Some(parent) = Path::new(compressed_path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    info!("Downloading WhosOnFirst database...");
    download_file_with_progress(&config.whosonfirst_db_url, Path::new(compressed_path)).await?;
    info!("Database download completed!");

    info!("Decompressing database...");
    decompress_database(&config.bzip2_cmd, compressed_path).await?;
    info!("Database decompressed successfully!");

    Ok(())
}

async fn decompress_database(bzip2_cmd: &str, compressed_path: &str) -> InitializationResult<()> {
    let output = run_command(bzip2_cmd, &["-dv", compressed_path], None).await?;

    if !output.stderr.is_empty() {
        warn!("Decompression output: {}", output.stderr);
    }

    Ok(())
}

pub fn print_startup_info(config: &Config, cli: &crate::cli::Cli) {
    info!("=== AnyNode Starting ===");
    info!("WhosOnFirst DB: {:?}", config.whosonfirst_db_path);
    info!("CID Mappings DB: {:?}", config.cid_db_path);
    info!("Localities Dir: {:?}", config.localities_dir);
    info!("Planet PMTiles: {:?}", config.planet_pmtiles_location);
    info!("Storage Port: {}", config.discovery_port);
    info!("Storage Data Dir: {:?}", config.storage_data_dir);
    info!("Max Concurrent Extractions: {}", config.max_concurrent_extractions);
    info!("Target Countries: {:?}", config.target_countries);
    info!("Non-Interactive: {}", cli.is_non_interactive());
    info!("Skip Download: {}", cli.should_skip_download());
    info!("Skip Extract: {}", cli.should_skip_extract());
    info!("Log Level: {}", cli.get_log_level());
    info!("========================");
}

pub fn print_final_stats(stats: &UploadStats) {
    info!("=== Final Statistics ===");
    info!("Total Uploaded: {}", stats.total_uploaded);
    info!("Total Failed: {}", stats.total_failed);
    info!("Total Bytes: {} bytes", stats.total_bytes_uploaded);
    info!("========================");
}
