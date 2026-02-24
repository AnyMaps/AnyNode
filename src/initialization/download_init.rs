use crate::config::Config;
use crate::utils::{download_file_with_progress, run_command};
use std::io::{self, Write};
use std::path::Path;
use tracing::{info, warn};

use super::{InitializationError, InitializationResult};

pub async fn ensure_database_is_present(
    config: &Config,
    cli: &crate::cli::Cli,
) -> InitializationResult<()> {
    let database_path = &config.whosonfirst_db_path;
    let compressed_path = format!("{}.bz2", database_path.display());

    if database_path.exists() {
        info!("WhosOnFirst database already present.");
        return Ok(());
    }

    if Path::new(&compressed_path).exists() {
        info!("Compressed database found, decompressing...");
        decompress_database(&config.bzip2_cmd, &compressed_path).await?;
        return Ok(());
    }

    info!("WhosOnFirst database not found.");

    if !cli.should_skip_download() {
        info!("Auto-downloading WhosOnFirst database...");
        download_and_decompress_database(config, &compressed_path).await?;
        return Ok(());
    }

    if !cli.is_non_interactive() {
        print!("Do you want to download the WhosOnFirst database? This may take a while. (y/n) ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() == "y" {
            download_and_decompress_database(config, &compressed_path).await?;
            return Ok(());
        }
    }

    info!("Database download skipped.");
    Err(InitializationError::DatabaseMissing)
}

async fn download_and_decompress_database(
    config: &Config,
    compressed_path: &str,
) -> InitializationResult<()> {
    if let Some(parent) = Path::new(compressed_path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    info!("Downloading WhosOnFirst database...");
    download_file_with_progress(&config.whosonfirst_db_url, Path::new(compressed_path)).await?;
    info!("Database download completed!");

    info!("Decompressing database...");
    decompress_database(&config.bzip2_cmd, compressed_path).await?;
    info!("Database decompressed successfully!");

    Ok(())
}

async fn decompress_database(bzip2_cmd: &str, compressed_path: &str) -> InitializationResult<()> {
    let output = run_command(bzip2_cmd, &["-dv", compressed_path], None).await?;

    if !output.stderr.is_empty() {
        warn!("Decompression output: {}", output.stderr);
    }

    Ok(())
}
