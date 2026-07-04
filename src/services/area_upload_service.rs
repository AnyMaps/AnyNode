use crate::services::{DatabaseService, KuboServiceTrait};
use crate::types::{CompletedUpload, PendingUpload, UploadQueue, UploadStats};
use futures::future::join_all;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[derive(Error, Debug)]
pub enum AreaUploadError {
    #[error("Database error: {0}")]
    DatabaseError(#[from] crate::services::DatabaseError),
    #[error("Kubo service error: {0}")]
    KuboServiceError(#[from] crate::services::KuboServiceError),
    #[error("File error: {0}")]
    FileError(#[from] std::io::Error),
    #[error("Upload queue error: {0}")]
    QueueError(String),
}

pub struct AreaUploadService {
    cid_db: Arc<DatabaseService>,
    whosonfirst_db: Arc<DatabaseService>,
    kubo: Arc<dyn KuboServiceTrait>,
    upload_queue: Arc<Mutex<UploadQueue>>,
    stats: Arc<Mutex<UploadStats>>,
    areas_dir: std::path::PathBuf,
    target_countries: Vec<String>,
    area_ids: Vec<u32>,
}

impl AreaUploadService {
    pub fn new(
        cid_db: Arc<DatabaseService>,
        whosonfirst_db: Arc<DatabaseService>,
        kubo: Arc<dyn KuboServiceTrait>,
        areas_dir: std::path::PathBuf,
        target_countries: Vec<String>,
        area_ids: Vec<u32>,
    ) -> Self {
        Self {
            cid_db,
            whosonfirst_db,
            kubo,
            upload_queue: Arc::new(Mutex::new(UploadQueue::new(10, 100))),
            stats: Arc::new(Mutex::new(UploadStats::default())),
            areas_dir,
            target_countries,
            area_ids,
        }
    }

    pub async fn process_areas(&self) -> Result<(), AreaUploadError> {
        if !self.areas_dir.exists() {
            warn!("Areas directory not found: {:?}", self.areas_dir);
            return Ok(());
        }

        if !self.area_ids.is_empty() {
            info!("Processing {} specific area IDs", self.area_ids.len());
            self.process_areas_by_ids().await
        } else {
            info!("Processing areas by country from filesystem");
            self.process_areas_by_country().await
        }
    }

    async fn process_areas_by_country(&self) -> Result<(), AreaUploadError> {
        let mut total_files = 0;
        let mut processed_files = 0;

        for country_dir_entry in std::fs::read_dir(&self.areas_dir)? {
            let country_dir = country_dir_entry?;
            let country_path = country_dir.path();

            if !country_path.is_dir() {
                continue;
            }

            let country_code = country_path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| {
                    AreaUploadError::QueueError("Invalid country directory name".to_string())
                })?;

            if !self.target_countries.is_empty()
                && !self.target_countries.contains(&country_code.to_string())
            {
                info!(
                    "Skipping country directory (not in target list): {}",
                    country_code
                );
                continue;
            }

            info!("Scanning country directory: {}", country_code);

            let (country_files, country_processed) = self
                .process_country_directory(&country_path, country_code)
                .await?;
            total_files += country_files;
            processed_files += country_processed;
        }

        if !self.upload_queue.lock().await.is_empty() {
            info!("Processing remaining uploads in queue...");
            self.process_upload_queue().await?;
        }

        let stats = self.stats.lock().await;
        info!(
            "Country scan completed! Total files found: {}, Total processed: {}, Total uploaded: {}, Total failed: {}, Total bytes: {}",
            total_files, processed_files, stats.total_uploaded, stats.total_failed, stats.total_bytes_uploaded
        );

        Ok(())
    }

    async fn process_areas_by_ids(&self) -> Result<(), AreaUploadError> {
        let mut total_files = 0;
        let mut processed_files = 0;

        for area_id in &self.area_ids {
            let found = self.find_and_process_area_file(*area_id).await?;
            if found {
                total_files += 1;
                processed_files += 1;
            } else {
                warn!("Area ID {} not found in filesystem", area_id);
            }
        }

        if !self.upload_queue.lock().await.is_empty() {
            info!("Processing remaining uploads in queue...");
            self.process_upload_queue().await?;
        }

        let stats = self.stats.lock().await;
        info!(
            "Processing done. Total files found: {}, Total processed: {}, Total uploaded: {}, Total failed: {}, Total bytes: {}",
            total_files, processed_files, stats.total_uploaded, stats.total_failed, stats.total_bytes_uploaded
        );

        Ok(())
    }

    async fn process_country_directory(
        &self,
        country_path: &std::path::Path,
        country_code: &str,
    ) -> Result<(usize, usize), AreaUploadError> {
        let mut total_files = 0;
        let mut processed_files = 0;

        for file_entry in std::fs::read_dir(country_path)? {
            let file_entry = file_entry?;
            let file_path = file_entry.path();

            if !file_path.is_file() || file_path.extension().is_none_or(|ext| ext != "pmtiles") {
                continue;
            }

            total_files += 1;

            let filename = file_path
                .file_stem()
                .and_then(|name| name.to_str())
                .ok_or_else(|| AreaUploadError::QueueError("Invalid filename".to_string()))?;

            let area_id = filename.parse::<u32>().map_err(|_| {
                AreaUploadError::QueueError(format!("Invalid area ID in filename: {}", filename))
            })?;

            match self.whosonfirst_db.get_area_by_id(area_id as i64).await {
                Ok(Some(_area)) => {
                    if self
                        .process_file_for_upload(&file_path, country_code, area_id)
                        .await?
                    {
                        processed_files += 1;
                    }
                }
                Ok(None) => {
                    warn!(
                        "Area ID {} found in filesystem but not in database, skipping",
                        area_id
                    );
                }
                Err(e) => {
                    error!("Database error checking area {}: {}", area_id, e);
                }
            }
        }

        info!(
            "Country {}: {} files found, {} processed",
            country_code, total_files, processed_files
        );
        Ok((total_files, processed_files))
    }

    async fn find_and_process_area_file(&self, area_id: u32) -> Result<bool, AreaUploadError> {
        for country_dir_entry in std::fs::read_dir(&self.areas_dir)? {
            let country_dir = country_dir_entry?;
            let country_path = country_dir.path();

            if !country_path.is_dir() {
                continue;
            }

            let file_path = country_path.join(format!("{}.pmtiles", area_id));
            if file_path.exists() {
                let country_code = country_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        AreaUploadError::QueueError("Invalid country directory name".to_string())
                    })?;

                match self.whosonfirst_db.get_area_by_id(area_id as i64).await {
                    Ok(Some(_area)) => {
                        if self
                            .process_file_for_upload(&file_path, country_code, area_id)
                            .await?
                        {
                            return Ok(true);
                        }
                    }
                    Ok(None) => {
                        warn!(
                            "Area ID {} found in filesystem but not in database, skipping",
                            area_id
                        );
                    }
                    Err(e) => {
                        error!("Database error checking area {}: {}", area_id, e);
                    }
                }
            }
        }

        Ok(false)
    }

