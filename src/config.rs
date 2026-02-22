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
    // Storage configuration
    pub storage_data_dir: PathBuf,
    pub storage_quota: u64,
    pub discovery_port: u16,
    pub max_peers: u32,
    pub bootstrap_nodes: Vec<String>,

    // Database paths
    pub whosonfirst_db_path: PathBuf,
    pub cid_db_path: PathBuf,

    // Directories
    pub localities_dir: PathBuf,

    // Tool commands
    pub bzip2_cmd: String,

    // Processing options
    pub target_countries: Vec<String>,
    pub max_concurrent_extractions: usize,
    pub planet_pmtiles_location: Option<String>,

    // URLs
    pub whosonfirst_db_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        // Load .env file if present
        dotenv().ok();

        // Storage configuration (all required)
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

        // Database paths (all required)
        let whosonfirst_db_path = PathBuf::from(
            env::var("WHOSONFIRST_DB_PATH")
                .map_err(|_| ConfigError::MissingEnvVar("WHOSONFIRST_DB_PATH".to_string()))?,
        );

        let cid_db_path = PathBuf::from(
            env::var("CID_DB_PATH")
                .map_err(|_| ConfigError::MissingEnvVar("CID_DB_PATH".to_string()))?,
        );

        // Directories (all required)
        let localities_dir = PathBuf::from(
            env::var("LOCALITIES_DIR")
                .map_err(|_| ConfigError::MissingEnvVar("LOCALITIES_DIR".to_string()))?,
        );

        // Tool commands (required)
        let bzip2_cmd = env::var("BZIP2_CMD")
            .map_err(|_| ConfigError::MissingEnvVar("BZIP2_CMD".to_string()))?;

        // Processing options
        let target_countries: Vec<String> = env::var("TARGET_COUNTRIES")
            .map_err(|_| ConfigError::MissingEnvVar("TARGET_COUNTRIES".to_string()))?
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

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

        // URLs (required)
        let whosonfirst_db_url = env::var("WHOSONFIRST_DB_URL")
            .map_err(|_| ConfigError::MissingEnvVar("WHOSONFIRST_DB_URL".to_string()))?;

        Ok(Self {
            storage_data_dir,
            storage_quota,
            discovery_port,
            max_peers,
            bootstrap_nodes,
            whosonfirst_db_path,
            cid_db_path,
            localities_dir,
            bzip2_cmd,
            target_countries,
            max_concurrent_extractions,
            planet_pmtiles_location,
            whosonfirst_db_url,
        })
    }

    pub fn load() -> Result<Self, ConfigError> {
        Self::from_env()
    }
}
