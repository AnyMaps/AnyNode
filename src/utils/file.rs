use futures::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs::{File, OpenOptions};
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

#[derive(Error, Debug)]
pub enum FileError {
    #[error("Download failed: {0}")]
    DownloadFailed(String),
    #[error("IO error: {0}")]
    IoError(String),
    #[error("Reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("Tokio IO error: {0}")]
    TokioIoError(#[from] tokio::io::Error),
}

const MAX_RETRIES: u32 = 5;
const RETRY_DELAY_SECS: u64 = 5;

/// Download a file with progress reporting, retry logic, and resume support.
/// Downloads to a `.part` temporary file and only renames to final destination when complete.
/// If a `.part` file exists, it will attempt to resume the download using HTTP Range headers.
pub async fn download_file_with_progress(url: &str, destination: &Path) -> Result<(), FileError> {
    let client = reqwest::Client::new();
    let temp_path = get_temp_path(destination);

    for attempt in 1..=MAX_RETRIES {
        match download_attempt(&client, url, &temp_path).await {
            Ok(()) => {
                // Download complete, rename temp file to final destination
                tokio::fs::rename(&temp_path, destination).await?;
                return Ok(());
            }
            Err(e) => {
                if attempt < MAX_RETRIES {
                    warn!(
                        "Download attempt {}/{} failed: {}. Retrying in {} seconds...",
                        attempt, MAX_RETRIES, e, RETRY_DELAY_SECS
                    );
                    tokio::time::sleep(tokio::time::Duration::from_secs(RETRY_DELAY_SECS)).await;
                } else {
                    // Clean up temp file on final failure
                    let _ = tokio::fs::remove_file(&temp_path).await;
                    return Err(e);
                }
            }
        }
    }

    Err(FileError::DownloadFailed(format!(
        "Failed after {} attempts",
        MAX_RETRIES
    )))
}

/// Generate a temporary file path for partial downloads
fn get_temp_path(destination: &Path) -> PathBuf {
    let mut temp_path = destination.to_path_buf();
    let file_name = temp_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("download");
    let temp_name = format!("{}.part", file_name);
    temp_path.set_file_name(temp_name);
    temp_path
}

/// Create a progress bar with standard styling
fn create_progress_bar(total_size: u64) -> ProgressBar {
    let pb = ProgressBar::new(total_size);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
        )
        .unwrap()
        .progress_chars("#>-"),
    );
    pb
}

async fn download_attempt(
    client: &reqwest::Client,
    url: &str,
    temp_path: &Path,
) -> Result<(), FileError> {
    // Check if we have a partial file to resume from
    let existing_size = if temp_path.exists() {
        let metadata = tokio::fs::metadata(temp_path).await?;
        metadata.len()
    } else {
        0
    };

    // Build the request, adding Range header if resuming
    let request = if existing_size > 0 {
        info!(
            "Resuming download from byte {} ({:.2} MB)",
            existing_size,
            existing_size as f64 / 1_048_576.0
        );
        client
            .get(url)
            .header("Range", format!("bytes={}-", existing_size))
            .build()?
    } else {
        client.get(url).build()?
    };

    let response = client.execute(request).await?;

    // Check if server supports range requests when resuming
    let (start_byte, total_size) = if existing_size > 0 {
        if response.status() == reqwest::StatusCode::PARTIAL_CONTENT {
            // Server accepted our range request
            let content_range = response
                .headers()
                .get("content-range")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            // Parse total size from "bytes start-end/total"
            let total = content_range
                .split('/')
                .last()
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(existing_size);

            (existing_size, total)
        } else if response.status().is_success() {
            // Server doesn't support range, start fresh
            info!("Server doesn't support resume, starting download from beginning");
            let total = response.content_length().unwrap_or(0);
            (0, total)
        } else {
            return Err(FileError::DownloadFailed(format!(
                "HTTP error: {}",
                response.status()
            )));
        }
    } else {
        if !response.status().is_success() {
            return Err(FileError::DownloadFailed(format!(
                "HTTP error: {}",
                response.status()
            )));
        }
        let total = response.content_length().unwrap_or(0);
        (0, total)
    };

    // Open file in append mode if resuming, create otherwise
    let mut file = if start_byte > 0 {
        OpenOptions::new()
            .write(true)
            .append(true)
            .open(temp_path)
            .await?
    } else {
        File::create(temp_path).await?
    };

    // Create progress bar
    let pb = create_progress_bar(total_size);
    pb.set_position(start_byte);

    let mut stream = response.bytes_stream();
    let mut downloaded = start_byte;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        pb.set_position(downloaded);
    }

    // Ensure all data is flushed to disk
    file.flush().await?;

    // Finish progress bar
    pb.finish_with_message("Download complete");

    // Verify download completion
    if total_size > 0 && downloaded < total_size {
        return Err(FileError::DownloadFailed(format!(
            "Incomplete download: got {} of {} bytes",
            downloaded, total_size
        )));
    }

    info!("Download completed: {}", temp_path.display());
    Ok(())
}