    async fn process_file_for_upload(
        &self,
        file_path: &std::path::Path,
        country_code: &str,
        area_id: u32,
    ) -> Result<bool, AreaUploadError> {
        if self.cid_db.has_cid_mapping(country_code, area_id).await? {
            info!("Area {} already uploaded, skipping", area_id);
            return Ok(false);
        }

        let pending_upload =
            PendingUpload::new(country_code.to_string(), area_id, file_path.to_path_buf());

        {
            let mut queue = self.upload_queue.lock().await;
            if !queue.add_upload(pending_upload) {
                warn!("Failed to add upload to queue: queue is full");
                return Ok(false);
            }
        }

        if self.upload_queue.lock().await.is_batch_ready() {
            self.process_upload_queue().await?;
        }

        Ok(true)
    }

    async fn process_upload_queue(&self) -> Result<(), AreaUploadError> {
        let batch = {
            let mut queue = self.upload_queue.lock().await;
            queue.take_batch()
        };

        if batch.is_empty() {
            return Ok(());
        }

        info!("Processing batch of {} uploads", batch.len());

        let upload_tasks: Vec<_> = batch
            .into_iter()
            .map(|pending| self.upload_single_file(pending))
            .collect();

        let results = join_all(upload_tasks).await;

        let mut successful_uploads = Vec::new();
        let mut failed_count = 0;

        for result in results {
            match result {
                Ok(upload) => successful_uploads.push(upload),
                Err(e) => {
                    error!("Upload failed: {}", e);
                    failed_count += 1;
                }
            }
        }

        if !successful_uploads.is_empty() {
            self.batch_update_cid_mappings(&successful_uploads).await?;

            let mut stats = self.stats.lock().await;
            for upload in &successful_uploads {
                stats.increment_uploaded(upload.file_size);
            }
        }

        {
            let mut stats = self.stats.lock().await;
            for _ in 0..failed_count {
                stats.increment_failed();
            }
        }

        info!(
            "Batch completed: {} successful, {} failed",
            successful_uploads.len(),
            failed_count
        );

        Ok(())
    }

