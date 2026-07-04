use crate::config::Config;
use crate::services::{
    AreaUploadService, CountryService, DatabaseService, ExtractionService, KuboService,
};
use crate::types::UploadStats;
use std::sync::Arc;
use tracing::info;

pub fn initialize_country_service() -> CountryService {
    info!("Initializing country service");
    let country_service = CountryService::new();
    info!("Country service initialized successfully");
    country_service
}

pub async fn initialize_kubo_service(
    config: &Config,
) -> super::InitializationResult<Arc<KuboService>> {
    info!("Initializing Kubo service");

    let kubo_service = KuboService::new(config)?;
    kubo_service.check_alive().await?;

    info!("Connected to Kubo at {}", config.kubo_api_url);
    Ok(Arc::new(kubo_service))
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

pub fn initialize_area_upload_service(
    cid_db: Arc<DatabaseService>,
    whosonfirst_db: Arc<DatabaseService>,
    kubo: Arc<KuboService>,
    config: &Config,
    area_ids: Vec<u32>,
) -> super::InitializationResult<AreaUploadService> {
    info!("Initializing area upload service");

    let upload_service = AreaUploadService::new(
        cid_db,
        whosonfirst_db,
        kubo,
        config.areas_dir.clone(),
        config.target_countries.clone(),
        area_ids,
    );

    info!("Area upload service initialized successfully");
    Ok(upload_service)
}

pub fn print_startup_info(config: &Config, cli: &crate::cli::Cli) {
    info!("=== AnyNode Starting ===");
    info!("WhosOnFirst DB: {:?}", config.whosonfirst_db_path);
    info!("CID Mappings DB: {:?}", config.cid_db_path);
    info!("Areas Dir: {:?}", config.areas_dir);
    info!("Planet PMTiles: {:?}", config.planet_pmtiles_location);
    info!("Kubo API URL: {}", config.kubo_api_url);
    info!("Pin on Upload: {}", config.pin_on_upload);
    info!(
        "Max Concurrent Extractions: {}",
        config.max_concurrent_extractions
    );
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
