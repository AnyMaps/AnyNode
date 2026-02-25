use crate::services::{DatabaseService, StorageService};
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
    #[error("Storage error: {0}")]
    StorageError(#[from] crate::services::StorageError),
    #[error("File error: {0}")]
    FileError(#[from] std::io::Error),
    #[error("Upload queue error: {0}")]
    QueueError(String),
}

pub struct AreaUploadService {
    cid_db: Arc<DatabaseService>,
    whosonfirst_db: Arc<DatabaseService>,
    storage: Arc<StorageService>,
    upload_queue: Arc<Mutex<UploadQueue>>,
    stats: Arc<Mutex<UploadStats>>,
    areas_dir: std::path::PathBuf,
}

impl AreaUploadService {
    pub fn new(
        cid_db: Arc<DatabaseService>,
        whosonfirst_db: Arc<DatabaseService>,
        storage: Arc<StorageService>,
        areas_dir: std::path::PathBuf,
    ) -> Self {
        Self {
            cid_db,
            whosonfirst_db,
            storage,
            upload_queue: Arc::new(Mutex::new(UploadQueue::new(10, 100))),
            stats: Arc::new(Mutex::new(UploadStats::new())),
            areas_dir,
        }
    }

    pub async fn process_all_areas(&self) -> Result<(), AreaUploadError> {
        info!("Starting to process all areas by scanning filesystem for PMTiles files");

        if !self.areas_dir.exists() {
            warn!("Areas directory not found: {:?}", self.areas_dir);
            return Ok(());
        }

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
            "Filesystem scan completed! Total files found: {}, Total processed: {}, Total uploaded: {}, Total failed: {}, Total bytes: {}",
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

            match self
                .whosonfirst_db
                .get_area_by_id(area_id as i64)
                .await
            {
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

        let pending_upload = PendingUpload::new(
            country_code.to_string(),
            area_id,
            file_path.to_path_buf(),
        );

        {
            let mut queue = self.upload_queue.lock().await;
            if let Err(e) = queue.add_upload(pending_upload) {
                warn!("Failed to add upload to queue: {}", e);
                return Ok(false);
            }
        }

        if self.upload_queue.lock().await.is_full() {
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

        let result = self.storage.upload_file(file_path).await.map_err(|e| {
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
