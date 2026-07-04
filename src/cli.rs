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
        short,
        long,
        value_name = "FILE",
        help = "Path to configuration file (overrides .env)"
    )]
    pub config: Option<PathBuf>,

    #[arg(long, help = "Override Kubo RPC API URL")]
    pub kubo_api_url: Option<String>,

    #[arg(long, help = "Skip pinning content after upload")]
    pub no_pin: bool,

    #[arg(short, long, help = "Verbose output")]
    pub verbose: bool,

    #[arg(short, long, help = "Quiet mode (minimal output)")]
    pub quiet: bool,

    #[arg(
        long,
        value_name = "IDS",
        help = "Comma-separated area IDs to extract (overrides AREA_IDS and TARGET_COUNTRIES env vars)"
    )]
    pub area_ids: Option<String>,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
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

    pub fn get_area_ids(&self, env_ids: Vec<u32>) -> Vec<u32> {
        if let Some(ids) = &self.area_ids {
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
