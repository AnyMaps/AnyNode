use anynode::app::NodeRunner;
use anynode::cli::Cli;
use anynode::config::Config;
use anynode::initialization::{
    ensure_database_is_present, ensure_directories, ensure_required_tools, initialize_cid_db,
    initialize_country_service, initialize_extraction_service, initialize_area_upload_service,
    initialize_storage_service, initialize_whosonfirst_db, print_startup_info, validate_config,
};
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse_args();

    let log_level = cli.get_log_level();
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    // Set up tracing with indicatif layer to keep progress bar visible
    let indicatif_layer = IndicatifLayer::new();

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(indicatif_layer.get_stderr_writer()))
        .with(indicatif_layer)
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
    let bootstrap_nodes = cli.get_bootstrap_nodes(config.bootstrap_nodes.clone());
    let nat = cli.get_nat(config.nat.clone());
    let listen_addrs = cli.get_listen_addrs(config.listen_addrs.clone());
    let storage_service = initialize_storage_service(
        &config,
        cli.get_port(Some(config.discovery_port)),
        cli.get_data_dir(Some(config.storage_data_dir.clone())),
        bootstrap_nodes,
        Some(nat),
        Some(listen_addrs),
    )
    .await?;
    let area_ids = cli.get_area_ids(config.area_ids.clone());

    let extraction_service = initialize_extraction_service(&config, whosonfirst_db.clone())?;
    let upload_service = initialize_area_upload_service(
        cid_db.clone(),
        whosonfirst_db.clone(),
        storage_service.clone(),
        &config,
        area_ids.clone(),
    )?;

    if !area_ids.is_empty() {
        info!("Processing {} specific area IDs", area_ids.len());
    } else {
        info!("Retrieving list of all countries...");
        let countries = country_service.get_countries_to_process(&config.target_countries);
        info!("Processing {} countries", countries.len());
    }

    let runner = NodeRunner::new(
        config.clone(),
        storage_service.clone(),
        extraction_service,
        upload_service,
        country_service,
        area_ids,
        cli.should_skip_extract(),
    );

    if let Err(e) = runner.run().await {
        error!("Application error: {}", e);
        return Err(e.into());
    }

    info!("Press Ctrl+C to stop the node gracefully");

    let monitor_handle = runner.start_monitoring();

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

    monitor_handle.abort();

    runner.shutdown().await?;

    info!("AnyNode shutdown complete");
    Ok(())
}
