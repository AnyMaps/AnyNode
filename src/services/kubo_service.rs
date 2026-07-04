use std::path::Path;
use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use thiserror::Error;
use tracing::{error, info};

use crate::config::Config;
use crate::services::kubo_client::{KuboClient, KuboClientError};
use crate::services::traits::KuboServiceTrait;

#[derive(Error, Debug)]
pub enum KuboServiceError {
    #[error("Kubo API unavailable: {0}")]
    ApiUnavailable(String),
    #[error("Kubo API error: {0}")]
    ApiError(String),
    #[error("Upload failed: {0}")]
    UploadFailed(String),
    #[error("Pin failed: {0}")]
    PinFailed(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

impl From<KuboClientError> for KuboServiceError {
    fn from(err: KuboClientError) -> Self {
        match err {
            KuboClientError::ApiUnavailable(url, detail) => {
                KuboServiceError::ApiUnavailable(format!("{}: {}", url, detail))
            }
            KuboClientError::ApiError(msg) => KuboServiceError::ApiError(msg),
            KuboClientError::UploadFailed(msg) => KuboServiceError::UploadFailed(msg),
            KuboClientError::PinFailed(cid, msg) => {
                KuboServiceError::PinFailed(format!("CID {}: {}", cid, msg))
            }
            KuboClientError::ParseError(msg) => KuboServiceError::ApiError(msg),
            KuboClientError::IoError(e) => KuboServiceError::IoError(e),
            KuboClientError::RequestError(e) => KuboServiceError::ApiUnavailable(e.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum KuboServiceStatus {
    Disconnected,
    Connected,
    Error(String),
}

#[derive(Debug, Clone)]
pub struct UploadResult {
    pub cid: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub peer_id: String,
    pub version: String,
    pub agent_version: Option<String>,
    pub addresses: Vec<String>,
    pub peers_connected: usize,
}

pub struct KuboService {
    client: KuboClient,
    pin_on_upload: bool,
    status: Arc<RwLock<KuboServiceStatus>>,
}

impl KuboService {
    pub fn new(config: &Config) -> Result<Self, KuboServiceError> {
        let client = KuboClient::new(
            &config.kubo_api_url,
            config.kubo_api_timeout,
            config.kubo_api_username.clone(),
            config.kubo_api_password.clone(),
        )?;

        Ok(Self {
            client,
            pin_on_upload: config.pin_on_upload,
            status: Arc::new(RwLock::new(KuboServiceStatus::Disconnected)),
        })
    }

    pub async fn check_alive(&self) -> Result<(), KuboServiceError> {
        match self.client.version().await {
            Ok(_) => {
                let mut status = self.status.write().unwrap_or_else(|e| e.into_inner());
                *status = KuboServiceStatus::Connected;
                info!("Connected to Kubo API");
                Ok(())
            }
            Err(e) => {
                let mut status = self.status.write().unwrap_or_else(|e| e.into_inner());
                *status = KuboServiceStatus::Error(e.to_string());
                error!("Kubo API unreachable: {}", e);
                Err(e.into())
            }
        }
    }

    pub fn get_status(&self) -> KuboServiceStatus {
        self.status
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    pub fn is_connected(&self) -> bool {
        matches!(self.get_status(), KuboServiceStatus::Connected)
    }

    pub async fn get_node_info(&self) -> Result<NodeInfo, KuboServiceError> {
        let id_info = self.client.id().await?;
        let version_info = self.client.version().await?;
        let swarm_info = self.client.swarm_peers().await?;

        Ok(NodeInfo {
            peer_id: id_info.id,
            version: version_info.version,
            agent_version: Some(id_info.agent_version),
            addresses: id_info.addresses,
            peers_connected: swarm_info.peers.len(),
        })
    }

    pub async fn upload_file(&self, file_path: &Path) -> Result<UploadResult, KuboServiceError> {
        if !file_path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File not found: {}", file_path.display()),
            )
            .into());
        }

        let file_size = tokio::fs::metadata(file_path).await?.len();

        info!(
            "Uploading file: {} ({} bytes)",
            file_path.display(),
            file_size
        );

        let result = self.client.add(file_path, self.pin_on_upload).await?;

        info!("Upload complete. CID: {}", result.hash);

        Ok(UploadResult {
            cid: result.hash,
            size: result.size,
        })
    }
}

#[async_trait]
impl KuboServiceTrait for KuboService {
    async fn check_alive(&self) -> Result<(), KuboServiceError> {
        self.check_alive().await
    }

    fn get_status(&self) -> KuboServiceStatus {
        self.get_status()
    }

    async fn get_node_info(&self) -> Result<NodeInfo, KuboServiceError> {
        self.get_node_info().await
    }

    async fn upload_file(&self, file_path: &Path) -> Result<UploadResult, KuboServiceError> {
        self.upload_file(file_path).await
    }

    fn is_connected(&self) -> bool {
        self.is_connected()
    }
}

impl Clone for KuboService {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            pin_on_upload: self.pin_on_upload,
            status: Arc::clone(&self.status),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn create_test_config(api_url: &str) -> Config {
        Config {
            kubo_api_url: api_url.to_string(),
            kubo_api_timeout: std::time::Duration::from_secs(30),
            pin_on_upload: true,
            kubo_api_username: None,
            kubo_api_password: None,
            whosonfirst_db_path: std::path::PathBuf::from("/tmp/test.db"),
            cid_db_path: std::path::PathBuf::from("/tmp/cid.db"),
            areas_dir: std::path::PathBuf::from("/tmp/areas"),
            bzip2_cmd: "bzip2".to_string(),
            pmtiles_cmd: "pmtiles".to_string(),
            target_countries: vec![],
            area_ids: vec![],
            max_concurrent_extractions: 4,
            planet_pmtiles_location: None,
            whosonfirst_db_url: "http://example.com/db".to_string(),
        }
    }

    #[tokio::test]
    async fn new_creates_service() {
        let server = MockServer::start().await;
        let config = create_test_config(&server.uri());
        let service = KuboService::new(&config).unwrap();
        assert_eq!(service.get_status(), KuboServiceStatus::Disconnected);
    }

    #[tokio::test]
    async fn check_alive_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v0/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Version": "0.20.0"
            })))
            .mount(&server)
            .await;

