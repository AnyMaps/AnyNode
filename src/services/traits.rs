use async_trait::async_trait;
use std::path::Path;

use super::{KuboServiceError, KuboServiceStatus, NodeInfo, UploadResult};

#[async_trait]
pub trait KuboServiceTrait: Send + Sync {
    async fn check_alive(&self) -> Result<(), KuboServiceError>;
    fn get_status(&self) -> KuboServiceStatus;
    async fn get_node_info(&self) -> Result<NodeInfo, KuboServiceError>;
    async fn upload_file(&self, file_path: &Path) -> Result<UploadResult, KuboServiceError>;
    fn is_connected(&self) -> bool;
}
