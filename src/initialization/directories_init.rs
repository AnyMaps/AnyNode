use crate::config::Config;
use std::path::PathBuf;
use tracing::info;

use super::InitializationResult;

pub async fn ensure_directories(config: &Config) -> InitializationResult<()> {
    info!("Ensuring required directories exist");

    if !config.localities_dir.exists() {
        tokio::fs::create_dir_all(&config.localities_dir).await?;
        info!("Created localities directory: {:?}", config.localities_dir);
    }

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
