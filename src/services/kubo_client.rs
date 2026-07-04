use std::path::Path;
use std::time::Duration;

use reqwest::multipart;
use serde::Deserialize;
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct KuboVersion {
    pub version: String,
}

#[derive(Debug, Clone)]
pub struct KuboId {
    pub id: String,
    pub addresses: Vec<String>,
    pub agent_version: String,
}

#[derive(Debug, Clone)]
pub struct KuboAddResponse {
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Clone)]
pub struct KuboSwarmPeers {
    pub peers: Vec<KuboPeer>,
}

#[derive(Debug, Clone)]
pub struct KuboPeer {
    pub addr: String,
    pub peer: String,
}

#[derive(Debug, thiserror::Error)]
pub enum KuboClientError {
    #[error("Kubo API unavailable at {0}: {1}")]
    ApiUnavailable(String, String),
    #[error("Kubo API error: {0}")]
    ApiError(String),
    #[error("Upload failed: {0}")]
    UploadFailed(String),
    #[error("Pin failed for CID {0}: {1}")]
    PinFailed(String, String),
    #[error("Failed to parse Kubo response: {0}")]
    ParseError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("HTTP request error: {0}")]
    RequestError(#[from] reqwest::Error),
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct KuboVersionRaw {
    Version: String,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct KuboIdRaw {
    ID: String,
    Addresses: Vec<String>,
    AgentVersion: String,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct KuboAddLineRaw {
    Hash: String,
    Size: String,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct KuboSwarmPeersRaw {
    Peers: Vec<KuboPeerRaw>,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct KuboPeerRaw {
    Addr: String,
    Peer: String,
}

#[allow(non_snake_case)]
#[derive(Deserialize)]
struct KuboErrorRaw {
    Message: String,
}

pub struct KuboClient {
    client: reqwest::Client,
    api_url: String,
    username: Option<String>,
    password: Option<String>,
}

impl KuboClient {
    pub fn new(
        api_url: &str,
        timeout: Duration,
        username: Option<String>,
        password: Option<String>,
    ) -> Result<Self, KuboClientError> {
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(KuboClientError::RequestError)?;

        let api_url = api_url.trim_end_matches('/').to_string();

        Ok(Self {
            client,
            api_url,
            username,
            password,
        })
    }

    fn add_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let (Some(user), Some(pass)) = (&self.username, &self.password) {
            request.basic_auth(user, Some(pass))
        } else {
            request
        }
    }

    pub async fn version(&self) -> Result<KuboVersion, KuboClientError> {
        let url = format!("{}/api/v0/version", self.api_url);
        debug!("Requesting Kubo version from {}", url);

        let request = self.add_auth(self.client.post(&url));
        let response = request
            .send()
            .await
            .map_err(|e| KuboClientError::ApiUnavailable(self.api_url.clone(), e.to_string()))?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            let msg = Self::parse_error_message(&body);
            return Err(KuboClientError::ApiError(msg));
        }

        let raw: KuboVersionRaw =
            serde_json::from_str(&body).map_err(|e| KuboClientError::ParseError(e.to_string()))?;

        info!("Kubo version: {}", raw.Version);

        Ok(KuboVersion {
            version: raw.Version,
        })
    }

    pub async fn id(&self) -> Result<KuboId, KuboClientError> {
        let url = format!("{}/api/v0/id", self.api_url);
        debug!("Requesting Kubo node ID from {}", url);

        let request = self.add_auth(self.client.post(&url));
        let response = request
            .send()
            .await
            .map_err(|e| KuboClientError::ApiUnavailable(self.api_url.clone(), e.to_string()))?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            let msg = Self::parse_error_message(&body);
            return Err(KuboClientError::ApiError(msg));
        }

        let raw: KuboIdRaw =
            serde_json::from_str(&body).map_err(|e| KuboClientError::ParseError(e.to_string()))?;

        Ok(KuboId {
            id: raw.ID,
            addresses: raw.Addresses,
            agent_version: raw.AgentVersion,
        })
    }

    pub async fn add(
        &self,
        file_path: &Path,
        pin: bool,
    ) -> Result<KuboAddResponse, KuboClientError> {
        let url = format!("{}/api/v0/add?pin={}&progress=false", self.api_url, pin);
        debug!(
            "Uploading file to Kubo: {} (pin={})",
            file_path.display(),
            pin
        );

        let file_name = file_path
            .file_name()
            .ok_or_else(|| {
                KuboClientError::UploadFailed(format!(
                    "No filename in path: {}",
                    file_path.display()
                ))
            })?
            .to_string_lossy()
            .into_owned();

        let file_bytes = tokio::fs::read(file_path).await?;

        let part = multipart::Part::bytes(file_bytes)
            .file_name(file_name)
            .mime_str("application/octet-stream")
            .map_err(|e| KuboClientError::UploadFailed(e.to_string()))?;

        let form = multipart::Form::new().part("file", part);

        let request = self.add_auth(self.client.post(&url).multipart(form));
        let response = request.send().await?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            let msg = Self::parse_error_message(&body);
            return Err(KuboClientError::UploadFailed(msg));
        }

        let last_line = body
            .lines()
            .rfind(|line| !line.trim().is_empty())
            .ok_or_else(|| {
                KuboClientError::ParseError("Empty response from add endpoint".to_string())
            })?;

        let raw: KuboAddLineRaw = serde_json::from_str(last_line)
            .map_err(|e| KuboClientError::ParseError(e.to_string()))?;

        let size = raw.Size.parse::<u64>().map_err(|e| {
            KuboClientError::ParseError(format!("Failed to parse size '{}': {}", raw.Size, e))
        })?;

        info!("File uploaded to Kubo. CID: {}, size: {}", raw.Hash, size);

        Ok(KuboAddResponse {
            hash: raw.Hash,
            size,
        })
    }

    pub async fn swarm_peers(&self) -> Result<KuboSwarmPeers, KuboClientError> {
        let url = format!("{}/api/v0/swarm/peers", self.api_url);
        debug!("Requesting swarm peers from Kubo");

        let request = self.add_auth(self.client.post(&url));
        let response = request
            .send()
            .await
            .map_err(|e| KuboClientError::ApiUnavailable(self.api_url.clone(), e.to_string()))?;

        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            let msg = Self::parse_error_message(&body);
            return Err(KuboClientError::ApiError(msg));
        }

        let raw: KuboSwarmPeersRaw =
            serde_json::from_str(&body).map_err(|e| KuboClientError::ParseError(e.to_string()))?;

        let peers = raw
            .Peers
            .into_iter()
            .map(|p| KuboPeer {
                addr: p.Addr,
                peer: p.Peer,
            })
            .collect();

        Ok(KuboSwarmPeers { peers })
    }

    fn parse_error_message(body: &str) -> String {
        serde_json::from_str::<KuboErrorRaw>(body)
            .map(|e| e.Message)
            .unwrap_or_else(|_| body.to_string())
    }
}

