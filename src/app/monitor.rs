use crate::services::{StorageService, StorageStatus};
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;

pub fn create_node_status_progress_bar() -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.green} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

pub async fn monitor_node_status(
    storage_service: Arc<StorageService>,
    progress_bar: ProgressBar,
) {
    let mut tick = interval(Duration::from_secs(2));

    loop {
        tick.tick().await;

        let status = storage_service.get_status().await;

        match storage_service.get_node_info().await {
            Ok(node_info) => {
                let status_str = format_status(&status);
                progress_bar.set_message(format!(
                    "Status: {} | Discovery: {} nodes",
                    status_str, node_info.discovery_node_count
                ));
            }
            Err(_) => {
                let status_str = format_status(&status);
                progress_bar.set_message(format!("Status: {}", status_str));
            }
        }
    }
}

pub fn format_status(status: &StorageStatus) -> &'static str {
    match status {
        StorageStatus::Disconnected => "Disconnected",
        StorageStatus::Initialized => "Initialized",
        StorageStatus::Connecting => "Connecting",
        StorageStatus::Connected => "Connected",
        StorageStatus::Error => "Error",
    }
}
