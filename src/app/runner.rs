use crate::app::monitor::{create_node_status_progress_bar, monitor_node_status};
use crate::config::Config;
use crate::initialization::print_final_stats;
use crate::services::{
    CountryService, ExtractionService, LocalityUploadService, StorageService,
};
use std::sync::Arc;
use tracing::{error, info, warn};

use super::ApplicationResult;

pub struct NodeRunner {
    config: Arc<Config>,
    storage_service: Arc<StorageService>,
    extraction_service: ExtractionService,
    upload_service: LocalityUploadService,
    country_service: CountryService,
    locality_ids: Vec<u32>,
    skip_extract: bool,
}

impl NodeRunner {
    pub fn new(
        config: Arc<Config>,
        storage_service: Arc<StorageService>,
        extraction_service: ExtractionService,
        upload_service: LocalityUploadService,
        country_service: CountryService,
        locality_ids: Vec<u32>,
        skip_extract: bool,
    ) -> Self {
        Self {
            config,
            storage_service,
            extraction_service,
            upload_service,
            country_service,
            locality_ids,
            skip_extract,
        }
    }

    pub async fn run(&self) -> ApplicationResult<()> {
        info!("Starting storage node...");
        self.storage_service.start_node().await?;
        info!("Storage node started successfully");

        if !self.skip_extract {
            info!("Extracting PMTiles from planet file...");
            if !self.locality_ids.is_empty() {
                info!("Processing {} specific locality IDs", self.locality_ids.len());
                if let Err(e) = self
                    .extraction_service
                    .extract_localities_by_ids(&self.locality_ids)
                    .await
                {
                    error!("Failed to extract PMTiles: {}", e);
                    warn!("Continuing with existing PMTiles if available...");
                }
            } else {
                let countries = self
                    .country_service
                    .get_countries_to_process(&self.config.target_countries)
                    .await?;
                info!("Processing {} countries", countries.len());
                if let Err(e) = self.extraction_service.extract_localities(&countries).await {
                    error!("Failed to extract PMTiles: {}", e);
                    warn!("Continuing with existing PMTiles if available...");
                }
            }
        } else {
            info!("Skipping PMTiles extraction (--no-extract flag set)");
        }

        info!("Uploading localities to storage...");
        self.upload_service.process_all_localities().await?;

        let stats = self.upload_service.get_stats().await;
        print_final_stats(&stats);

        self.display_node_info().await;

        Ok(())
    }

    async fn display_node_info(&self) {
        match self.storage_service.get_node_info().await {
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
                if let Some(spr) = node_info.spr {
                    info!("Signed Peer Record:\n  {}", spr);
                }
                info!("Discovery table nodes: {}", node_info.discovery_node_count);
                if node_info.discovery_node_count > 0 {
                    info!("Successfully connected to the network via bootstrap nodes");
                } else {
                    warn!("No peers in discovery table - bootstrap may have failed");
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
    }

    pub fn start_monitoring(&self) -> tokio::task::JoinHandle<()> {
        let progress_bar = create_node_status_progress_bar();
        let storage_service = self.storage_service.clone();

        tokio::spawn(async move {
            monitor_node_status(storage_service, progress_bar).await;
        })
    }

    pub async fn shutdown(&self) -> Result<(), crate::services::StorageError> {
        info!("Stopping storage node...");
        self.storage_service.stop_node().await?;
        info!("Storage node stopped successfully");
        Ok(())
    }
}
