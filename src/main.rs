use anynode::cli::Cli;
use anynode::config::Config;
use anynode::initialization::{
    ensure_database_is_present, ensure_directories, ensure_required_tools, initialize_cid_db,
    initialize_country_service, initialize_extraction_service, initialize_locality_upload_service,
    initialize_storage_service, initialize_whosonfirst_db, print_final_stats, print_startup_info,
    validate_config,
};
use anynode::services::LocalityUploadService;
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info, warn};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse_args();

    let log_level = cli.get_log_level();
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(true)
        .init();

    info!("AnyNode v0.1.0 starting...");

    let config = Config::load()?;
    let config = Arc::new(config);

    print_startup_info(&config, &cli);

    if let Err(e) = ensure_required_tools(&config).await {
        error!("Failed to ensure required tools: {}", e);
        return Err(e.into());
    }

    if let Err(e) = ensure_database_is_present(&config, &cli).await {
        error!("Failed to ensure database is present: {}", e);
        return Err(e.into());
    }

    if let Err(e) = validate_config(&config) {
        error!("Configuration validation failed: {}", e);
        return Err(e.into());
    }

    ensure_directories(&config).await?;

    let whosonfirst_db = initialize_whosonfirst_db(&config).await?;
    let cid_db = initialize_cid_db(&config).await?;
    let country_service = initialize_country_service();
    let storage_service = initialize_storage_service(
        &config,
        cli.get_port(Some(config.discovery_port)),
        cli.get_data_dir(Some(config.storage_data_dir.clone())),
    )
    .await?;
    let extraction_service = initialize_extraction_service(&config, whosonfirst_db.clone())?;
    let upload_service = initialize_locality_upload_service(
        cid_db.clone(),
        whosonfirst_db.clone(),
        storage_service.clone(),
        &config,
    )?;

    let countries = country_service.get_countries_to_process(&config.target_countries);
    info!("Processing {} countries", countries.len());

    info!("Starting storage node...");
    storage_service.start_node().await?;
    info!("Storage node started successfully");

    if !cli.should_skip_extract() {
        info!("Step 1: Extracting PMTiles from planet file...");
        if let Err(e) = extract_pmtiles(&extraction_service, &countries, &whosonfirst_db).await {
            error!("Failed to extract PMTiles: {}", e);
            warn!("Continuing with existing PMTiles if available...");
        }
    } else {
        info!("Step 1: Skipping PMTiles extraction (--no-extract flag set)");
    }

    info!("Step 2: Uploading localities to storage...");
    if let Err(e) = upload_localities(&upload_service).await {
        error!("Failed to upload localities: {}", e);
        return Err(e.into());
    }

    let stats = upload_service.get_stats().await;
    print_final_stats(&stats);

    // Get node info and display
    match storage_service.get_node_info().await {
        Ok(node_info) => {
            info!("Storage node is now running and serving files to the network...");
            if let Some(peer_id) = node_info.peer_id {
                info!("Peer ID: {}", peer_id);
            }
            if !node_info.addresses.is_empty() {
                info!("Node Addresses:");
                for addr in &node_info.addresses {
                    info!("  {}", addr);
                }
            }
            if !node_info.announce_addresses.is_empty() {
                info!("Announce Addresses:");
                for addr in &node_info.announce_addresses {
                    info!("  {}", addr);
                }
            }
            if let Some(version) = node_info.version {
                info!("Storage version: {}", version);
            }
        }
        Err(e) => {
            info!("Storage node is now running and serving files to the network...");
            warn!("Failed to get node info: {}", e);
        }
    }

    info!("Press Ctrl+C to stop the node gracefully");

    // Keep the node running until interrupted
    tokio::select! {
        _ = async {
            signal::ctrl_c().await.expect("Failed to listen for ctrl+c");
        } => {
            info!("Received Ctrl+C, shutting down gracefully...");
        }
        _ = async {
            let mut sig_term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Failed to setup SIGTERM handler");
            sig_term.recv().await;
        } => {
            info!("Received termination signal, shutting down gracefully...");
        }
    }

    // Stop the node gracefully
    info!("Stopping storage node...");
    storage_service.stop_node().await?;
    info!("Storage node stopped successfully");

    info!("AnyNode shutdown complete");
    Ok(())
}

async fn extract_pmtiles(
    extraction_service: &anynode::services::ExtractionService,
    countries: &[String],
    _whosonfirst_db: &Arc<anynode::services::DatabaseService>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Extracting PMTiles for {} countries", countries.len());

    extraction_service.extract_localities(countries).await?;

    info!("PMTiles extraction completed for all countries");
    Ok(())
}

async fn upload_localities(
    upload_service: &LocalityUploadService,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Starting locality upload process...");

    upload_service.process_all_localities().await?;

    info!("Locality upload process completed");
    Ok(())
}
