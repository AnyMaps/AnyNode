use crate::config::Config;
use tracing::info;

use super::{InitializationError, InitializationResult};

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
