use std::env::VarError;
use std::path::PathBuf;
use std::time::Duration;

pub trait Environment: Send + Sync {
    fn var(&self, key: &str) -> Result<String, VarError>;
}

pub struct RealEnv;

impl Environment for RealEnv {
    fn var(&self, key: &str) -> Result<String, VarError> {
        std::env::var(key)
    }
}

#[derive(Debug)]
pub enum ConfigError {
    MissingEnvVar(String),
    InvalidValue(String),
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigError::MissingEnvVar(var) => {
                write!(f, "Missing required environment variable: {}", var)
            }
            ConfigError::InvalidValue(msg) => write!(f, "Invalid configuration value: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Clone, Debug)]
pub struct Config {
    pub kubo_api_url: String,
    pub kubo_api_timeout: Duration,
    pub pin_on_upload: bool,
    pub kubo_api_username: Option<String>,
    pub kubo_api_password: Option<String>,

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
        Self::from_env_with(&RealEnv)
    }

    pub fn from_env_with<E: Environment>(env: &E) -> Result<Self, ConfigError> {
        let kubo_api_url = env
            .var("KUBO_API_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:5001".to_string());

        let kubo_api_timeout_secs: u64 = env
            .var("KUBO_API_TIMEOUT_SECS")
            .ok()
            .filter(|s| !s.is_empty())
            .and_then(|s| s.parse().ok())
            .unwrap_or(300);
        let kubo_api_timeout = Duration::from_secs(kubo_api_timeout_secs);

        let pin_on_upload = env
            .var("KUBO_PIN_ON_UPLOAD")
            .ok()
            .filter(|s| !s.is_empty())
            .map(|s| s.parse::<bool>().unwrap_or(true))
            .unwrap_or(true);

        let kubo_api_username = env.var("KUBO_API_USERNAME").ok().filter(|s| !s.is_empty());
        let kubo_api_password = env.var("KUBO_API_PASSWORD").ok().filter(|s| !s.is_empty());

        let whosonfirst_db_path = PathBuf::from(
            env.var("WHOSONFIRST_DB_PATH")
                .map_err(|_| ConfigError::MissingEnvVar("WHOSONFIRST_DB_PATH".to_string()))?,
        );

        let cid_db_path = PathBuf::from(
            env.var("CID_DB_PATH")
                .map_err(|_| ConfigError::MissingEnvVar("CID_DB_PATH".to_string()))?,
        );

        let areas_dir = PathBuf::from(
            env.var("AREAS_DIR")
                .map_err(|_| ConfigError::MissingEnvVar("AREAS_DIR".to_string()))?,
        );

        let bzip2_cmd = env
            .var("BZIP2_CMD")
            .map_err(|_| ConfigError::MissingEnvVar("BZIP2_CMD".to_string()))?;

        let pmtiles_cmd = env
            .var("PMTILES_CMD")
            .map_err(|_| ConfigError::MissingEnvVar("PMTILES_CMD".to_string()))?;

        let target_countries: Vec<String> = env
            .var("TARGET_COUNTRIES")
            .map_err(|_| ConfigError::MissingEnvVar("TARGET_COUNTRIES".to_string()))?
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let area_ids: Vec<u32> = env
            .var("AREA_IDS")
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

        let max_concurrent_extractions: usize = env
            .var("MAX_CONCURRENT_EXTRACTIONS")
            .map_err(|_| ConfigError::MissingEnvVar("MAX_CONCURRENT_EXTRACTIONS".to_string()))?
            .parse()
            .map_err(|e| ConfigError::InvalidValue(format!("MAX_CONCURRENT_EXTRACTIONS: {}", e)))?;

        let planet_pmtiles_location = env
            .var("PLANET_PMTILES_LOCATION")
            .ok()
            .filter(|s| !s.is_empty());

        let whosonfirst_db_url = env
            .var("WHOSONFIRST_DB_URL")
            .map_err(|_| ConfigError::MissingEnvVar("WHOSONFIRST_DB_URL".to_string()))?;

        Ok(Self {
            kubo_api_url,
            kubo_api_timeout,
            pin_on_upload,
            kubo_api_username,
            kubo_api_password,
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

    pub fn apply_cli_overrides(&mut self, cli: &crate::cli::Cli) {
        if let Some(url) = cli.kubo_api_url.as_ref() {
            self.kubo_api_url = url.clone();
        }
        if cli.no_pin {
            self.pin_on_upload = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockEnv(HashMap<&'static str, String>);

    impl MockEnv {
        fn new() -> Self {
            Self(HashMap::new())
        }

        fn set(mut self, k: &'static str, v: impl Into<String>) -> Self {
            self.0.insert(k, v.into());
            self
        }
    }

    impl Environment for MockEnv {
        fn var(&self, key: &str) -> Result<String, VarError> {
            self.0.get(key).cloned().ok_or(VarError::NotPresent)
        }
    }

    fn minimal_env() -> MockEnv {
        MockEnv::new()
            .set("WHOSONFIRST_DB_PATH", "/tmp/test.db")
            .set("CID_DB_PATH", "/tmp/cid.db")
            .set("AREAS_DIR", "/tmp/areas")
            .set("BZIP2_CMD", "bzip2")
            .set("PMTILES_CMD", "pmtiles")
            .set("TARGET_COUNTRIES", "")
            .set("MAX_CONCURRENT_EXTRACTIONS", "4")
            .set("WHOSONFIRST_DB_URL", "http://example.com/db")
    }

    #[test]
    fn config_defaults_kubo_url() {
        let env = minimal_env();
        let config = Config::from_env_with(&env).unwrap();
        assert_eq!(config.kubo_api_url, "http://127.0.0.1:5001");
    }

    #[test]
    fn config_defaults_timeout() {
        let env = minimal_env();
        let config = Config::from_env_with(&env).unwrap();
        assert_eq!(config.kubo_api_timeout, Duration::from_secs(300));
    }

    #[test]
    fn config_defaults_pin_on_upload() {
        let env = minimal_env();
        let config = Config::from_env_with(&env).unwrap();
        assert!(config.pin_on_upload);
    }

    #[test]
    fn config_custom_kubo_url() {
        let env = minimal_env().set("KUBO_API_URL", "http://custom:9999");
        let config = Config::from_env_with(&env).unwrap();
        assert_eq!(config.kubo_api_url, "http://custom:9999");
    }

    #[test]
    fn config_custom_timeout() {
        let env = minimal_env().set("KUBO_API_TIMEOUT_SECS", "600");
        let config = Config::from_env_with(&env).unwrap();
        assert_eq!(config.kubo_api_timeout, Duration::from_secs(600));
    }

    #[test]
    fn config_custom_pin_false() {
        let env = minimal_env().set("KUBO_PIN_ON_UPLOAD", "false");
        let config = Config::from_env_with(&env).unwrap();
        assert!(!config.pin_on_upload);
    }

    #[test]
    fn config_target_countries_parsing() {
        let env = minimal_env().set("TARGET_COUNTRIES", "US,CA,GB");
        let config = Config::from_env_with(&env).unwrap();
        assert_eq!(config.target_countries, vec!["US", "CA", "GB"]);
    }

    #[test]
    fn config_target_countries_empty() {
        let env = minimal_env().set("TARGET_COUNTRIES", "");
        let config = Config::from_env_with(&env).unwrap();
        assert!(config.target_countries.is_empty());
    }

    #[test]
    fn config_area_ids_parsing() {
        let env = minimal_env().set("AREA_IDS", "123,456,789");
        let config = Config::from_env_with(&env).unwrap();
        assert_eq!(config.area_ids, vec![123, 456, 789]);
    }

    #[test]
    fn config_area_ids_empty() {
        let env = minimal_env().set("AREA_IDS", "");
        let config = Config::from_env_with(&env).unwrap();
        assert!(config.area_ids.is_empty());
    }

    #[test]
    #[allow(clippy::panic)]
    fn config_missing_required_var() {
        let env = MockEnv::new();
        let result = Config::from_env_with(&env);
        assert!(result.is_err());
        if let Err(ConfigError::MissingEnvVar(var)) = result {
            assert_eq!(var, "WHOSONFIRST_DB_PATH");
        } else {
            panic!("Expected MissingEnvVar error for WHOSONFIRST_DB_PATH");
        }
    }

    #[test]
    fn config_apply_cli_kubo_url() {
        let env = minimal_env().set("KUBO_API_URL", "http://original:5001");
        let mut config = Config::from_env_with(&env).unwrap();
        let cli = crate::cli::Cli {
            non_interactive: false,
            no_download: false,
            no_extract: false,
            config: None,
            kubo_api_url: Some("http://override:8080".to_string()),
            no_pin: false,
            verbose: false,
            quiet: false,
            area_ids: None,
        };

        config.apply_cli_overrides(&cli);
        assert_eq!(config.kubo_api_url, "http://override:8080");
    }

    #[test]
    fn config_apply_cli_no_pin() {
        let env = minimal_env().set("KUBO_PIN_ON_UPLOAD", "true");
        let mut config = Config::from_env_with(&env).unwrap();
        let cli = crate::cli::Cli {
            non_interactive: false,
            no_download: false,
            no_extract: false,
            config: None,
            kubo_api_url: None,
            no_pin: true,
            verbose: false,
            quiet: false,
            area_ids: None,
        };

        config.apply_cli_overrides(&cli);
        assert!(!config.pin_on_upload);
    }
}
