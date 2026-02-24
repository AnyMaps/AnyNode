use crate::config::Config;
use crate::services::DatabaseService;
use std::sync::Arc;
use tracing::info;

use super::InitializationResult;

pub async fn initialize_whosonfirst_db(config: &Config) -> InitializationResult<Arc<DatabaseService>> {
    info!("Initializing WhosOnFirst database at {:?}", config.whosonfirst_db_path);

    let db = DatabaseService::new(
        config.whosonfirst_db_path.to_str().unwrap(),
        false, // Don't create CID tables for WhosOnFirst DB
    )
    .await?;

    info!("WhosOnFirst database initialized successfully");
    Ok(Arc::new(db))
}

pub async fn initialize_cid_db(config: &Config) -> InitializationResult<Arc<DatabaseService>> {
    info!("Initializing CID mappings database at {:?}", config.cid_db_path);

    if let Some(parent) = config.cid_db_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let db = DatabaseService::new(
        config.cid_db_path.to_str().unwrap(),
        true, // Create CID tables
    )
    .await?;

    info!("CID mappings database initialized successfully");
    Ok(Arc::new(db))
}
