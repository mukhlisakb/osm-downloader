use anyhow::{Context, Result};
use directories::ProjectDirs;

pub fn init() -> Result<()> {
    let project_dirs = ProjectDirs::from("com", "osm-downloader", "osm-downloader")
        .context("Failed to determine project directories")?;
    let log_dir = project_dirs.data_dir().join("logs");
    std::fs::create_dir_all(&log_dir).context("Failed to create log directory")?;

    let file_appender = tracing_appender::rolling::daily(&log_dir, "osm-downloader.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false)
        .init();

    // Leak the guard so logging continues until the process exits
    // In a real app we might want to manage this better, but for this simple app it's fine
    std::mem::forget(_guard);

    tracing::info!("Logging initialized in {:?}", log_dir);
    Ok(())
}
