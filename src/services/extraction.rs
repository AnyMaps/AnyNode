use crate::config::Config;
use crate::services::DatabaseService;
use crate::types::Locality;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Semaphore;
use tracing::{error, info};

#[derive(Error, Debug)]
pub enum ExtractionError {
    #[error("Planet PMTiles location not configured")]
    PlanetLocationNotConfigured,
    #[error("Planet PMTiles file not found: {0}")]
    PlanetFileNotFound(String),
    #[error("Extraction failed for locality {0}: {1}")]
    ExtractionFailed(i64, String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

/// Represents a planet PMTiles source - either a local file or remote URL
#[derive(Clone, Debug)]
pub enum PlanetSource {
    Local(PathBuf),
    Remote(String),
}

impl PlanetSource {
    /// Returns true if this is a remote URL
    pub fn is_remote(&self) -> bool {
        matches!(self, PlanetSource::Remote(_))
    }

    /// Returns the source as a string for passing to pmtiles command
    pub fn as_str(&self) -> &str {
        match self {
            PlanetSource::Local(path) => path.to_str().unwrap_or(""),
            PlanetSource::Remote(url) => url,
        }
    }
}

pub struct ExtractionService {
    config: Arc<Config>,
    db_service: Arc<DatabaseService>,
}

impl ExtractionService {
    pub fn new(config: Arc<Config>, db_service: Arc<DatabaseService>) -> Self {
        Self { config, db_service }
    }

    /// Get the planet PMTiles source, which can be either a local file or remote URL
    pub fn get_planet_source(&self) -> Result<PlanetSource, ExtractionError> {
        let location = self
            .config
            .planet_pmtiles_location
            .as_ref()
            .ok_or(ExtractionError::PlanetLocationNotConfigured)?;

        // Check if it's a URL
        if location.starts_with("http://") || location.starts_with("https://") {
            info!("Using remote PMTiles source: {}", location);
            Ok(PlanetSource::Remote(location.clone()))
        } else {
            // It's a local file path
            let path = PathBuf::from(location);
            if !path.exists() {
                return Err(ExtractionError::PlanetFileNotFound(
                    path.to_string_lossy().to_string(),
                ));
            }
            info!("Using local PMTiles file: {}", path.display());
            Ok(PlanetSource::Local(path))
        }
    }

    pub async fn extract_locality(
        &self,
        locality: &Locality,
        planet_source: &PlanetSource,
        country_dir: &Path,
    ) -> Result<(), ExtractionError> {
        let output_path = country_dir.join(format!("{}.pmtiles", locality.id));

        if output_path.exists() {
            info!("Skipping existing file: {}", output_path.display());
            return Ok(());
        }

        let bbox = format!(
            "{},{},{},{}",
            locality.min_longitude,
            locality.min_latitude,
            locality.max_longitude,
            locality.max_latitude
        );

        info!(
            "Extracting locality {} ({}) with bbox: {}",
            locality.id, locality.name, bbox
        );

        let output = tokio::process::Command::new(&self.config.pmtiles_cmd)
            .args([
                "extract",
                planet_source.as_str(),
                output_path.to_str().unwrap(),
                &format!("--bbox={}", bbox),
            ])
            .output()
            .await
            .map_err(|e| ExtractionError::ExtractionFailed(locality.id, e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!("Extraction failed for locality {}: {}", locality.id, stderr);
            return Err(ExtractionError::ExtractionFailed(
                locality.id,
                stderr.to_string(),
            ));
        }

        if output_path.exists() {
            info!("Successfully created file: {}", output_path.display());
            Ok(())
        } else {
            error!("Failed to create file: {}", output_path.display());
            Err(ExtractionError::ExtractionFailed(
                locality.id,
                "Output file not created".to_string(),
            ))
        }
    }

    pub async fn extract_localities(
        &self,
        country_codes: &[String],
    ) -> Result<(), ExtractionError> {
        let planet_source = self.get_planet_source()?;

        for country_code in country_codes {
            info!("Processing country: {}", country_code);

            let country_dir = self.config.localities_dir.join(country_code);
            if !country_dir.exists() {
                std::fs::create_dir_all(&country_dir)?;
            }

            let localities = self
                .db_service
                .get_country_localities(country_code)
                .await
                .map_err(|e| ExtractionError::DatabaseError(e.to_string()))?;

            if localities.is_empty() {
                info!("No localities found for country: {}", country_code);
                continue;
            }

            info!(
                "Found {} localities for country: {}",
                localities.len(),
                country_code
            );

            let mut existing_count = 0;
            for locality in &localities {
                let output_path = country_dir.join(format!("{}.pmtiles", locality.id));
                if output_path.exists() {
                    existing_count += 1;
                }
            }

            let total_count = localities.len();
            let remaining_count = total_count - existing_count;

            if remaining_count == 0 {
                info!(
                    "All {} localities already exist for country: {}",
                    total_count, country_code
                );
                continue;
            }

            info!(
                "Progress: {}/{} localities already exist, {} remaining to extract",
                existing_count, total_count, remaining_count
            );

            let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_extractions));
            let mut tasks = Vec::new();
            let completed_count = Arc::new(std::sync::atomic::AtomicUsize::new(existing_count));

            for locality in localities {
                let planet_source = planet_source.clone();
                let country_dir = country_dir.clone();
                let semaphore = semaphore.clone();
                let extraction_service = self.clone();
                let completed_count = completed_count.clone();

                let task = tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.unwrap();
                    let result = extraction_service
                        .extract_locality(&locality, &planet_source, &country_dir)
                        .await;

                    if result.is_ok() {
                        let current =
                            completed_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        info!(
                            "Progress: {}/{} localities extracted for {}",
                            current + 1,
                            total_count,
                            locality.country
                        );
                    }

                    result
                });

                tasks.push(task);
            }

            let results = futures::future::join_all(tasks).await;

            let mut has_errors = false;
            for result in results {
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        error!("Extraction task failed: {}", e);
                        has_errors = true;
                    }
                    Err(e) => {
                        error!("Extraction task panicked: {:?}", e);
                        has_errors = true;
                    }
                }
            }

            if has_errors {
                return Err(ExtractionError::ExtractionFailed(
                    0,
                    format!("Some extraction tasks failed for country: {}", country_code),
                ));
            }
        }

        Ok(())
    }

    pub async fn get_pmtiles_file_count(&self, country_code: &str) -> Result<u32, ExtractionError> {
        let country_dir = self.config.localities_dir.join(country_code);

        if !country_dir.exists() {
            return Ok(0);
        }

        let mut count = 0;
        let mut entries = tokio::fs::read_dir(&country_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("pmtiles") {
                count += 1;
            }
        }

        Ok(count)
    }

    pub async fn batch_get_pmtiles_file_count(
        &self,
        country_codes: &[String],
    ) -> Result<HashMap<String, u32>, ExtractionError> {
        let mut counts = HashMap::new();

        for country_code in country_codes {
            let count = self.get_pmtiles_file_count(country_code).await?;
            counts.insert(country_code.clone(), count);
        }

        Ok(counts)
    }
}

impl Clone for ExtractionService {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            db_service: self.db_service.clone(),
        }
    }
}
