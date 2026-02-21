use std::sync::Arc;
use storage_bindings::node::config::RepoKind;
use storage_bindings::{debug, upload_file, StorageConfig, StorageNode, LogLevel};
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use tracing::info;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("Node creation failed: {0}")]
    NodeCreation(String),
    #[error("Node start failed: {0}")]
    NodeStart(String),
    #[error("Node stop failed: {0}")]
    NodeStop(String),
    #[error("Node not initialized")]
    NodeNotInitialized,
    #[error("Node not started")]
    NodeNotStarted,
    #[error("Upload failed: {0}")]
    UploadFailed(String),
    #[error("Download failed: {0}")]
    DownloadFailed(String),
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum StorageStatus {
    #[default]
    Disconnected,
    Initialized,
    Connecting,
    Connected,
    Error,
}

#[derive(Debug, Clone)]
pub struct UploadResult {
    pub cid: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct DownloadResult {
    pub cid: String,
    pub size: usize,
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub peer_id: Option<String>,
    pub version: Option<String>,
    pub repo_path: Option<String>,
    pub addresses: Vec<String>,
    pub announce_addresses: Vec<String>,
}

pub struct StorageService {
    node: Arc<Mutex<Option<StorageNode>>>,
    config: StorageConfig,
    status: Arc<RwLock<StorageStatus>>,
}

impl StorageService {
    pub async fn new(
        data_dir: &std::path::Path,
        storage_quota: u64,
        discovery_port: u16,
        max_peers: u32,
    ) -> Result<Self, StorageError> {
        let config = StorageConfig::new()
            .log_level(LogLevel::Info)
            .data_dir(data_dir)
            .storage_quota(storage_quota)
            .max_peers(max_peers)
            .discovery_port(discovery_port)
            .repo_kind(RepoKind::LevelDb);

        let service = Self {
            node: Arc::new(Mutex::new(None)),
            config,
            status: Arc::new(RwLock::new(StorageStatus::Disconnected)),
        };

        service.initialize_node().await?;

        Ok(service)
    }

    pub async fn initialize_node(&self) -> Result<(), StorageError> {
        {
            let mut status = self.status.write().await;
            *status = StorageStatus::Connecting;
        }

        {
            let node_guard = self.node.lock().await;
            if node_guard.is_some() {
                let mut status = self.status.write().await;
                *status = StorageStatus::Initialized;
                return Ok(());
            }
        }

        let node = StorageNode::new(self.config.clone())
            .await
            .map_err(|e| StorageError::NodeCreation(e.to_string()))?;

        {
            let mut node_guard = self.node.lock().await;
            *node_guard = Some(node);
        }

        {
            let mut status = self.status.write().await;
            *status = StorageStatus::Initialized;
        }

        info!("Storage node initialized");
        Ok(())
    }

    pub async fn start_node(&self) -> Result<(), StorageError> {
        {
            let mut status = self.status.write().await;
            *status = StorageStatus::Connecting;
        }

        let node = {
            let mut node_guard = self.node.lock().await;
            match node_guard.take() {
                Some(node) => node,
                None => {
                    drop(node_guard);
                    self.initialize_node().await?;
                    let mut node_guard = self.node.lock().await;
                    node_guard.take().ok_or(StorageError::NodeNotInitialized)?
                }
            }
        };

        node.start()
            .await
            .map_err(|e| StorageError::NodeStart(e.to_string()))?;

        {
            let mut node_guard = self.node.lock().await;
            *node_guard = Some(node);
        }

        {
            let mut status = self.status.write().await;
            *status = StorageStatus::Connected;
        }

        info!("Storage node started");
        Ok(())
    }

    pub async fn stop_node(&self) -> Result<(), StorageError> {
        {
            let mut status = self.status.write().await;
            *status = StorageStatus::Disconnected;
        }

        {
            let node_option = {
                let mut node_guard = self.node.lock().await;
                node_guard.take()
            };

            if let Some(node) = node_option {
                node.stop()
                    .await
                    .map_err(|e| StorageError::NodeStop(e.to_string()))?;

                let mut node_guard = self.node.lock().await;
                *node_guard = Some(node);
            }
        }

        {
            let mut status = self.status.write().await;
            *status = StorageStatus::Initialized;
        }

        info!("Storage node stopped");
        Ok(())
    }

    pub async fn get_status(&self) -> StorageStatus {
        self.status.read().await.clone()
    }

    pub async fn get_node_info(&self) -> Result<NodeInfo, StorageError> {
        let node = {
            let node_guard = self.node.lock().await;
            node_guard
                .as_ref()
                .ok_or(StorageError::NodeNotInitialized)?
                .clone()
        };

        let peer_id = node.peer_id().await.ok();
        let version = node.version().await.ok();
        let repo_path = node.repo().await.ok();

        // Get debug info for addresses
        let debug_info = debug(&node).await.ok();
        let (addresses, announce_addresses) = match debug_info {
            Some(info) => (info.addrs, info.announce_addresses),
            None => (Vec::new(), Vec::new()),
        };

        Ok(NodeInfo {
            peer_id,
            version,
            repo_path,
            addresses,
            announce_addresses,
        })
    }

    pub async fn upload_file(&self, file_path: &std::path::Path) -> Result<UploadResult, StorageError> {
        let node = {
            let node_guard = self.node.lock().await;
            node_guard
                .as_ref()
                .ok_or(StorageError::NodeNotInitialized)?
                .clone()
        };

        if !node.is_started() {
            return Err(StorageError::NodeNotStarted);
        }

        if !file_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File not found: {}", file_path.display()),
            ).into());
        }

        let file_size = tokio::fs::metadata(file_path).await?.len();

        info!(
            "Uploading file: {} ({} bytes)",
            file_path.display(),
            file_size
        );

        // Create upload options
        let file_path_owned = file_path.to_path_buf();
        let upload_options = storage_bindings::UploadOptions::new()
            .filepath(&file_path_owned)
            .on_progress(move |progress| {
                let percentage = (progress.percentage * 100.0) as u32;
                info!("Upload progress: {}%", percentage);
            });

        // Perform upload
        let result = upload_file(&node, upload_options)
            .await
            .map_err(|e| StorageError::UploadFailed(e.to_string()))?;

        info!("Upload complete. CID: {}", result.cid);

        Ok(UploadResult {
            cid: result.cid,
            size: file_size,
        })
    }

    pub async fn is_started(&self) -> bool {
        let node_guard = self.node.lock().await;
        if let Some(node) = node_guard.as_ref() {
            node.is_started()
        } else {
            false
        }
    }
}

impl Clone for StorageService {
    fn clone(&self) -> Self {
        Self {
            node: Arc::clone(&self.node),
            config: self.config.clone(),
            status: Arc::clone(&self.status),
        }
    }
}