impl Clone for KuboClient {
    fn clone(&self) -> Self {
        Self {
            client: self.client.clone(),
            api_url: self.api_url.clone(),
            username: self.username.clone(),
            password: self.password.clone(),
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

    #[test]
    fn new_strips_trailing_slash() {
        let client = KuboClient::new(
            "http://localhost:5001/",
            Duration::from_secs(30),
            None,
            None,
        )
        .unwrap();
        assert_eq!(client.api_url, "http://localhost:5001");
    }

    #[tokio::test]
    async fn version_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v0/version"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Version": "0.20.0",
                "Commit": "abc123",
                "Repo": "10"
            })))
            .mount(&server)
            .await;

        let client = KuboClient::new(&server.uri(), Duration::from_secs(30), None, None).unwrap();
        let result = client.version().await.unwrap();

        assert_eq!(result.version, "0.20.0");
    }

    #[tokio::test]
    async fn version_api_unavailable() {
        let client = KuboClient::new(
            "http://127.0.0.1:59999",
            Duration::from_millis(100),
            None,
            None,
        )
        .unwrap();
        let result = client.version().await;

        assert!(matches!(result, Err(KuboClientError::ApiUnavailable(_, _))));
    }

    #[tokio::test]
    async fn version_api_error_500() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v0/version"))
            .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
                "Message": "internal server error"
            })))
            .mount(&server)
            .await;

        let client = KuboClient::new(&server.uri(), Duration::from_secs(30), None, None).unwrap();
        let result = client.version().await;

        assert!(matches!(result, Err(KuboClientError::ApiError(_))));
    }

    #[tokio::test]
    async fn version_parse_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v0/version"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not valid json"))
            .mount(&server)
            .await;

        let client = KuboClient::new(&server.uri(), Duration::from_secs(30), None, None).unwrap();
        let result = client.version().await;

        assert!(matches!(result, Err(KuboClientError::ParseError(_))));
    }

    #[tokio::test]
    async fn id_success() {
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

        let client = KuboClient::new(&server.uri(), Duration::from_secs(30), None, None).unwrap();
        let result = client.id().await.unwrap();

        assert_eq!(result.id, "QmPeerId123");
        assert_eq!(
            result.addresses,
            vec!["/ip4/127.0.0.1/tcp/4001/p2p/QmPeerId123"]
        );
        assert_eq!(result.agent_version, "kubo/0.20.0");
    }

    #[tokio::test]
    async fn id_api_unavailable() {
        let client = KuboClient::new(
            "http://127.0.0.1:59999",
            Duration::from_millis(100),
            None,
            None,
        )
        .unwrap();
        let result = client.id().await;

        assert!(matches!(result, Err(KuboClientError::ApiUnavailable(_, _))));
    }

    #[tokio::test]
    async fn id_api_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v0/id"))
            .respond_with(ResponseTemplate::new(403).set_body_json(serde_json::json!({
                "Message": "access denied"
            })))
            .mount(&server)
            .await;

        let client = KuboClient::new(&server.uri(), Duration::from_secs(30), None, None).unwrap();
        let result = client.id().await;

        assert!(matches!(result, Err(KuboClientError::ApiError(_))));
    }

    #[tokio::test]
    async fn add_success() {
        let server = MockServer::start().await;

        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"test data").unwrap();
        let temp_path = temp.path();

        Mock::given(method("POST"))
            .and(path("/api/v0/add"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Name": "test.txt",
                "Hash": "QmTestHash123",
                "Size": "12345"
            })))
            .mount(&server)
            .await;

        let client = KuboClient::new(&server.uri(), Duration::from_secs(30), None, None).unwrap();
        let result = client.add(temp_path, false).await.unwrap();

        assert_eq!(result.hash, "QmTestHash123");
        assert_eq!(result.size, 12345);
    }

    #[tokio::test]
    async fn add_multiline_response() {
        let server = MockServer::start().await;

        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"test data").unwrap();
        let temp_path = temp.path();

        let multiline_body = r#"{"Name":"test.txt","Hash":"QmFirst","Size":"100"}
{"Name":"test.txt","Hash":"QmSecond","Size":"200"}
{"Name":"test.txt","Hash":"QmLastHash","Size":"300"}
"#;

        Mock::given(method("POST"))
            .and(path("/api/v0/add"))
            .respond_with(ResponseTemplate::new(200).set_body_string(multiline_body))
            .mount(&server)
            .await;

        let client = KuboClient::new(&server.uri(), Duration::from_secs(30), None, None).unwrap();
        let result = client.add(temp_path, false).await.unwrap();

        assert_eq!(result.hash, "QmLastHash");
        assert_eq!(result.size, 300);
    }

    #[tokio::test]
    async fn add_api_error() {
        let server = MockServer::start().await;

        let mut temp = NamedTempFile::new().unwrap();
        temp.write_all(b"test data").unwrap();
        let temp_path = temp.path();

        Mock::given(method("POST"))
            .and(path("/api/v0/add"))
            .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
                "Message": "no space left on device"
            })))
            .mount(&server)
            .await;

        let client = KuboClient::new(&server.uri(), Duration::from_secs(30), None, None).unwrap();
        let result = client.add(temp_path, false).await;

        assert!(matches!(result, Err(KuboClientError::UploadFailed(_))));
    }

    #[tokio::test]
    async fn add_file_not_found() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v0/add"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Hash": "QmTest",
                "Size": "100"
            })))
            .mount(&server)
            .await;

        let client = KuboClient::new(&server.uri(), Duration::from_secs(30), None, None).unwrap();
        let result = client
            .add(Path::new("/nonexistent/path/file.txt"), false)
            .await;

        assert!(matches!(result, Err(KuboClientError::IoError(_))));
    }

    #[tokio::test]
    async fn swarm_peers_success() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v0/swarm/peers"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Peers": [
                    {
                        "Addr": "/ip4/192.168.1.1/tcp/4001",
                        "Peer": "QmPeer1"
                    },
                    {
                        "Addr": "/ip4/192.168.1.2/tcp/4001",
                        "Peer": "QmPeer2"
                    }
                ]
            })))
            .mount(&server)
            .await;

        let client = KuboClient::new(&server.uri(), Duration::from_secs(30), None, None).unwrap();
        let result = client.swarm_peers().await.unwrap();

        assert_eq!(result.peers.len(), 2);
        assert_eq!(result.peers[0].addr, "/ip4/192.168.1.1/tcp/4001");
        assert_eq!(result.peers[0].peer, "QmPeer1");
        assert_eq!(result.peers[1].addr, "/ip4/192.168.1.2/tcp/4001");
        assert_eq!(result.peers[1].peer, "QmPeer2");
    }

    #[tokio::test]
    async fn swarm_peers_empty() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v0/swarm/peers"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "Peers": []
            })))
            .mount(&server)
            .await;

        let client = KuboClient::new(&server.uri(), Duration::from_secs(30), None, None).unwrap();
        let result = client.swarm_peers().await.unwrap();

        assert!(result.peers.is_empty());
    }

    #[tokio::test]
    async fn swarm_peers_api_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/v0/swarm/peers"))
            .respond_with(ResponseTemplate::new(503).set_body_json(serde_json::json!({
                "Message": "service unavailable"
            })))
            .mount(&server)
            .await;

        let client = KuboClient::new(&server.uri(), Duration::from_secs(30), None, None).unwrap();
        let result = client.swarm_peers().await;

        assert!(matches!(result, Err(KuboClientError::ApiError(_))));
    }
}
