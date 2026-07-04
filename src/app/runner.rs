use crate::config::Config;
use crate::initialization::print_final_stats;
use crate::services::{AreaUploadService, CountryService, ExtractionService, KuboService};
use std::sync::Arc;
use tracing::{error, info, warn};

use super::ApplicationResult;

pub struct NodeRunner {
    config: Arc<Config>,
    kubo_service: Arc<KuboService>,
    extraction_service: ExtractionService,
    upload_service: AreaUploadService,
    country_service: CountryService,
    area_ids: Vec<u32>,
    skip_extract: bool,
}

impl NodeRunner {
    pub fn new(
        config: Arc<Config>,
        kubo_service: Arc<KuboService>,
        extraction_service: ExtractionService,
        upload_service: AreaUploadService,
        country_service: CountryService,
        area_ids: Vec<u32>,
        skip_extract: bool,
    ) -> Self {
        Self {
            config,
            kubo_service,
            extraction_service,
            upload_service,
            country_service,
            area_ids,
            skip_extract,
        }
    }

    pub async fn run(&self) -> ApplicationResult<()> {
        info!("Connecting to Kubo API...");
        self.kubo_service.check_alive().await?;
        info!("Connected to Kubo API successfully");

        if !self.skip_extract {
            info!("Extracting PMTiles from planet file...");
            if !self.area_ids.is_empty() {
                info!("Processing {} specific area IDs", self.area_ids.len());
                if let Err(e) = self
                    .extraction_service
                    .extract_areas_by_ids(&self.area_ids)
                    .await
                {
                    error!("Failed to extract PMTiles: {}", e);
                    warn!("Continuing with existing PMTiles if available...");
                }
            } else {
                let countries = self
                    .country_service
                    .get_countries_to_process(&self.config.target_countries);
                info!("Processing {} countries", countries.len());
                if let Err(e) = self.extraction_service.extract_areas(&countries).await {
                    error!("Failed to extract PMTiles: {}", e);
                    warn!("Continuing with existing PMTiles if available...");
                }
            }
        } else {
            info!("Skipping PMTiles extraction (--no-extract flag set)");
        }

        info!("Uploading areas to storage...");
        self.upload_service.process_areas().await?;

        let stats = self.upload_service.get_stats().await;
        print_final_stats(&stats);

        Ok(())
    }

    pub async fn display_summary(&self) {
        info!("=== AnyNode Summary ===");

        match self.kubo_service.get_node_info().await {
            Ok(node_info) => {
                info!("Kubo Node: {}", node_info.peer_id);
                info!("Kubo Version: {}", node_info.version);
                if let Some(agent) = &node_info.agent_version {
                    info!("Agent: {}", agent);
                }
                info!("Peers Connected: {}", node_info.peers_connected);
                if !node_info.addresses.is_empty() {
                    info!("Listen Addresses:");
                    for addr in &node_info.addresses {
                        info!("  - {}", addr);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to get node info for summary: {}", e);
            }
        }

        let stats = self.upload_service.get_stats().await;
        info!("Areas Uploaded: {}", stats.total_uploaded);
        info!("Areas Failed: {}", stats.total_failed);
        info!("Total Bytes Uploaded: {}", stats.total_bytes_uploaded);
    }
}
