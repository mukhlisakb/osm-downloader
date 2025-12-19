use anyhow::{anyhow, Result};
use futures::StreamExt;
use reqwest::Client;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

#[derive(Clone, Debug)]
pub enum DownloadFormat {
    Pbf,
    Shapefile,
}

impl DownloadFormat {
    pub fn suffix(&self) -> &'static str {
        match self {
            DownloadFormat::Pbf => "-latest.osm.pbf",
            DownloadFormat::Shapefile => "-latest-free.shp.zip",
        }
    }
}

#[derive(Debug)]
pub enum DownloadEvent {
    Progress(f64), // Percentage 0.0 to 100.0
    Complete(PathBuf),
    Error(String),
    ImportStarted,
    ImportFinished(String), // Message
    ImportFailed(String), // Error message
}

pub struct Downloader {
    client: Client,
}

impl Downloader {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(Duration::from_secs(300)) // Increased timeout to 5 minutes
                .build()
                .unwrap_or_default(),
        }
    }

    pub fn construct_url(
        &self,
        continent: &str,
        country: &str,
        region: &str,
        format: &DownloadFormat,
    ) -> String {
        let base = "https://download.geofabrik.de";
        let suffix = format.suffix();
        
        // Normalize inputs (basic trim and lowercase)
        let continent = continent.trim().to_lowercase();
        let country = country.trim().to_lowercase();
        let region = region.trim().to_lowercase();

        if region.is_empty() {
            if country.is_empty() {
                format!("{}/{}{}", base, continent, suffix)
            } else {
                format!("{}/{}/{}{}", base, continent, country, suffix)
            }
        } else {
             format!("{}/{}/{}/{}{}", base, continent, country, region, suffix)
        }
    }

    pub async fn download_file(
        &self,
        url: String,
        output_dir: PathBuf,
        tx: tokio::sync::mpsc::Sender<DownloadEvent>,
    ) -> Result<PathBuf> {
        let max_retries = 3;
        let mut retry_count = 0;

        loop {
            match self.attempt_download(&url, &output_dir, &tx).await {
                Ok(path) => return Ok(path),
                Err(e) => {
                    retry_count += 1;
                    if retry_count >= max_retries {
                        let err_msg = format!("Failed after {} retries: {}", max_retries, e);
                        let _ = tx.send(DownloadEvent::Error(err_msg.clone())).await;
                        return Err(anyhow!(err_msg));
                    }
                    let _ = tx.send(DownloadEvent::Error(format!("Retry {}/{}: {}", retry_count, max_retries, e))).await;
                    warn!("Download failed, retrying ({}/{}): {}", retry_count, max_retries, e);
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }

    async fn attempt_download(
        &self,
        url: &str,
        output_dir: &Path,
        tx: &tokio::sync::mpsc::Sender<DownloadEvent>,
    ) -> Result<PathBuf> {
        info!("Starting download from: {}", url);
        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            return Err(anyhow!("HTTP Error: {}", response.status()));
        }

        let total_size = response.content_length().unwrap_or(0);
        
        // Extract filename from URL
        let filename = url.split('/').last().unwrap_or("downloaded_file");
        let file_path = output_dir.join(filename);

        let mut file = File::create(&file_path).await?;
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        while let Some(chunk_result) = stream.next().await {
            let chunk = chunk_result?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;

            if total_size > 0 {
                let percentage = (downloaded as f64 / total_size as f64) * 100.0;
                let _ = tx.send(DownloadEvent::Progress(percentage)).await;
            }
        }

        file.flush().await?;
        if total_size > 0 && downloaded != total_size {
            let msg = format!(
                "Download incomplete: expected {} bytes, got {} bytes",
                total_size, downloaded
            );
            warn!("{}", msg);
            let _ = tx.send(DownloadEvent::Error(msg.clone())).await;
            return Err(anyhow!(msg));
        }
        let _ = tx.send(DownloadEvent::Complete(file_path.clone())).await;
        info!("Download completed: {:?}", file_path);
        
        Ok(file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_construct_url() {
        let downloader = Downloader::new();
        
        // Case 1: Continent/Country/Region
        let url = downloader.construct_url("Asia", "Indonesia", "Kalimantan", &DownloadFormat::Pbf);
        assert_eq!(url, "https://download.geofabrik.de/asia/indonesia/kalimantan-latest.osm.pbf");

        // Case 2: Continent/Country
        let url = downloader.construct_url("Europe", "Germany", "", &DownloadFormat::Shapefile);
        assert_eq!(url, "https://download.geofabrik.de/europe/germany-latest-free.shp.zip");

        // Case 3: Continent only
        let url = downloader.construct_url("Africa", "", "", &DownloadFormat::Pbf);
        assert_eq!(url, "https://download.geofabrik.de/africa-latest.osm.pbf");
        
        // Case 4: Trimming and Lowercase
        let url = downloader.construct_url(" Asia ", " Indonesia ", " Kalimantan ", &DownloadFormat::Pbf);
        assert_eq!(url, "https://download.geofabrik.de/asia/indonesia/kalimantan-latest.osm.pbf");
    }
}
