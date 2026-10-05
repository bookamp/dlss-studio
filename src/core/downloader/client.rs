use std::fs;
use std::io::Read;
use std::path::Path;
use sha2::{Digest, Sha256};
use super::progress::{DownloadProgress, DownloadStage};

pub fn compute_sha256(path: &Path) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

async fn download_file_attempt<F>(
    client: &reqwest::Client,
    url: &str,
    target_path: &Path,
    expected_sha256: &str,
    component_name: &str,
    component_id: &str,
    progress_fn: &mut F,
    attempt: usize,
) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    use std::io::Write;

    if attempt > 1 {
        progress_fn(DownloadProgress {
            is_downloading: true,
            component_name: component_name.to_string(),
            component_id: component_id.to_string(),
            url: url.to_string(),
            downloaded_bytes: 0,
            total_bytes: None,
            percentage: 0.0,
            stage: DownloadStage::Retrying(attempt),
            message: format!("Retrying {} (attempt {}/3)...", component_name, attempt),
        });
    } else {
        progress_fn(DownloadProgress {
            is_downloading: true,
            component_name: component_name.to_string(),
            component_id: component_id.to_string(),
            url: url.to_string(),
            downloaded_bytes: 0,
            total_bytes: None,
            percentage: 0.0,
            stage: DownloadStage::Connecting,
            message: format!("Connecting to {}...", component_name),
        });
    }

    let mut resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to connect to {}: {}", url, e))?;

    if !resp.status().is_success() {
        return Err(format!("Download failed for {} (HTTP status {})", url, resp.status()));
    }

    let total_size = resp.content_length();
    let mut downloaded: u64 = 0;
    let part_path = target_path.with_extension("part");

    // Stream directly to file on disk to prevent RAM bloat
    let mut out_file = fs::File::create(&part_path)
        .map_err(|e| format!("Failed to create part file {}: {}", part_path.display(), e))?;
    let mut hasher = Sha256::new();
    let mut last_emit = std::time::Instant::now();
    let mut last_pct = 0.0f32;

    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| format!("Error streaming from {}: {}", url, e))?
    {
        out_file
            .write_all(&chunk)
            .map_err(|e| format!("Disk write error for {}: {}", part_path.display(), e))?;

        if !expected_sha256.is_empty() {
            hasher.update(&chunk);
        }
        downloaded += chunk.len() as u64;

        let pct = if let Some(total) = total_size {
            if total > 0 {
                (downloaded as f32 / total as f32) * 100.0
            } else {
                0.0
            }
        } else {
            0.0
        };

        if (pct - last_pct).abs() >= 1.0 || last_emit.elapsed() >= std::time::Duration::from_millis(100) {
            last_pct = pct;
            last_emit = std::time::Instant::now();
            progress_fn(DownloadProgress {
                is_downloading: true,
                component_name: component_name.to_string(),
                component_id: component_id.to_string(),
                url: url.to_string(),
                downloaded_bytes: downloaded,
                total_bytes: total_size,
                percentage: pct.min(100.0),
                stage: DownloadStage::Downloading,
                message: format!("Downloading {}...", component_name),
            });
        }
    }

    out_file
        .flush()
        .map_err(|e| format!("Failed to flush {}: {}", part_path.display(), e))?;
    drop(out_file);

    // Final 100% emission
    progress_fn(DownloadProgress {
        is_downloading: true,
        component_name: component_name.to_string(),
        component_id: component_id.to_string(),
        url: url.to_string(),
        downloaded_bytes: downloaded,
        total_bytes: total_size,
        percentage: 100.0,
        stage: DownloadStage::Downloading,
        message: format!("Downloading {}... 100%", component_name),
    });

    if !expected_sha256.is_empty() {
        progress_fn(DownloadProgress {
            is_downloading: true,
            component_name: component_name.to_string(),
            component_id: component_id.to_string(),
            url: url.to_string(),
            downloaded_bytes: downloaded,
            total_bytes: total_size,
            percentage: 100.0,
            stage: DownloadStage::Verifying,
            message: "Verifying checksum...".to_string(),
        });
        let hash = hex::encode(hasher.finalize());
        if !hash.eq_ignore_ascii_case(expected_sha256) {
            let _ = fs::remove_file(&part_path);
            return Err(format!(
                "Checksum mismatch for {}: expected {}, got {}",
                url, expected_sha256, hash
            ));
        }
    }

    fs::rename(&part_path, target_path)
        .map_err(|e| format!("Failed to finalize {}: {}", target_path.display(), e))?;

    progress_fn(DownloadProgress {
        is_downloading: true,
        component_name: component_name.to_string(),
        component_id: component_id.to_string(),
        url: url.to_string(),
        downloaded_bytes: downloaded,
        total_bytes: total_size,
        percentage: 100.0,
        stage: DownloadStage::Downloading,
        message: if component_name.is_empty() { "Complete".to_string() } else { format!("{} ready", component_name) },
    });

    Ok(())
}

pub async fn download_file_with_progress<F>(
    url: &str,
    target_path: &Path,
    expected_sha256: &str,
    component_name: &str,
    component_id: &str,
    mut progress_fn: F,
) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    if target_path.is_file() {
        if let Ok(hash) = compute_sha256(target_path) {
            if expected_sha256.is_empty() || hash.eq_ignore_ascii_case(expected_sha256) {
                return Ok(());
            }
        }
    }

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/128.0.0.0 Safari/537.36 DLSS-Studio/1.0")
        .redirect(reqwest::redirect::Policy::limited(10))
        .connect_timeout(std::time::Duration::from_secs(20))
        .timeout(std::time::Duration::from_secs(300))
        .tcp_keepalive(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("HTTP client error: {}", e))?;

    let mut last_err = String::new();
    for attempt in 1..=3 {
        match download_file_attempt(&client, url, target_path, expected_sha256, component_name, component_id, &mut progress_fn, attempt).await {
            Ok(()) => return Ok(()),
            Err(e) => {
                last_err = e;
                if attempt < 3 {
                    tokio::time::sleep(std::time::Duration::from_millis(1200 * attempt as u64)).await;
                }
            }
        }
    }

    Err(last_err)
}

pub async fn download_file_with_sha256(url: &str, target_path: &Path, expected_sha256: &str) -> Result<(), String> {
    download_file_with_progress(url, target_path, expected_sha256, "", "", |_| {}).await
}
