use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "anynode")]
#[command(author = "Xavier Saliniere <bonjour@xaviers.sh>")]
#[command(version = "0.1.0")]
#[command(about = "Extract PMTiles map data and upload to decentralized storage", long_about = None)]
pub struct Cli {
    #[arg(long, help = "Run in non-interactive mode (no prompts)")]
    pub non_interactive: bool,

    #[arg(long, help = "Skip downloading planet files")]
    pub no_download: bool,

    #[arg(long, help = "Skip extracting PMTiles from planet files")]
    pub no_extract: bool,

    #[arg(
        long,
        help = "Port for the Storage node (overrides STORAGE_DISCOVERY_PORT env var)"
    )]
    pub port: Option<u16>,

    #[arg(
        long,
        value_name = "DIR",
        help = "Data directory for Storage node (overrides STORAGE_DATA_DIR env var)"
    )]
    pub data_dir: Option<PathBuf>,

    #[arg(
        short,
        long,
        value_name = "FILE",
        help = "Path to configuration file (overrides .env)"
    )]
    pub config: Option<PathBuf>,

    #[arg(short, long, help = "Verbose output")]
    pub verbose: bool,

    #[arg(short, long, help = "Quiet mode (minimal output)")]
    pub quiet: bool,

    #[arg(
        long,
        value_name = "SPR_URI",
        help = "Bootstrap node SPR URI (can be repeated for multiple nodes)"
    )]
    pub bootstrap: Vec<String>,

    #[arg(
        long,
        value_name = "METHOD",
        help = "NAT traversal method: any, none, upnp, pmp, or extip:<IP> (overrides STORAGE_NAT env var)"
    )]
    pub nat: Option<String>,

    #[arg(
        long,
        value_name = "ADDRS",
        help = "Listen addresses (comma-separated multi-addresses, overrides STORAGE_LISTEN_ADDRS env var)"
    )]
    pub listen_addrs: Option<String>,

    #[arg(
        long,
        value_name = "IDS",
        help = "Comma-separated locality IDs to extract (overrides LOCALITY_IDS and TARGET_COUNTRIES env vars)"
    )]
    pub locality_ids: Option<String>,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }

    pub fn get_port(&self, env_port: Option<u16>) -> Option<u16> {
        self.port.or(env_port)
    }

    pub fn get_data_dir(&self, env_dir: Option<PathBuf>) -> Option<PathBuf> {
        self.data_dir.clone().or(env_dir)
    }

    pub fn is_non_interactive(&self) -> bool {
        self.non_interactive
    }

    pub fn should_skip_download(&self) -> bool {
        self.no_download
    }

    pub fn should_skip_extract(&self) -> bool {
        self.no_extract
    }

    pub fn get_log_level(&self) -> &str {
        if self.quiet {
            "error"
        } else if self.verbose {
            "debug"
        } else {
            "info"
        }
    }

    pub fn get_bootstrap_nodes(&self, env_nodes: Vec<String>) -> Vec<String> {
        if !self.bootstrap.is_empty() {
            self.bootstrap.clone()
        } else {
            env_nodes
        }
    }

    pub fn get_nat(&self, env_nat: String) -> String {
        self.nat.clone().unwrap_or(env_nat)
    }

    pub fn get_listen_addrs(&self, env_addrs: Vec<String>) -> Vec<String> {
        if let Some(addrs) = &self.listen_addrs {
            addrs
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        } else {
            env_addrs
        }
    }

    pub fn get_locality_ids(&self, env_ids: Vec<u32>) -> Vec<u32> {
        if let Some(ids) = &self.locality_ids {
            ids.split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<u32>().ok())
                .collect()
        } else {
            env_ids
        }
    }
}
