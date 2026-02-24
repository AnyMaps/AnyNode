use crate::config::Config;
use crate::services::{
    CountryService, DatabaseService, ExtractionService, LocalityUploadService, StorageService,
};
use crate::types::UploadStats;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;

pub fn initialize_country_service(whosonfirst_db: Arc<DatabaseService>) -> CountryService {
    info!("Initializing country service");
    let country_service = CountryService::new(whosonfirst_db);
    info!("Country service initialized successfully");
    country_service
}

pub async fn initialize_storage_service(
    config: &Config,
    port_override: Option<u16>,
    data_dir_override: Option<PathBuf>,
    bootstrap_nodes: Vec<String>,
    nat_override: Option<String>,
    listen_addrs_override: Option<Vec<String>>,
) -> super::InitializationResult<Arc<StorageService>> {
    info!("Initializing storage service");

    let port = port_override.unwrap_or(config.discovery_port);
    let data_dir = data_dir_override.unwrap_or_else(|| config.storage_data_dir.clone());
    let nat = nat_override.unwrap_or_else(|| config.nat.clone());
    let listen_addrs = listen_addrs_override.unwrap_or_else(|| config.listen_addrs.clone());

    tokio::fs::create_dir_all(&data_dir).await?;

    if !bootstrap_nodes.is_empty() {
        info!("Using {} bootstrap node(s)", bootstrap_nodes.len());
    }

    info!("Using NAT configuration: {}", nat);
    info!("Using listen addresses: {:?}", listen_addrs);

    let storage_service = StorageService::new(
        &data_dir,
        config.storage_quota,
        port,
        config.max_peers,
        bootstrap_nodes,
        nat,
        listen_addrs,
    )
    .await?;

    info!("Storage service initialized successfully");
    Ok(Arc::new(storage_service))
}

pub fn initialize_extraction_service(
    config: &Arc<Config>,
    whosonfirst_db: Arc<DatabaseService>,
) -> super::InitializationResult<ExtractionService> {
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
) -> super::InitializationResult<LocalityUploadService> {
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