        let config = create_test_config(&server.uri());
        let service = KuboService::new(&config).unwrap();
        service.check_alive().await.unwrap();

        assert_eq!(service.get_status(), KuboServiceStatus::Connected);
    }

    #[tokio::test]
    async fn check_alive_failure() {
        let config = create_test_config("http://127.0.0.1:59999");
        let service = KuboService::new(&config).unwrap();
        let result = service.check_alive().await;

        assert!(result.is_err());
        assert!(matches!(service.get_status(), KuboServiceStatus::Error(_)));
    }

    #[tokio::test]
    async fn get_status_reflects_state() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v0/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Version": "0.20.0"
            })))
            .mount(&server)
            .await;

        let config = create_test_config(&server.uri());
        let service = KuboService::new(&config).unwrap();
        assert_eq!(service.get_status(), KuboServiceStatus::Disconnected);

        service.check_alive().await.unwrap();
        assert_eq!(service.get_status(), KuboServiceStatus::Connected);
    }

    #[tokio::test]
    async fn get_node_info_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v0/id"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "ID": "QmPeerId123",
                "Addresses": ["/ip4/127.0.0.1/tcp/4001/p2p/QmPeerId123"],
                "AgentVersion": "kubo/0.20.0"
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/v0/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Version": "0.20.0"
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/api/v0/swarm/peers"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Peers": [
                    {"Addr": "/ip4/192.168.1.1/tcp/4001", "Peer": "QmPeer1"},
                    {"Addr": "/ip4/192.168.1.2/tcp/4001", "Peer": "QmPeer2"}
                ]
            })))
            .mount(&server)
            .await;

        let config = create_test_config(&server.uri());
        let service = KuboService::new(&config).unwrap();
        let info = service.get_node_info().await.unwrap();

        assert_eq!(info.peer_id, "QmPeerId123");
        assert_eq!(info.version, "0.20.0");
        assert_eq!(info.agent_version, Some("kubo/0.20.0".to_string()));
        assert_eq!(
            info.addresses,
            vec!["/ip4/127.0.0.1/tcp/4001/p2p/QmPeerId123"]
        );
        assert_eq!(info.peers_connected, 2);
    }

    #[tokio::test]
    async fn upload_file_success() {
        let server = MockServer::start().await;

        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"test data").unwrap();

        Mock::given(method("POST"))
            .and(path("/api/v0/add"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Name": "test.txt",
                "Hash": "QmTestHash123",
                "Size": "12345"
            })))
            .mount(&server)
            .await;

        let config = create_test_config(&server.uri());
        let service = KuboService::new(&config).unwrap();
        let result = service.upload_file(temp.path()).await.unwrap();

        assert_eq!(result.cid, "QmTestHash123");
        assert_eq!(result.size, 12345);
    }

    #[tokio::test]
    async fn upload_file_missing_file() {
        let server = MockServer::start().await;
        let config = create_test_config(&server.uri());
        let service = KuboService::new(&config).unwrap();
        let result = service
            .upload_file(Path::new("/nonexistent/path/file.txt"))
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), KuboServiceError::IoError(_)));
    }

    #[tokio::test]
    async fn is_connected_initially_false() {
        let server = MockServer::start().await;
        let config = create_test_config(&server.uri());
        let service = KuboService::new(&config).unwrap();
        assert!(!service.is_connected());
    }

    #[tokio::test]
    async fn is_connected_after_check() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v0/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Version": "0.20.0"
            })))
            .mount(&server)
            .await;

        let config = create_test_config(&server.uri());
        let service = KuboService::new(&config).unwrap();
        service.check_alive().await.unwrap();
        assert!(service.is_connected());
    }
}
