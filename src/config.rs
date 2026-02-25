use dotenvy::dotenv;
use std::env;
use std::path::PathBuf;

#[derive(Debug)]
pub enum ConfigError {
    MissingEnvVar(String),
    InvalidValue(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::MissingEnvVar(var) => write!(f, "Missing required environment variable: {}", var),
            ConfigError::InvalidValue(msg) => write!(f, "Invalid configuration value: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Clone, Debug)]
pub struct Config {
    pub storage_data_dir: PathBuf,
    pub storage_quota: u64,
    pub discovery_port: u16,
    pub max_peers: u32,
    pub bootstrap_nodes: Vec<String>, // TODO: Add a type for SPR URIs, with proper parsing

    pub nat: String, // TODO: properly type this
    pub listen_addrs: Vec<String>, // TODO: Add a type for those URIs as well, with proper parsing

    pub whosonfirst_db_path: PathBuf,
    pub cid_db_path: PathBuf,

    pub areas_dir: PathBuf,

    pub bzip2_cmd: String,
    pub pmtiles_cmd: String,

    pub target_countries: Vec<String>,
    pub area_ids: Vec<u32>,
    pub max_concurrent_extractions: usize,
    pub planet_pmtiles_location: Option<String>, // TODO: Need validation on this (can either be a path or url)

    pub whosonfirst_db_url: String, // TODO: Need validation on this
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenv().ok();

        let storage_data_dir = PathBuf::from(
            env::var("STORAGE_DATA_DIR")
                .map_err(|_| ConfigError::MissingEnvVar("STORAGE_DATA_DIR".to_string()))?,
        );

        let storage_quota_gb: u64 = env::var("STORAGE_QUOTA_GB")
            .map_err(|_| ConfigError::MissingEnvVar("STORAGE_QUOTA_GB".to_string()))?
            .parse()
            .map_err(|e| ConfigError::InvalidValue(format!("STORAGE_QUOTA_GB: {}", e)))?;
        let storage_quota = storage_quota_gb * 1024 * 1024 * 1024; // Convert GB to bytes

        let discovery_port: u16 = env::var("STORAGE_DISCOVERY_PORT")
            .map_err(|_| ConfigError::MissingEnvVar("STORAGE_DISCOVERY_PORT".to_string()))?
            .parse()
            .map_err(|e| ConfigError::InvalidValue(format!("STORAGE_DISCOVERY_PORT: {}", e)))?;

        let max_peers: u32 = env::var("STORAGE_MAX_PEERS")
            .map_err(|_| ConfigError::MissingEnvVar("STORAGE_MAX_PEERS".to_string()))?
            .parse()
            .map_err(|e| ConfigError::InvalidValue(format!("STORAGE_MAX_PEERS: {}", e)))?;

        let whosonfirst_db_path = PathBuf::from(
            env::var("WHOSONFIRST_DB_PATH")
                .map_err(|_| ConfigError::MissingEnvVar("WHOSONFIRST_DB_PATH".to_string()))?,
        );

        let cid_db_path = PathBuf::from(
            env::var("CID_DB_PATH")
                .map_err(|_| ConfigError::MissingEnvVar("CID_DB_PATH".to_string()))?,
        );

        let areas_dir = PathBuf::from(
            env::var("AREAS_DIR")
                .map_err(|_| ConfigError::MissingEnvVar("AREAS_DIR".to_string()))?,
        );

        let bzip2_cmd = env::var("BZIP2_CMD")
            .map_err(|_| ConfigError::MissingEnvVar("BZIP2_CMD".to_string()))?;

        let pmtiles_cmd = env::var("PMTILES_CMD")
            .map_err(|_| ConfigError::MissingEnvVar("PMTILES_CMD".to_string()))?;

        let target_countries: Vec<String> = env::var("TARGET_COUNTRIES")
            .map_err(|_| ConfigError::MissingEnvVar("TARGET_COUNTRIES".to_string()))?
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        // Optional - comma-separated area IDs to process (overrides TARGET_COUNTRIES)
        let area_ids: Vec<u32> = env::var("AREA_IDS")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse::<u32>().ok())
                    .collect()
            })
            .unwrap_or_default();

        let max_concurrent_extractions: usize = env::var("MAX_CONCURRENT_EXTRACTIONS")
            .map_err(|_| ConfigError::MissingEnvVar("MAX_CONCURRENT_EXTRACTIONS".to_string()))?
            .parse()
            .map_err(|e| ConfigError::InvalidValue(format!("MAX_CONCURRENT_EXTRACTIONS: {}", e)))?;

        // Optional - empty string means None
        // Can be a local file path or a remote URL (http:// or https://)
        let planet_pmtiles_location = env::var("PLANET_PMTILES_LOCATION")
            .ok()
            .filter(|s| !s.is_empty());

        // Optional - comma-separated SPR URIs for bootstrap nodes
        let bootstrap_nodes: Vec<String> = env::var("STORAGE_BOOTSTRAP_NODES")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.split(',').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        let nat = env::var("STORAGE_NAT")
            .map_err(|_| ConfigError::MissingEnvVar("STORAGE_NAT".to_string()))?;

        let listen_addrs: Vec<String> = env::var("STORAGE_LISTEN_ADDRS")
            .map_err(|_| ConfigError::MissingEnvVar("STORAGE_LISTEN_ADDRS".to_string()))?
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let whosonfirst_db_url = env::var("WHOSONFIRST_DB_URL")
            .map_err(|_| ConfigError::MissingEnvVar("WHOSONFIRST_DB_URL".to_string()))?;

        Ok(Self {
            storage_data_dir,
            storage_quota,
            discovery_port,
            max_peers,
            bootstrap_nodes,
            nat,
            listen_addrs,
            whosonfirst_db_path,
            cid_db_path,
            areas_dir,
            bzip2_cmd,
            pmtiles_cmd,
            target_countries,
            area_ids,
            max_concurrent_extractions,
            planet_pmtiles_location,
            whosonfirst_db_url,
        })
    }

    pub fn load() -> Result<Self, ConfigError> {
        Self::from_env()
    }
}