    async fn upload_single_file(
        &self,
        pending: PendingUpload,
    ) -> Result<CompletedUpload, AreaUploadError> {
        let file_path = &pending.file_path;

        if !file_path.exists() {
            return Err(AreaUploadError::FileError(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File not found: {:?}", file_path),
            )));
        }

        let file_size = tokio::fs::metadata(file_path).await?.len();

        info!(
            "Uploading area {} from country {} ({} bytes)",
            pending.area_id, pending.country_code, file_size
        );

        let result = self.kubo.upload_file(file_path).await.map_err(|e| {
            error!("Upload failed for area {}: {}", pending.area_id, e);
            e
        })?;

        let completed_upload = CompletedUpload::new(
            pending.country_code.clone(),
            pending.area_id,
            result.cid.clone(),
            file_size,
        );

        info!(
            "Successfully uploaded area {} with CID: {}",
            pending.area_id, result.cid
        );

        Ok(completed_upload)
    }

    async fn batch_update_cid_mappings(
        &self,
        uploads: &[CompletedUpload],
    ) -> Result<(), AreaUploadError> {
        let mappings: Vec<_> = uploads
            .iter()
            .map(|upload| {
                (
                    upload.country_code.clone(),
                    upload.area_id,
                    upload.cid.clone(),
                    upload.file_size,
                )
            })
            .collect();

        self.cid_db.batch_insert_cid_mappings(&mappings).await?;

        info!("Updated {} CID mappings in database", mappings.len());
        Ok(())
    }

    pub async fn get_stats(&self) -> UploadStats {
        self.stats.lock().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::services::{
        KuboService, KuboServiceError, KuboServiceStatus, NodeInfo, UploadResult,
    };
    use async_trait::async_trait;
    use mockall::mock;
    use rusqlite::Connection;
    use std::path::PathBuf;
    use std::time::Duration;
    use tempfile::TempDir;

    mock! {
        pub KuboService {}

        #[async_trait]
        impl KuboServiceTrait for KuboService {
            async fn check_alive(&self) -> Result<(), KuboServiceError>;
            fn get_status(&self) -> KuboServiceStatus;
            async fn get_node_info(&self) -> Result<NodeInfo, KuboServiceError>;
            async fn upload_file(&self, file_path: &std::path::Path) -> Result<UploadResult, KuboServiceError>;
            fn is_connected(&self) -> bool;
        }
    }

    fn create_test_config() -> Config {
        Config {
            kubo_api_url: "http://127.0.0.1:9999".to_string(),
            kubo_api_timeout: Duration::from_secs(5),
            pin_on_upload: false,
            kubo_api_username: None,
            kubo_api_password: None,
            whosonfirst_db_path: PathBuf::from(":memory:"),
            cid_db_path: PathBuf::from(":memory:"),
            areas_dir: PathBuf::from("/nonexistent"),
            bzip2_cmd: "bzip2".to_string(),
            pmtiles_cmd: "pmtiles".to_string(),
            target_countries: vec![],
            area_ids: vec![],
            max_concurrent_extractions: 1,
            planet_pmtiles_location: None,
            whosonfirst_db_url: String::new(),
        }
    }

    fn create_test_db() -> DatabaseService {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");
        DatabaseService::from_connection(conn, true).expect("Failed to create DatabaseService")
    }

    fn create_spr_table(conn: &Connection) {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS spr (
                id INTEGER PRIMARY KEY,
                name TEXT,
                country TEXT,
                placetype TEXT,
                latitude REAL,
                longitude REAL,
                min_longitude REAL,
                min_latitude REAL,
                max_longitude REAL,
                max_latitude REAL,
                is_current INTEGER DEFAULT 1,
                is_deprecated INTEGER DEFAULT 0
            )",
            [],
        )
        .expect("Failed to create spr table");
    }

    fn insert_test_area(conn: &Connection, id: i64, name: &str, country: &str, placetype: &str) {
        conn.execute(
            "INSERT INTO spr (id, name, country, placetype, latitude, longitude, min_longitude, min_latitude, max_longitude, max_latitude, is_current, is_deprecated)
             VALUES (?1, ?2, ?3, ?4, 37.5, -119.5, -124.5, 32.5, -114.1, 42.0, 1, 0)",
            rusqlite::params![id, name, country, placetype],
        )
        .expect("Failed to insert test area");
    }

    fn create_test_db_with_areas(area_ids: &[i64]) -> DatabaseService {
        let conn = Connection::open_in_memory().expect("Failed to create in-memory database");
        create_spr_table(&conn);
        for id in area_ids {
            insert_test_area(&conn, *id, &format!("Area {}", id), "US", "region");
        }
        DatabaseService::from_connection(conn, true).expect("Failed to create DatabaseService")
    }

    fn create_test_kubo() -> Arc<KuboService> {
        let config = create_test_config();
        Arc::new(KuboService::new(&config).expect("Failed to create KuboService"))
    }

    fn create_test_service(
        areas_dir: PathBuf,
        target_countries: Vec<String>,
        area_ids: Vec<u32>,
    ) -> AreaUploadService {
        let cid_db = Arc::new(create_test_db());
        let whosonfirst_db = Arc::new(create_test_db());
        let kubo = create_test_kubo();

        AreaUploadService::new(
            cid_db,
            whosonfirst_db,
            kubo,
            areas_dir,
            target_countries,
            area_ids,
        )
    }

    fn create_mock_service(
        areas_dir: PathBuf,
        target_countries: Vec<String>,
        area_ids: Vec<u32>,
        mock_kubo: MockKuboService,
    ) -> AreaUploadService {
        let cid_db = Arc::new(create_test_db());
        let whosonfirst_db = Arc::new(create_test_db());
        let kubo = Arc::new(mock_kubo) as Arc<dyn KuboServiceTrait>;

        AreaUploadService::new(
            cid_db,
            whosonfirst_db,
            kubo,
            areas_dir,
            target_countries,
            area_ids,
        )
    }

    #[test]
    fn new_creates_service() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let service = create_test_service(temp_dir.path().to_path_buf(), vec![], vec![]);
        assert!(service.areas_dir.exists());
    }

    #[tokio::test]
    async fn get_stats_initial() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let service = create_test_service(temp_dir.path().to_path_buf(), vec![], vec![]);
        let stats = service.get_stats().await;
        assert_eq!(stats.total_uploaded, 0);
        assert_eq!(stats.total_failed, 0);
        assert_eq!(stats.total_bytes_uploaded, 0);
    }

    #[tokio::test]
    async fn process_areas_missing_dir() {
        let service = create_test_service(PathBuf::from("/nonexistent/path"), vec![], vec![]);
        let result = service.process_areas().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn process_areas_by_ids_empty() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let service = create_test_service(temp_dir.path().to_path_buf(), vec![], vec![]);
        let result = service.process_areas().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn process_areas_with_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let country_dir = temp_dir.path().join("US");
        std::fs::create_dir_all(&country_dir).expect("Failed to create country dir");

        let pmtiles_path = country_dir.join("12345.pmtiles");
        std::fs::write(&pmtiles_path, b"fake pmtiles data").expect("Failed to write test file");

        let mut mock_kubo = MockKuboService::new();
        mock_kubo.expect_upload_file().returning(|_| {
            Ok(UploadResult {
                cid: "QmTestCID123".to_string(),
                size: 18,
            })
        });

        let cid_db = Arc::new(create_test_db());
        let whosonfirst_db = Arc::new(create_test_db_with_areas(&[12345]));

        let service = AreaUploadService::new(
            cid_db,
            whosonfirst_db,
            Arc::new(mock_kubo),
            temp_dir.path().to_path_buf(),
            vec!["US".to_string()],
            vec![],
        );

        let result = service.process_areas().await;
        assert!(result.is_ok());

        let stats = service.get_stats().await;
        assert_eq!(stats.total_uploaded, 1);
    }

    #[tokio::test]
    async fn upload_single_file() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.pmtiles");
        std::fs::write(&file_path, b"test data").expect("Failed to write test file");

        let mut mock_kubo = MockKuboService::new();
        mock_kubo.expect_upload_file().returning(|_| {
            Ok(UploadResult {
                cid: "QmTestCID456".to_string(),
                size: 9,
            })
        });

        let service = create_mock_service(temp_dir.path().to_path_buf(), vec![], vec![], mock_kubo);

        let pending = PendingUpload::new("US".to_string(), 12345, file_path);

        let result = service.upload_single_file(pending).await;
        assert!(result.is_ok());

        let upload = result.unwrap();
        assert_eq!(upload.cid, "QmTestCID456");
        assert_eq!(upload.area_id, 12345);
    }

    #[tokio::test]
    async fn upload_single_file_not_found() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        let mock_kubo = MockKuboService::new();

        let service = create_mock_service(temp_dir.path().to_path_buf(), vec![], vec![], mock_kubo);

        let pending = PendingUpload::new(
            "US".to_string(),
            99999,
            PathBuf::from("/nonexistent/file.pmtiles"),
        );

        let result = service.upload_single_file(pending).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn upload_single_file_kubo_error() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test.pmtiles");
        std::fs::write(&file_path, b"test data").expect("Failed to write test file");

        let mut mock_kubo = MockKuboService::new();
        mock_kubo.expect_upload_file().returning(|_| {
            Err(KuboServiceError::UploadFailed(
                "Connection refused".to_string(),
            ))
        });

        let service = create_mock_service(temp_dir.path().to_path_buf(), vec![], vec![], mock_kubo);

        let pending = PendingUpload::new("US".to_string(), 12345, file_path);

        let result = service.upload_single_file(pending).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn batch_update_cid_mappings() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let country_dir = temp_dir.path().join("US");
        std::fs::create_dir_all(&country_dir).expect("Failed to create country dir");

        let file1 = country_dir.join("100.pmtiles");
        let file2 = country_dir.join("200.pmtiles");
        std::fs::write(&file1, b"data1").expect("Failed to write test file");
        std::fs::write(&file2, b"data2").expect("Failed to write test file");

        let mut mock_kubo = MockKuboService::new();
        mock_kubo
            .expect_upload_file()
            .returning(|_| {
                Ok(UploadResult {
                    cid: "QmCID1".to_string(),
                    size: 5,
                })
            })
            .times(2);

        let cid_db = Arc::new(create_test_db());
        let whosonfirst_db = Arc::new(create_test_db_with_areas(&[100, 200]));

        let service = AreaUploadService::new(
            cid_db.clone(),
            whosonfirst_db,
            Arc::new(mock_kubo),
            temp_dir.path().to_path_buf(),
            vec!["US".to_string()],
            vec![],
        );

        let result = service.process_areas().await;
        assert!(result.is_ok());

        let stats = service.get_stats().await;
        assert_eq!(stats.total_uploaded, 2);
        assert!(stats.total_bytes_uploaded > 0);

        assert!(cid_db.has_cid_mapping("US", 100).await.unwrap());
        assert!(cid_db.has_cid_mapping("US", 200).await.unwrap());
    }
}
