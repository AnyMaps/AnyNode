use crate::config::Config;
use crate::services::DatabaseService;
use std::sync::Arc;
use tracing::info;

use super::{InitializationError, InitializationResult};

pub async fn initialize_whosonfirst_db(
    config: &Config,
) -> InitializationResult<Arc<DatabaseService>> {
    info!(
        "Initializing WhosOnFirst database at {:?}",
        config.whosonfirst_db_path
    );

    let db_path = config.whosonfirst_db_path.to_str().ok_or_else(|| {
        InitializationError::InvalidPath(format!(
            "WhosOnFirst database path: {}",
            config.whosonfirst_db_path.display()
        ))
    })?;
    let db = DatabaseService::new(db_path, false).await?;

    info!("WhosOnFirst database initialized successfully");
    Ok(Arc::new(db))
}

pub async fn initialize_cid_db(config: &Config) -> InitializationResult<Arc<DatabaseService>> {
    info!(
        "Initializing CID mappings database at {:?}",
        config.cid_db_path
    );

    if let Some(parent) = config.cid_db_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let cid_path = config.cid_db_path.to_str().ok_or_else(|| {
        InitializationError::InvalidPath(format!(
            "CID database path: {}",
            config.cid_db_path.display()
        ))
    })?;
    let db = DatabaseService::new(cid_path, true).await?;

    info!("CID mappings database initialized successfully");
    Ok(Arc::new(db))
}
