use crate::config::Config;
use tracing::info;

use super::InitializationResult;

pub async fn ensure_required_tools(config: &Config) -> InitializationResult<()> {
    info!("Ensuring required tools are present");
    crate::utils::ensure_tools_are_present(&[&config.bzip2_cmd, &config.pmtiles_cmd]).await?;
    info!("All required tools are present");
    Ok(())
}
