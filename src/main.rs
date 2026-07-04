use anynode::app::NodeRunner;
use anynode::cli::Cli;
use anynode::config::Config;
use anynode::initialization::{
    ensure_database_is_present, ensure_directories, ensure_required_tools,
    initialize_area_upload_service, initialize_cid_db, initialize_country_service,
    initialize_extraction_service, initialize_kubo_service, initialize_whosonfirst_db,
    print_startup_info, validate_config,
};
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};
use tracing_indicatif::IndicatifLayer;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    drop(dotenvy::dotenv());

    let cli = Cli::parse_args();

    let log_level = cli.get_log_level();
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    let indicatif_layer = IndicatifLayer::new();

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().with_writer(indicatif_layer.get_stderr_writer()))
        .with(indicatif_layer)
        .init();

    info!("AnyNode v0.1.0 starting...");

    let mut config = Config::load()?;
    config.apply_cli_overrides(&cli);
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
    let kubo_service = initialize_kubo_service(&config).await?;
    let area_ids = cli.get_area_ids(config.area_ids.clone());

    let extraction_service = initialize_extraction_service(&config, whosonfirst_db.clone())?;
    let upload_service = initialize_area_upload_service(
        cid_db.clone(),
        whosonfirst_db.clone(),
        kubo_service.clone(),
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
        kubo_service.clone(),
        extraction_service,
        upload_service,
        country_service,
        area_ids,
        cli.should_skip_extract(),
    );

    let result = tokio::select! {
        result = runner.run() => result,
        _ = signal::ctrl_c() => {
            info!("Interrupted by user");
            return Err("Interrupted".into());
        }
    };

    if let Err(e) = result {
        error!("Application error: {}", e);
        return Err(e.into());
    }

    runner.display_summary().await;

    info!("AnyNode completed successfully");
    Ok(())
}
