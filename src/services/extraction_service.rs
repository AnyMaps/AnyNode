use crate::config::Config;
use crate::services::DatabaseService;
use crate::types::AdministrativeArea;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Semaphore;
use tracing::{error, info, warn};

#[derive(Error, Debug)]
pub enum ExtractionError {
    #[error("Planet PMTiles location not configured")]
    PlanetLocationNotConfigured,
    #[error("Planet PMTiles file not found: {0}")]
    PlanetFileNotFound(String),
    #[error("Extraction failed for area {0}: {1}")]
    ExtractionFailed(i64, String),
    #[error("Database error: {0}")]
    DatabaseError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Internal error: {0}")]
    InternalError(String),
}

#[derive(Clone, Debug)]
pub enum PlanetSource {
    Local(PathBuf),
    Remote(String),
}

impl PlanetSource {
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

    pub fn get_planet_source(&self) -> Result<PlanetSource, ExtractionError> {
        let location = self
            .config
            .planet_pmtiles_location
            .as_ref()
            .ok_or(ExtractionError::PlanetLocationNotConfigured)?;

        if location.starts_with("http://") || location.starts_with("https://") {
            info!("Using remote PMTiles source: {}", location);
            Ok(PlanetSource::Remote(location.clone()))
        } else {
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

    pub async fn extract_area(
        &self,
        area: &AdministrativeArea,
        planet_source: &PlanetSource,
        country_dir: &Path,
    ) -> Result<(), ExtractionError> {
        let output_path = country_dir.join(format!("{}.pmtiles", area.id));

        if output_path.exists() {
            info!("Skipping existing file: {}", output_path.display());
            return Ok(());
        }

        let bbox = format!(
            "{},{},{},{}",
            area.min_longitude, area.min_latitude, area.max_longitude, area.max_latitude
        );

        info!(
            "Extracting {} {} ({}) with bbox: {}",
            area.placetype, area.id, area.name, bbox
        );

        let output_path_str = output_path.to_str().ok_or_else(|| {
            ExtractionError::ExtractionFailed(
                area.id,
                format!("Invalid UTF-8 in output path: {}", output_path.display()),
            )
        })?;
        let output = tokio::process::Command::new(&self.config.pmtiles_cmd)
            .args([
                "extract",
                planet_source.as_str(),
                output_path_str,
                &format!("--bbox={}", bbox),
            ])
            .output()
            .await
            .map_err(|e| ExtractionError::ExtractionFailed(area.id, e.to_string()))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!(
                "Extraction failed for {} {}: {}",
                area.placetype, area.id, stderr
            );
            return Err(ExtractionError::ExtractionFailed(
                area.id,
                stderr.to_string(),
            ));
        }

        if output_path.exists() {
            info!("Successfully created file: {}", output_path.display());
            Ok(())
        } else {
            error!("Failed to create file: {}", output_path.display());
            Err(ExtractionError::ExtractionFailed(
                area.id,
                "Output file not created".to_string(),
            ))
        }
    }

    pub async fn extract_areas(&self, country_codes: &[String]) -> Result<(), ExtractionError> {
        let planet_source = self.get_planet_source()?;

        for country_code in country_codes {
            info!("Processing country: {}", country_code);

            let country_dir = self.config.areas_dir.join(country_code);
            if !country_dir.exists() {
                std::fs::create_dir_all(&country_dir)?;
            }

            let areas = self
                .db_service
                .get_country_areas(country_code)
                .await
                .map_err(|e| ExtractionError::DatabaseError(e.to_string()))?;

            if areas.is_empty() {
                info!("No areas found for country: {}", country_code);
                continue;
            }

            info!("Found {} areas for country: {}", areas.len(), country_code);

            let mut existing_count = 0;
            for area in &areas {
                let output_path = country_dir.join(format!("{}.pmtiles", area.id));
                if output_path.exists() {
                    existing_count += 1;
                }
            }

            let total_count = areas.len();
            let remaining_count = total_count - existing_count;

            if remaining_count == 0 {
                info!(
                    "All {} areas already exist for country: {}",
                    total_count, country_code
                );
                continue;
            }

            info!(
                "Progress: {}/{} areas already exist, {} remaining to extract",
                existing_count, total_count, remaining_count
            );

            let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_extractions));
            let mut tasks = Vec::new();
            let completed_count = Arc::new(std::sync::atomic::AtomicUsize::new(existing_count));

            for area in areas {
                let planet_source = planet_source.clone();
                let country_dir = country_dir.clone();
                let semaphore = semaphore.clone();
                let extraction_service = self.clone();
                let completed_count = completed_count.clone();

                let task = tokio::spawn(async move {
                    let _permit = semaphore.acquire().await.map_err(|e| {
                        ExtractionError::InternalError(format!("semaphore error: {}", e))
                    })?;
                    let result = extraction_service
                        .extract_area(&area, &planet_source, &country_dir)
                        .await;

                    if result.is_ok() {
                        let current =
                            completed_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                        info!(
                            "Progress: {}/{} areas extracted for {}",
                            current + 1,
                            total_count,
                            area.country
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

    pub async fn extract_areas_by_ids(&self, area_ids: &[u32]) -> Result<(), ExtractionError> {
        let planet_source = self.get_planet_source()?;

        let areas = self
            .db_service
            .get_areas_by_ids(area_ids)
            .await
            .map_err(|e| ExtractionError::DatabaseError(e.to_string()))?;

        if areas.is_empty() {
            info!("No valid areas found for provided IDs");
            return Ok(());
        }

        info!(
            "Found {} areas for {} provided IDs",
            areas.len(),
            area_ids.len()
        );

        let found_ids: std::collections::HashSet<i64> = areas.iter().map(|a| a.id).collect();
        for id in area_ids {
            if !found_ids.contains(&(*id as i64)) {
                warn!(
                    "Area ID {} not found in database or not a valid region/county",
                    id
                );
            }
        }

        let mut by_country: HashMap<String, Vec<AdministrativeArea>> = HashMap::new();
        for area in areas {
            by_country
                .entry(area.country.clone())
                .or_default()
                .push(area);
        }

        let semaphore = Arc::new(Semaphore::new(self.config.max_concurrent_extractions));
        let mut tasks = Vec::new();

        for (country_code, country_areas) in by_country {
            let country_dir = self.config.areas_dir.join(&country_code);
            if !country_dir.exists() {
                std::fs::create_dir_all(&country_dir)?;
            }

            for area in country_areas {
                let planet_source = planet_source.clone();
                let country_dir = country_dir.clone();
                let semaphore = semaphore.clone();
                let extraction_service = self.clone();

                let task = tokio::spawn(async move {
                    let _permit = match semaphore.acquire().await {
                        Ok(p) => p,
                        Err(e) => {
                            return Err(ExtractionError::InternalError(format!(
                                "semaphore error: {}",
                                e
                            )))
                        }
                    };
                    extraction_service
                        .extract_area(&area, &planet_source, &country_dir)
                        .await
                });

                tasks.push(task);
            }
        }

        let results = futures::future::join_all(tasks).await;

        let mut has_errors = false;
        for result in results {
            match result {
                Ok(Ok(_)) => {}
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
                "Some extraction tasks failed".to_string(),
            ));
        }

        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::services::DatabaseService;
    use std::path::PathBuf;
    use std::time::Duration;

    fn make_config(planet_location: Option<String>) -> Arc<Config> {
        Arc::new(Config {
            kubo_api_url: "http://127.0.0.1:5001".to_string(),
            kubo_api_timeout: Duration::from_secs(30),
            pin_on_upload: false,
            kubo_api_username: None,
            kubo_api_password: None,
            whosonfirst_db_path: PathBuf::from("/tmp/whosonfirst.db"),
            cid_db_path: PathBuf::from("/tmp/cid.db"),
            areas_dir: PathBuf::from("/tmp/areas"),
            bzip2_cmd: "bzip2".to_string(),
            pmtiles_cmd: "pmtiles".to_string(),
            target_countries: vec![],
            area_ids: vec![],
            max_concurrent_extractions: 1,
            planet_pmtiles_location: planet_location,
            whosonfirst_db_url: "http://example.com".to_string(),
        })
    }

    fn make_service(planet_location: Option<String>) -> ExtractionService {
        let config = make_config(planet_location);
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        let db_service = Arc::new(DatabaseService::from_connection(conn, false).unwrap());
        ExtractionService::new(config, db_service)
    }

    #[test]
    #[allow(clippy::panic)]
    fn get_planet_source_none_configured() {
        let service = make_service(None);
        let result = service.get_planet_source();
        assert!(result.is_err());
        match result.unwrap_err() {
            ExtractionError::PlanetLocationNotConfigured => {}
            other => panic!("expected PlanetLocationNotConfigured, got {:?}", other),
        }
    }

    #[test]
    #[allow(clippy::panic)]
    fn get_planet_source_remote_url_https() {
        let service = make_service(Some("https://example.com/planet.pmtiles".to_string()));
        let result = service.get_planet_source();
        assert!(result.is_ok());
        match result.unwrap() {
            PlanetSource::Remote(url) => {
                assert_eq!(url, "https://example.com/planet.pmtiles");
            }
            PlanetSource::Local(_) => panic!("expected Remote, got Local"),
        }
    }

    #[test]
    #[allow(clippy::panic)]
    fn get_planet_source_remote_url_http() {
        let service = make_service(Some("http://example.com/planet.pmtiles".to_string()));
        let result = service.get_planet_source();
        assert!(result.is_ok());
        match result.unwrap() {
            PlanetSource::Remote(url) => {
                assert_eq!(url, "http://example.com/planet.pmtiles");
            }
            PlanetSource::Local(_) => panic!("expected Remote, got Local"),
        }
    }

    #[test]
    #[allow(clippy::panic)]
    fn get_planet_source_local_exists() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("planet.pmtiles");
        std::fs::write(&file_path, b"fake").unwrap();

        let service = make_service(Some(file_path.to_str().unwrap().to_string()));
        let result = service.get_planet_source();
        assert!(result.is_ok());
        match result.unwrap() {
            PlanetSource::Local(path) => {
                assert_eq!(path, file_path);
            }
            PlanetSource::Remote(_) => panic!("expected Local, got Remote"),
        }
    }

    #[test]
    #[allow(clippy::panic)]
    fn get_planet_source_local_not_found() {
        let service = make_service(Some("/tmp/nonexistent_planet_xyz.pmtiles".to_string()));
        let result = service.get_planet_source();
        assert!(result.is_err());
        match result.unwrap_err() {
            ExtractionError::PlanetFileNotFound(path) => {
                assert!(path.contains("nonexistent_planet_xyz"));
            }
            other => panic!("expected PlanetFileNotFound, got {:?}", other),
        }
    }

    #[test]
    fn planet_source_as_str_local() {
        let source = PlanetSource::Local(PathBuf::from("/data/planet.pmtiles"));
        assert_eq!(source.as_str(), "/data/planet.pmtiles");
    }

    #[test]
    fn planet_source_as_str_remote() {
        let source = PlanetSource::Remote("https://example.com/planet.pmtiles".to_string());
        assert_eq!(source.as_str(), "https://example.com/planet.pmtiles");
    }
}
