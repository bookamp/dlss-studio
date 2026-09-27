use std::fs;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use sha2::{Digest, Sha256};

pub const FEEDER_ARCHIVE_URL: &str = "https://github.com/jlrouzies-fr/DLSS5-Feeder/releases/download/v1.16.0-beta.3/DLSS5-Feeder-1.16.0-beta.3.zip";
pub const FEEDER_ARCHIVE_SHA256: &str = "0d1deebf531436a6d0914548e450a790aefa53cb4e9f6dfdcd48aff74831cb21";

pub const VORT_ARCHIVE_URL: &str = "https://codeload.github.com/vortigern11/vort_Shaders/zip/b410b9f0c0fbb83c8cb42164aaf1655fab386f4a";
pub const VORT_ARCHIVE_SHA256: &str = "231ba34a75556f9943e359559a89b0d0cc2caa322d9dcdee5630061bf9fe13b6";

pub const RESHADE_SHADERS_SLIM_URL: &str = "https://codeload.github.com/crosire/reshade-shaders/zip/6db142b4b1a05c764222e5b0bd9a644b7ccfe1dc";
pub const RESHADE_SHADERS_SLIM_SHA256: &str = "12d082c8ab1dbcb5e221e1b6116a0343f3182ee517f09bb966b117acc7635312";

pub const RESHADE_FXH_URL: &str = "https://raw.githubusercontent.com/crosire/reshade-shaders/slim/Shaders/ReShade.fxh";
pub const RESHADE_FXH_SHA256: &str = "6dabfbbaf968c3871905d2ea17f96572ff7b1cec01310b5d0e5252b66b30174f";

pub const RESHADE_UI_FXH_URL: &str = "https://raw.githubusercontent.com/crosire/reshade-shaders/slim/Shaders/ReShadeUI.fxh";
pub const RESHADE_UI_FXH_SHA256: &str = "78adf672df47460297eb9fe6dd238d2aafa24510b52b84feb1a745dff70eb901";

pub const DRAWTEXT_FXH_URL: &str = "https://raw.githubusercontent.com/crosire/reshade-shaders/slim/Shaders/DrawText.fxh";
pub const DRAWTEXT_FXH_SHA256: &str = "b79cc4dfb3e98bcf4c06193d00ea7631d74f467f73a4deeeee13e71336d3e680";

pub const FONTATLAS_PNG_URL: &str = "https://raw.githubusercontent.com/crosire/reshade-shaders/slim/Textures/FontAtlas.png";
pub const FONTATLAS_PNG_SHA256: &str = "11a711a8167d1c1606892e6fa6f661a477e749d6cbdb1ff700ac381842066ec3";

pub const MFG_10_URL: &str = "https://github.com/mavismmg/MFGAdaUnlock-RenoDx/releases/download/1.0/renodx-mfgunlock.addon64";
pub const MFG_10_SHA256: &str = "f9f10c685e3e89077f751df2394a1629615a56b58d111dff26b39894e772d50e";

pub const OPTISCALER_084_URL: &str = "https://github.com/wilsjo2/OptiScaler-DLSSNR-PreSR-Multipass/releases/download/v0.8.4/OptiScaler-NR-v0.8.4.zip";
pub const OPTISCALER_084_SHA256: &str = "8789912859882e66b3f3a1aa768db947da779dfd65225df69ea919052e73a2e4";

pub const DLSS_MIP_FIX_URL: &str = "https://github.com/bookamp/dlss-studio/releases/latest/download/dlss-mip-fix.addon64";

pub const RENODX_DLSS5_URL: &str = "https://github.com/yumlevi/renodx-dlss-installer/releases/download/latest/renodx-dlss5.addon64";
pub const STREAMLINE_ZIP_URL: &str = "https://github.com/yumlevi/renodx-dlss-installer/releases/download/latest/streamline.zip";
pub const RESHADE_SETUP_URL: &str = "https://reshade.me/downloads/ReShade_Setup_6.8.0_Addon.exe";
pub const RESHADE_SETUP_SHA256: &str = "afe4c8f13048306307983b8b3d41d5bf00a86820440b0e57dea10950e1176445";

pub const DGVOODOO_URL: &str = "https://github.com/dege-diosg/dgVoodoo2/releases/download/v2.87.5/dgVoodoo2_87_5.zip";
pub const DGVOODOO_SHA256: &str = "5ffde6927f7355ca3fdd5d785b581256a8e6539fa13e395a891ade6ba1040850";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DgVoodooComponents {
    pub d3d9_x86: PathBuf,
    pub d3d9_x64: PathBuf,
    pub d3d8_x86: Option<PathBuf>,
    pub conf: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeederComponents {
    pub addon64: PathBuf,
    pub addon32: Option<PathBuf>,
    pub host64: Option<PathBuf>,
    pub shader_dir: PathBuf,
    pub vk_layer_dir: Option<PathBuf>,
}

pub fn get_components_root() -> PathBuf {
    let p = crate::core::state::get_appdata_dir().join("components");
    let _ = fs::create_dir_all(&p);
    p
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadStage {
    Checking,
    Connecting,
    Downloading,
    Verifying,
    Retrying(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub struct DownloadProgress {
    pub is_downloading: bool,
    pub component_name: String,
    pub component_id: String,
    pub url: String,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub percentage: f32,
    pub stage: DownloadStage,
    pub message: String,
}

impl DownloadProgress {
    pub fn checking() -> Self {
        Self {
            is_downloading: true,
            component_name: String::new(),
            component_id: String::new(),
            url: String::new(),
            downloaded_bytes: 0,
            total_bytes: None,
            percentage: 0.0,
            stage: DownloadStage::Checking,
            message: "Checking components...".to_string(),
        }
    }
}

impl Default for DownloadProgress {
    fn default() -> Self {
        Self {
            is_downloading: false,
            component_name: String::new(),
            component_id: String::new(),
            url: String::new(),
            downloaded_bytes: 0,
            total_bytes: None,
            percentage: 0.0,
            stage: DownloadStage::Downloading,
            message: String::new(),
        }
    }
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

pub fn format_download_progress(lang: &str, prog: &DownloadProgress) -> String {
    match prog.stage {
        DownloadStage::Checking => crate::core::i18n::t(lang, "download_checking").to_string(),
        DownloadStage::Connecting => crate::core::i18n::t_param(lang, "download_connecting", &prog.component_name),
        DownloadStage::Downloading => {
            let base = crate::core::i18n::t_param(lang, "download_downloading", &prog.component_name);
            let bytes_str = match prog.total_bytes {
                Some(total) => format!(" ({} / {})", format_bytes(prog.downloaded_bytes), format_bytes(total)),
                None => {
                    if prog.downloaded_bytes > 0 {
                        format!(" ({})", format_bytes(prog.downloaded_bytes))
                    } else {
                        String::new()
                    }
                }
            };
            format!("{}{}", base, bytes_str)
        }
        DownloadStage::Verifying => crate::core::i18n::t(lang, "download_verifying").to_string(),
        DownloadStage::Retrying(attempt) => {
            crate::core::i18n::t_params(lang, "download_retrying", &[&prog.component_name, &attempt.to_string()])
        }
    }
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

pub fn extract_zip<R: Read + Seek>(reader: R, out_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|e| format!("Failed to create {}: {}", out_dir.display(), e))?;
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| format!("Invalid zip archive: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| format!("Corrupt zip entry: {}", e))?;
        let outpath = match file.enclosed_name() {
            Some(path) => out_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath).map_err(|e| format!("Failed to create {}: {}", outpath.display(), e))?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p).map_err(|e| format!("Failed to create {}: {}", p.display(), e))?;
                }
            }
            let mut outfile = fs::File::create(&outpath).map_err(|e| format!("Failed to create {}: {}", outpath.display(), e))?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| format!("Failed to extract to {}: {}", outpath.display(), e))?;
        }
    }
    Ok(())
}

/// Checks whether dgVoodoo2 is cached on disk
pub fn is_dgvoodoo_cached() -> bool {
    find_local_dgvoodoo_components().is_some()
}

/// Discovers local dgVoodoo2 components on disk
pub fn find_local_dgvoodoo_components() -> Option<DgVoodooComponents> {
    let mut search_dirs = Vec::new();
    search_dirs.push(get_components_root());
    if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
        search_dirs.push(PathBuf::from(&local_appdata).join("dlss-5-studio").join("components"));
        search_dirs.push(PathBuf::from(&local_appdata).join("DLSS-Studio").join("components"));
    }
    if let Ok(appdata) = std::env::var("APPDATA") {
        search_dirs.push(PathBuf::from(&appdata).join("dlss-5-studio").join("components"));
        search_dirs.push(PathBuf::from(&appdata).join("DLSS-Studio").join("components"));
    }
    if let Ok(pd) = std::env::var("ProgramData") {
        search_dirs.push(PathBuf::from(&pd).join("dlss-5-studio").join("components"));
    }
    search_dirs.push(PathBuf::from(r"C:\DLSS 5 Studio\data\components"));
    search_dirs.push(PathBuf::from(r"C:\DLSS 5 Studio\components"));
    if let Ok(exe_p) = std::env::current_exe() {
        if let Some(exe_dir) = exe_p.parent() {
            search_dirs.push(exe_dir.join("components"));
            search_dirs.push(exe_dir.join("components").join("dgvoodoo"));
            search_dirs.push(exe_dir.join("data").join("components"));
            if let Some(parent) = exe_dir.parent() {
                search_dirs.push(parent.join("components"));
                search_dirs.push(parent.join("data").join("components"));
            }
        }
    }

    for dir in search_dirs {
        let dg = if dir.join("dgvoodoo").is_dir() {
            dir.join("dgvoodoo")
        } else {
            dir
        };

        let d3d9_x86 = if dg.join("x86").join("D3D9.dll").is_file() {
            dg.join("x86").join("D3D9.dll")
        } else if dg.join("D3D9.dll").is_file() {
            dg.join("D3D9.dll")
        } else {
            continue;
        };

        let d3d9_x64 = if dg.join("x64").join("D3D9.dll").is_file() {
            dg.join("x64").join("D3D9.dll")
        } else if dg.join("D3D9_x64.dll").is_file() {
            dg.join("D3D9_x64.dll")
        } else {
            d3d9_x86.clone()
        };

        let d3d8_x86 = if dg.join("x86").join("D3D8.dll").is_file() {
            Some(dg.join("x86").join("D3D8.dll"))
        } else if dg.join("D3D8.dll").is_file() {
            Some(dg.join("D3D8.dll"))
        } else {
            None
        };

        let conf = if dg.join("dgVoodoo.conf").is_file() {
            dg.join("dgVoodoo.conf")
        } else {
            continue;
        };

        return Some(DgVoodooComponents {
            d3d9_x86,
            d3d9_x64,
            d3d8_x86,
            conf,
        });
    }
    None
}

/// Helper to construct a tar.exe command configured with CREATE_NO_WINDOW on Windows
/// to prevent console window flashing during background component extractions.
pub fn silent_tar_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("tar.exe");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Asynchronously resolves, downloads (if missing), verifies, and extracts dgVoodoo2 components.
pub async fn ensure_dgvoodoo_components(log: &mut Vec<String>) -> Result<DgVoodooComponents, String> {
    if let Some(local) = find_local_dgvoodoo_components() {
        return Ok(local);
    }

    let comp_root = get_components_root();
    let dgvoodoo_dir = comp_root.join("dgvoodoo");
    let zip_path = comp_root.join("dgVoodoo2_87_5.zip");

    let _ = fs::create_dir_all(dgvoodoo_dir.join("x86"));
    let _ = fs::create_dir_all(dgvoodoo_dir.join("x64"));

    log.push("[DOWNLOAD] Fetching dgVoodoo2 v2.87.5 (D3D9/D3D8 -> D3D11 wrapper)...".to_string());
    download_file_with_sha256(DGVOODOO_URL, &zip_path, DGVOODOO_SHA256).await?;

    let _ = silent_tar_command()
        .arg("-xf")
        .arg(&zip_path)
        .arg("-C")
        .arg(&dgvoodoo_dir)
        .arg("dgVoodoo.conf")
        .status();

    let _ = silent_tar_command()
        .arg("-xf")
        .arg(&zip_path)
        .arg("-C")
        .arg(dgvoodoo_dir.join("x86"))
        .arg("--strip-components")
        .arg("2")
        .arg("MS/x86/D3D9.dll")
        .status();

    let _ = silent_tar_command()
        .arg("-xf")
        .arg(&zip_path)
        .arg("-C")
        .arg(dgvoodoo_dir.join("x86"))
        .arg("--strip-components")
        .arg("2")
        .arg("MS/x86/D3D8.dll")
        .status();

    let _ = silent_tar_command()
        .arg("-xf")
        .arg(&zip_path)
        .arg("-C")
        .arg(dgvoodoo_dir.join("x64"))
        .arg("--strip-components")
        .arg("2")
        .arg("MS/x64/D3D9.dll")
        .status();

    let _ = fs::remove_file(&zip_path);

    find_local_dgvoodoo_components()
        .ok_or_else(|| "Failed to extract and assemble dgVoodoo2 components".to_string())
}

/// Checks local search paths before attempting an online download
pub fn find_local_feeder_components() -> Option<FeederComponents> {
    let mut search_dirs = Vec::new();
    search_dirs.push(get_components_root());

    if let Ok(exe_p) = std::env::current_exe() {
        if let Some(exe_dir) = exe_p.parent() {
            search_dirs.push(exe_dir.join("components"));
            search_dirs.push(exe_dir.join("components").join("feeder"));
        }
    }

    for dir in search_dirs {
        let addon64 = if dir.join("dlss5-feed.addon64").is_file() {
            dir.join("dlss5-feed.addon64")
        } else if dir.join("DLSS5-Feeder-1.16.0-beta.3").join("dlss5-feed.addon64").is_file() {
            dir.join("DLSS5-Feeder-1.16.0-beta.3").join("dlss5-feed.addon64")
        } else if dir.join("DLSS5-Feeder-1.16.0-beta.1").join("dlss5-feed.addon64").is_file() {
            dir.join("DLSS5-Feeder-1.16.0-beta.1").join("dlss5-feed.addon64")
        } else {
            continue;
        };

        let shader_dir = if dir.join("reshade-shaders").join("Shaders").join("DLSS5_Feed.fx").is_file()
            && dir.join("reshade-shaders").join("Shaders").join("DrawText.fxh").is_file()
        {
            dir.join("reshade-shaders")
        } else if dir.join("feeder-shaders").join("Shaders").join("DLSS5_Feed.fx").is_file()
            && dir.join("feeder-shaders").join("Shaders").join("DrawText.fxh").is_file()
        {
            dir.join("feeder-shaders")
        } else {
            continue;
        };

        let vk_layer_dir = if dir.join("layer-x64").join("VkLayer_feed_vk.dll").is_file() {
            Some(dir.join("layer-x64"))
        } else if dir.join("DLSS5-Feeder-1.16.0-beta.3").join("layer-x64").join("VkLayer_feed_vk.dll").is_file() {
            Some(dir.join("DLSS5-Feeder-1.16.0-beta.3").join("layer-x64"))
        } else if dir.join("DLSS5-Feeder-1.16.0-beta.1").join("layer-x64").join("VkLayer_feed_vk.dll").is_file() {
            Some(dir.join("DLSS5-Feeder-1.16.0-beta.1").join("layer-x64"))
        } else {
            None
        };

        let addon32 = if dir.join("dlss5-feed.addon32").is_file() {
            Some(dir.join("dlss5-feed.addon32"))
        } else if dir.join("DLSS5-Feeder-1.16.0-beta.3").join("dlss5-feed.addon32").is_file() {
            Some(dir.join("DLSS5-Feeder-1.16.0-beta.3").join("dlss5-feed.addon32"))
        } else if dir.join("DLSS5-Feeder-1.16.0-beta.1").join("dlss5-feed.addon32").is_file() {
            Some(dir.join("DLSS5-Feeder-1.16.0-beta.1").join("dlss5-feed.addon32"))
        } else {
            None
        };

        let host64 = if dir.join("dlss5-feed-host64.exe").is_file() {
            Some(dir.join("dlss5-feed-host64.exe"))
        } else if dir.join("DLSS5-Feeder-1.16.0-beta.3").join("dlss5-feed-host64.exe").is_file() {
            Some(dir.join("DLSS5-Feeder-1.16.0-beta.3").join("dlss5-feed-host64.exe"))
        } else if dir.join("DLSS5-Feeder-1.16.0-beta.3").join("host64").join("dlss5-feed-host64.exe").is_file() {
            Some(dir.join("DLSS5-Feeder-1.16.0-beta.3").join("host64").join("dlss5-feed-host64.exe"))
        } else if dir.join("DLSS5-Feeder-1.16.0-beta.1").join("dlss5-feed-host64.exe").is_file() {
            Some(dir.join("DLSS5-Feeder-1.16.0-beta.1").join("dlss5-feed-host64.exe"))
        } else if dir.join("DLSS5-Feeder-1.16.0-beta.1").join("host64").join("dlss5-feed-host64.exe").is_file() {
            Some(dir.join("DLSS5-Feeder-1.16.0-beta.1").join("host64").join("dlss5-feed-host64.exe"))
        } else {
            None
        };

        return Some(FeederComponents {
            addon64,
            addon32,
            host64,
            shader_dir,
            vk_layer_dir,
        });
    }
    None
}

/// Asynchronously resolves, downloads (if missing), verifies, and assembles Feeder components.
pub async fn ensure_feeder_components(log: &mut Vec<String>) -> Result<FeederComponents, String> {
    if let Some(local) = find_local_feeder_components() {
        log.push(format!("[FEEDER-CACHE] Using verified local feeder components from {}", local.addon64.parent().unwrap_or(&local.addon64).display()));
        return Ok(local);
    }

    let comp_root = get_components_root();
    let feeder_dir = comp_root.join("DLSS5-Feeder-1.16.0-beta.3");
    let feeder_zip = comp_root.join("DLSS5-Feeder-1.16.0-beta.3.zip");

    // 1. Download & extract DLSS5-Feeder v1.16.0-beta.3
    if !feeder_dir.join("dlss5-feed.addon64").is_file() {
        log.push("[DOWNLOAD] Fetching latest DLSS5-Feeder v1.16.0-beta.3 from upstream GitHub...".to_string());
        download_file_with_sha256(FEEDER_ARCHIVE_URL, &feeder_zip, FEEDER_ARCHIVE_SHA256).await?;
        log.push("[DOWNLOAD] Verifying DLSS5-Feeder SHA-256 checksum: OK".to_string());

        let file = fs::File::open(&feeder_zip).map_err(|e| format!("Failed to open {}: {}", feeder_zip.display(), e))?;
        extract_zip(file, &feeder_dir)?;
        log.push("[FEEDER] DLSS5-Feeder unpacked successfully".to_string());
    }

    // 2. Download & extract vort_Shaders
    let vort_dir = comp_root.join("vort_Shaders-b410b9f");
    let vort_zip = comp_root.join("vort_Shaders-b410b9f.zip");
    if !vort_dir.join("Shaders").join("vort_Motion.fx").is_file() {
        log.push("[DOWNLOAD] Fetching latest VORT Motion Vector shaders from upstream...".to_string());
        download_file_with_sha256(VORT_ARCHIVE_URL, &vort_zip, VORT_ARCHIVE_SHA256).await?;
        log.push("[DOWNLOAD] Verifying VORT Shaders SHA-256 checksum: OK".to_string());

        let file = fs::File::open(&vort_zip).map_err(|e| format!("Failed to open {}: {}", vort_zip.display(), e))?;
        extract_zip(file, &vort_dir)?;
        let _ = fs::remove_file(&vort_zip);
    }

    // 3. Download ReShade standard framework headers and textures
    let slim_dir = comp_root.join("reshade-shaders-slim");
    let slim_zip = comp_root.join("reshade-shaders-slim.zip");
    if !slim_dir.join("reshade-shaders-6db142b4b1a05c764222e5b0bd9a644b7ccfe1dc").join("Shaders").join("DrawText.fxh").is_file()
        && !slim_dir.join("Shaders").join("DrawText.fxh").is_file()
    {
        log.push("[DOWNLOAD] Fetching ReShade framework headers and textures from upstream slim branch...".to_string());
        if download_file_with_sha256(RESHADE_SHADERS_SLIM_URL, &slim_zip, RESHADE_SHADERS_SLIM_SHA256).await.is_ok() {
            if let Ok(file) = fs::File::open(&slim_zip) {
                let _ = extract_zip(file, &slim_dir);
                let _ = fs::remove_file(&slim_zip);
                log.push("[FEEDER] ReShade framework headers unpacked successfully".to_string());
            }
        }
    }

    let slim_sub = if slim_dir.join("reshade-shaders-6db142b4b1a05c764222e5b0bd9a644b7ccfe1dc").is_dir() {
        slim_dir.join("reshade-shaders-6db142b4b1a05c764222e5b0bd9a644b7ccfe1dc")
    } else {
        slim_dir.clone()
    };

    // Direct fallback for individual files if needed
    let headers_dir = comp_root.join("reshade-headers");
    let fxh_path = headers_dir.join("ReShade.fxh");
    let ui_fxh_path = headers_dir.join("ReShadeUI.fxh");
    let drawtext_path = headers_dir.join("DrawText.fxh");
    let fontatlas_path = headers_dir.join("FontAtlas.png");

    if !slim_sub.join("Shaders").join("ReShade.fxh").is_file() && !fxh_path.is_file() {
        let _ = download_file_with_sha256(RESHADE_FXH_URL, &fxh_path, RESHADE_FXH_SHA256).await;
    }
    if !slim_sub.join("Shaders").join("ReShadeUI.fxh").is_file() && !ui_fxh_path.is_file() {
        let _ = download_file_with_sha256(RESHADE_UI_FXH_URL, &ui_fxh_path, RESHADE_UI_FXH_SHA256).await;
    }
    if !slim_sub.join("Shaders").join("DrawText.fxh").is_file() && !drawtext_path.is_file() {
        let _ = download_file_with_sha256(DRAWTEXT_FXH_URL, &drawtext_path, DRAWTEXT_FXH_SHA256).await;
    }
    if !slim_sub.join("Textures").join("FontAtlas.png").is_file() && !fontatlas_path.is_file() {
        let _ = download_file_with_sha256(FONTATLAS_PNG_URL, &fontatlas_path, FONTATLAS_PNG_SHA256).await;
    }

    // 4. Assemble consolidated reshade-shaders tree
    let consolidated_shaders = comp_root.join("feeder-shaders");
    let target_shaders = consolidated_shaders.join("Shaders");
    let target_textures = consolidated_shaders.join("Textures");
    let target_includes = target_shaders.join("Includes");
    fs::create_dir_all(&target_includes).map_err(|e| format!("Failed to create {}: {}", target_includes.display(), e))?;
    fs::create_dir_all(&target_textures).map_err(|e| format!("Failed to create {}: {}", target_textures.display(), e))?;

    // Copy DLSS5_Feed.fx from feeder
    let dlss5_feed_fx = feeder_dir.join("reshade-shaders").join("Shaders").join("DLSS5_Feed.fx");
    if dlss5_feed_fx.is_file() {
        let _ = fs::copy(&dlss5_feed_fx, target_shaders.join("DLSS5_Feed.fx"));
    }

    // Copy all framework headers and textures from slim bundle
    if slim_sub.join("Shaders").is_dir() {
        for entry in walkdir::WalkDir::new(slim_sub.join("Shaders")).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".fxh") || fname.ends_with(".fx") {
                    let _ = fs::copy(entry.path(), target_shaders.join(&fname));
                }
            }
        }
    }
    if slim_sub.join("Textures").is_dir() {
        for entry in walkdir::WalkDir::new(slim_sub.join("Textures")).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy().to_string();
                let _ = fs::copy(entry.path(), target_textures.join(fname));
            }
        }
    }

    // Fallback direct copies
    if fxh_path.is_file() {
        let _ = fs::copy(&fxh_path, target_shaders.join("ReShade.fxh"));
    }
    if ui_fxh_path.is_file() {
        let _ = fs::copy(&ui_fxh_path, target_shaders.join("ReShadeUI.fxh"));
    }
    if drawtext_path.is_file() {
        let _ = fs::copy(&drawtext_path, target_shaders.join("DrawText.fxh"));
    }
    if fontatlas_path.is_file() {
        let _ = fs::copy(&fontatlas_path, target_textures.join("FontAtlas.png"));
    }

    // Copy VORT motion shaders and includes
    let vort_sub = if vort_dir.join("vort_Shaders-b410b9f0c0fbb83c8cb42164aaf1655fab386f4a").is_dir() {
        vort_dir.join("vort_Shaders-b410b9f0c0fbb83c8cb42164aaf1655fab386f4a")
    } else {
        vort_dir.clone()
    };

    if vort_sub.join("Shaders").is_dir() {
        for entry in walkdir::WalkDir::new(vort_sub.join("Shaders")).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy().to_string();
                if fname.ends_with(".fx") || fname.ends_with(".fxh") {
                    let dest = if entry.path().parent().map(|p| p.ends_with("Includes")).unwrap_or(false) {
                        target_includes.join(&fname)
                    } else {
                        target_shaders.join(&fname)
                    };
                    let _ = fs::copy(entry.path(), dest);
                }
            }
        }
    }

    if vort_sub.join("Textures").is_dir() {
        for entry in walkdir::WalkDir::new(vort_sub.join("Textures")).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                let fname = entry.file_name().to_string_lossy().to_string();
                let _ = fs::copy(entry.path(), target_textures.join(fname));
            }
        }
    }

    let vk_layer_dir = if feeder_dir.join("layer-x64").join("VkLayer_feed_vk.dll").is_file() {
        Some(feeder_dir.join("layer-x64"))
    } else {
        None
    };

    log.push("[FEEDER] All Feeder shaders and components ready".to_string());

    Ok(FeederComponents {
        addon64: feeder_dir.join("dlss5-feed.addon64"),
        addon32: if feeder_dir.join("dlss5-feed.addon32").is_file() { Some(feeder_dir.join("dlss5-feed.addon32")) } else { None },
        host64: if feeder_dir.join("host64").join("dlss5-feed-host64.exe").is_file() { Some(feeder_dir.join("host64").join("dlss5-feed-host64.exe")) } else { None },
        shader_dir: consolidated_shaders,
        vk_layer_dir,
    })
}

/// Asynchronously resolves or downloads the latest MFG Unlock v1.0 addon
pub async fn ensure_mfg_v09_addon(log: &mut Vec<String>) -> Result<PathBuf, String> {
    let comp_root = get_components_root();
    let mfg_dir = comp_root.join("mfg-unlock-1.0");
    let target = mfg_dir.join("renodx-mfgunlock.addon64");

    if target.is_file() {
        if let Ok(hash) = compute_sha256(&target) {
            if hash.eq_ignore_ascii_case(MFG_10_SHA256) {
                return Ok(target);
            }
        }
    }

    log.push("[DOWNLOAD] Fetching latest MFG Unlock v1.0 (RenoDX companion) from upstream...".to_string());
    download_file_with_sha256(MFG_10_URL, &target, MFG_10_SHA256).await?;
    log.push("[DOWNLOAD] Verifying MFG Unlock v1.0 SHA-256: OK".to_string());
    Ok(target)
}

/// Asynchronously resolves or downloads NIGos's DLSS 5 D3D12 Mip Fix addon
#[allow(dead_code)]
pub async fn ensure_dlss5_d3d12_fix_addon(log: &mut Vec<String>) -> Result<PathBuf, String> {
    let comp_root = get_components_root();
    let fix_dir = comp_root.join("dlss-mip-fix");
    let target = fix_dir.join("dlss-mip-fix.addon64");

    // Check local build targets first
    for local_cand in &[
        PathBuf::from("target/release/dlss-mip-fix.addon64"),
        PathBuf::from("target/release/dlss_mip_addon.dll"),
        PathBuf::from("dist/dlss-mip-fix.addon64"),
    ] {
        if local_cand.is_file() {
            let _ = fs::create_dir_all(&fix_dir);
            let _ = fs::copy(local_cand, &target);
            return Ok(target);
        }
    }

    // Check adjacent to running executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let next_to_exe = parent.join("dlss-mip-fix.addon64");
            if next_to_exe.is_file() {
                let _ = fs::create_dir_all(&fix_dir);
                let _ = fs::copy(&next_to_exe, &target);
                return Ok(target);
            }
        }
    }

    if target.is_file() {
        return Ok(target);
    }

    // Check legacy fallback
    let legacy_target = comp_root.join("dlss5-d3d12-fix").join("dlss5-d3d12-fix.addon64");
    if legacy_target.is_file() {
        let _ = fs::create_dir_all(&fix_dir);
        let _ = fs::copy(&legacy_target, &target);
        return Ok(target);
    }

    let _ = fs::create_dir_all(&fix_dir);
    log.push("[DOWNLOAD] Fetching DLSS Studio D3D12 Mip Companion from GitHub Releases...".to_string());
    download_file_with_sha256(DLSS_MIP_FIX_URL, &target, "").await?;
    log.push("[DOWNLOAD] DLSS Studio D3D12 Mip Companion downloaded: OK".to_string());
    Ok(target)
}

/// Checks whether the RenoDX 4x MFG Unlock addon is cached on disk
pub fn is_mfg_addon_cached() -> bool {
    let root = get_components_root();
    root.join("mfg-unlock-1.0").join("renodx-mfgunlock.addon64").is_file()
        || root.join("mfg-unlock-0.9").join("renodx-mfgunlock.addon64").is_file()
        || root.join("mfg-unlock-0.8").join("renodx-mfgunlock.addon64").is_file()
        || root.join("renodx-mfgunlock.addon64").is_file()
}

/// Checks whether the DLSS 5 Feeder and shaders are cached on disk
pub fn is_feeder_cached() -> bool {
    find_local_feeder_components().is_some()
}

/// Checks whether the RenoDX v4.7 Integrated Engine is cached on disk
pub fn is_renodx_engine_cached() -> bool {
    let root = get_components_root();
    root.join("renodx-dlss5").join("renodx-dlss5.addon64").is_file()
        || root.join("renodx-dlss5.addon64").is_file()
}

/// Checks whether the D3D12 Mip Fix companion is cached on disk
pub fn is_dlss5_d3d12_fix_cached() -> bool {
    if std::path::Path::new("target/release/dlss_mip_addon.dll").is_file()
        || std::path::Path::new("target/release/dlss-mip-fix.addon64").is_file()
        || std::path::Path::new("dist/dlss-mip-fix.addon64").is_file()
    {
        return true;
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            if parent.join("dlss-mip-fix.addon64").is_file() {
                return true;
            }
        }
    }
    let root = get_components_root();
    root.join("dlss-mip-fix").join("dlss-mip-fix.addon64").is_file()
        || root.join("dlss-mip-fix.addon64").is_file()
        || root.join("dlss5-d3d12-fix").join("dlss5-d3d12-fix.addon64").is_file()
        || root.join("dlss5-d3d12-fix.addon64").is_file()
}

/// Checks whether the OptiScaler runtime is cached on disk
pub fn is_optiscaler_cached() -> bool {
    let root = get_components_root();
    root.join("OptiScaler-0.8.4-dlssnr").join("OptiScaler.dll").is_file()
        || root.join("OptiScaler-0.8.3-dlssnr").join("OptiScaler.dll").is_file()
        || root.join("OptiScaler-0.7.7-dlssnr").join("OptiScaler.dll").is_file()
        || root.join("OptiScaler.dll").is_file()
}

/// Checks whether Streamline Runtime is cached on disk
pub fn is_streamline_cached() -> bool {
    let streamline_dir = get_components_root().join("streamline-2.14.1").join("streamline");
    streamline_dir.join("sl.interposer.dll").is_file() && streamline_dir.join("sl.common.dll").is_file()
}

/// Checks whether ReShade 6.8.0 runtime (both 64-bit and 32-bit) is cached on disk
pub fn is_reshade_cached() -> bool {
    let root = get_components_root();
    (root.join("ReShade64.dll").is_file() || root.join("reshade-vulkan").join("ReShade64.dll").is_file())
        && (root.join("ReShade32.dll").is_file() || root.join("reshade-vulkan").join("ReShade32.dll").is_file())
}

/// Checks whether all mandatory components are already cached on disk
pub fn are_all_mandatory_components_cached() -> bool {
    is_mfg_addon_cached()
        && is_feeder_cached()
        && is_renodx_engine_cached()
        && is_dlss5_d3d12_fix_cached()
        && is_optiscaler_cached()
        && is_reshade_cached()
        && is_streamline_cached()
        && is_dgvoodoo_cached()
}

/// Extracts ReShade64.dll and ReShade32.dll from the official setup archive using native Windows tar
pub fn extract_reshade_from_setup(setup_path: &Path, out_dir: &Path) -> Result<(), String> {
    let _ = fs::create_dir_all(out_dir);
    let status = silent_tar_command()
        .arg("-xf")
        .arg(setup_path)
        .arg("-C")
        .arg(out_dir)
        .arg("ReShade64.dll")
        .arg("ReShade32.dll")
        .status()
        .map_err(|e| format!("Failed to execute tar.exe to extract ReShade: {}", e))?;
    if !status.success() {
        return Err(format!("tar.exe extraction failed with code {:?}", status.code()));
    }
    Ok(())
}

/// Orchestrates the automated background downloading of all mandatory components with detailed streaming progress
pub async fn ensure_all_mandatory_components_with_progress<F>(mut progress_fn: F) -> Result<(), String>
where
    F: FnMut(DownloadProgress),
{
    let mut log = Vec::new();
    let mut errors = Vec::new();
    const TOTAL_STEPS: usize = 8;

    // 1. RenoDX v4.7 Integrated Engine
    if !is_renodx_engine_cached() {
        let comp_root = get_components_root();
        let renodx_dir = comp_root.join("renodx-dlss5");
        let target = renodx_dir.join("renodx-dlss5.addon64");
        let _ = fs::create_dir_all(&renodx_dir);
        let mut step_prog = |mut p: DownloadProgress| {
            p.percentage = ((0.0 * 100.0) + p.percentage) / TOTAL_STEPS as f32;
            progress_fn(p);
        };
        if let Err(e) = download_file_with_progress(
            RENODX_DLSS5_URL,
            &target,
            "",
            "RenoDX v4.7 (Integrated DLSS 5 Engine)",
            "renodx_engine",
            &mut step_prog,
        ).await {
            crate::core::state::log_message(&format!("@{{log_download_error|RenoDX v4.7|{}}}", e));
            errors.push(format!("RenoDX v4.7: {}", e));
        }
    }

    // 2. DLSS Studio D3D12 Mip Companion
    if !is_dlss5_d3d12_fix_cached() {
        let comp_root = get_components_root();
        let fix_dir = comp_root.join("dlss-mip-fix");
        let target = fix_dir.join("dlss-mip-fix.addon64");
        let _ = fs::create_dir_all(&fix_dir);
        let mut step_prog = |mut p: DownloadProgress| {
            p.percentage = ((1.0 * 100.0) + p.percentage) / TOTAL_STEPS as f32;
            progress_fn(p);
        };
        if let Err(e) = download_file_with_progress(
            DLSS_MIP_FIX_URL,
            &target,
            "",
            "DLSS Studio D3D12 Mip Companion",
            "dlss_mip_fix",
            &mut step_prog,
        ).await {
            crate::core::state::log_message(&format!("@{{log_download_error|DLSS Studio D3D12 Mip Companion|{}}}", e));
            errors.push(format!("DLSS Studio D3D12 Mip Companion: {}", e));
        }
    }

    // 3. RenoDX 4x MFG Unlock v1.0
    if !is_mfg_addon_cached() {
        let comp_root = get_components_root();
        let mfg_dir = comp_root.join("mfg-unlock-1.0");
        let target = mfg_dir.join("renodx-mfgunlock.addon64");
        let _ = fs::create_dir_all(&mfg_dir);
        let mut step_prog = |mut p: DownloadProgress| {
            p.percentage = ((2.0 * 100.0) + p.percentage) / TOTAL_STEPS as f32;
            progress_fn(p);
        };
        if let Err(e) = download_file_with_progress(
            MFG_10_URL,
            &target,
            MFG_10_SHA256,
            "RenoDX 4x MFG Unlock v1.0",
            "mfg_unlock",
            &mut step_prog,
        ).await {
            crate::core::state::log_message(&format!("@{{log_download_error|RenoDX 4x MFG Unlock v1.0|{}}}", e));
            errors.push(format!("RenoDX 4x MFG Unlock v1.0: {}", e));
        }
    }

    // 4. DLSS 5 Feeder & Motion Shaders
    if !is_feeder_cached() {
        let comp_root = get_components_root();
        let feeder_dir = comp_root.join("DLSS5-Feeder-1.16.0-beta.3");
        let feeder_zip = comp_root.join("DLSS5-Feeder-1.16.0-beta.3.zip");
        if !feeder_dir.join("dlss5-feed.addon64").is_file() {
            let mut step_prog = |mut p: DownloadProgress| {
                p.percentage = ((3.0 * 100.0) + p.percentage) / TOTAL_STEPS as f32;
                progress_fn(p);
            };
            if let Err(e) = download_file_with_progress(
                FEEDER_ARCHIVE_URL,
                &feeder_zip,
                FEEDER_ARCHIVE_SHA256,
                "DLSS 5 Feeder (Neural Pipeline Interceptor)",
                "feeder",
                &mut step_prog,
            ).await {
                crate::core::state::log_message(&format!("@{{log_download_error|DLSS 5 Feeder|{}}}", e));
                errors.push(format!("DLSS 5 Feeder: {}", e));
            } else if let Ok(file) = fs::File::open(&feeder_zip) {
                let feeder_dir_c = feeder_dir.clone();
                let feeder_zip_c = feeder_zip.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    let _ = extract_zip(file, &feeder_dir_c);
                    let _ = fs::remove_file(&feeder_zip_c);
                }).await;
            }
        }
        let _ = ensure_feeder_components(&mut log).await;
    }

    // 5. OptiScaler DLSS-NR v0.8.4
    if !is_optiscaler_cached() {
        let comp_root = get_components_root();
        let opti_dir = comp_root.join("OptiScaler-0.8.4-dlssnr");
        let zip_path = comp_root.join("OptiScaler-NR-v0.8.4.zip");
        let mut step_prog = |mut p: DownloadProgress| {
            p.percentage = ((4.0 * 100.0) + p.percentage) / TOTAL_STEPS as f32;
            progress_fn(p);
        };
        if let Err(e) = download_file_with_progress(
            OPTISCALER_084_URL,
            &zip_path,
            OPTISCALER_084_SHA256,
            "OptiScaler DLSS-NR v0.8.4",
            "optiscaler",
            &mut step_prog,
        ).await {
            crate::core::state::log_message(&format!("@{{log_download_error|OptiScaler v0.8.4|{}}}", e));
            errors.push(format!("OptiScaler DLSS-NR: {}", e));
        } else if let Ok(file) = fs::File::open(&zip_path) {
            let opti_dir_c = opti_dir.clone();
            let zip_path_c = zip_path.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let _ = extract_zip(file, &opti_dir_c);
                let _ = fs::remove_file(&zip_path_c);
            }).await;
        }
    }

    // 6. ReShade 6.8.0 Add-on Runtime
    if !is_reshade_cached() {
        let comp_root = get_components_root();
        let setup_path = comp_root.join("ReShade_Setup_6.8.0_Addon.exe");
        let mut step_prog = |mut p: DownloadProgress| {
            p.percentage = ((5.0 * 100.0) + p.percentage) / TOTAL_STEPS as f32;
            progress_fn(p);
        };
        if let Err(e) = download_file_with_progress(
            RESHADE_SETUP_URL,
            &setup_path,
            RESHADE_SETUP_SHA256,
            "ReShade 6.8.0 (Add-on Support)",
            "reshade",
            &mut step_prog,
        ).await {
            crate::core::state::log_message(&format!("@{{log_download_error|ReShade 6.8.0|{}}}", e));
            errors.push(format!("ReShade 6.8.0: {}", e));
        } else {
            let setup_path_c = setup_path.clone();
            let comp_root_c = comp_root.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let _ = extract_reshade_from_setup(&setup_path_c, &comp_root_c);
                let _ = fs::remove_file(&setup_path_c);
            }).await;
        }
    }

    // 7. Streamline Runtime v2.14.1
    if !is_streamline_cached() {
        let comp_root = get_components_root();
        let streamline_dir = comp_root.join("streamline-2.14.1");
        let zip_path = comp_root.join("streamline.zip");
        let mut step_prog = |mut p: DownloadProgress| {
            p.percentage = ((6.0 * 100.0) + p.percentage) / TOTAL_STEPS as f32;
            progress_fn(p);
        };
        if let Err(e) = download_file_with_progress(
            STREAMLINE_ZIP_URL,
            &zip_path,
            "",
            "Streamline Runtime v2.14.1",
            "streamline",
            &mut step_prog,
        ).await {
            crate::core::state::log_message(&format!("@{{log_download_error|Streamline|{}}}", e));
            errors.push(format!("Streamline Runtime v2.14.1: {}", e));
        } else if let Ok(file) = fs::File::open(&zip_path) {
            let streamline_dir_c = streamline_dir.clone();
            let zip_path_c = zip_path.clone();
            let _ = tokio::task::spawn_blocking(move || {
                let _ = extract_zip(file, &streamline_dir_c);
                let _ = fs::remove_file(&zip_path_c);
            }).await;
        }
    }

    // 8. dgVoodoo2 v2.87.5 (Legacy DirectX Wrapper)
    if !is_dgvoodoo_cached() {
        let comp_root = get_components_root();
        let zip_path = comp_root.join("dgVoodoo2_87_5.zip");
        let mut step_prog = |mut p: DownloadProgress| {
            p.percentage = ((7.0 * 100.0) + p.percentage) / TOTAL_STEPS as f32;
            progress_fn(p);
        };
        if let Err(e) = download_file_with_progress(
            DGVOODOO_URL,
            &zip_path,
            DGVOODOO_SHA256,
            "dgVoodoo2 v2.87.5 (Legacy D3D -> D3D11)",
            "dgvoodoo",
            &mut step_prog,
        ).await {
            crate::core::state::log_message(&format!("@{{log_download_error|dgVoodoo2|{}}}", e));
            errors.push(format!("dgVoodoo2: {}", e));
        } else {
            let comp_root_c = comp_root.clone();
            let zip_path_c = zip_path.clone();
            let extract_res = tokio::task::spawn_blocking(move || {
                let dgvoodoo_dir = comp_root_c.join("dgvoodoo");
                let _ = fs::create_dir_all(dgvoodoo_dir.join("x86"));
                let _ = fs::create_dir_all(dgvoodoo_dir.join("x64"));

                let _ = silent_tar_command()
                    .arg("-xf")
                    .arg(&zip_path_c)
                    .arg("-C")
                    .arg(&dgvoodoo_dir)
                    .arg("dgVoodoo.conf")
                    .status();

                let _ = silent_tar_command()
                    .arg("-xf")
                    .arg(&zip_path_c)
                    .arg("-C")
                    .arg(dgvoodoo_dir.join("x86"))
                    .arg("--strip-components")
                    .arg("2")
                    .arg("MS/x86/D3D9.dll")
                    .status();

                let _ = silent_tar_command()
                    .arg("-xf")
                    .arg(&zip_path_c)
                    .arg("-C")
                    .arg(dgvoodoo_dir.join("x86"))
                    .arg("--strip-components")
                    .arg("2")
                    .arg("MS/x86/D3D8.dll")
                    .status();

                let _ = silent_tar_command()
                    .arg("-xf")
                    .arg(&zip_path_c)
                    .arg("-C")
                    .arg(dgvoodoo_dir.join("x64"))
                    .arg("--strip-components")
                    .arg("2")
                    .arg("MS/x64/D3D9.dll")
                    .status();

                let _ = fs::remove_file(&zip_path_c);
            }).await;

            if let Err(e) = extract_res {
                errors.push(format!("dgVoodoo2 extraction error: {}", e));
            }
        }
    }

    if are_all_mandatory_components_cached() {
        progress_fn(DownloadProgress {
            is_downloading: false,
            component_name: String::new(),
            component_id: String::new(),
            url: String::new(),
            downloaded_bytes: 0,
            total_bytes: None,
            percentage: 100.0,
            stage: DownloadStage::Downloading,
            message: "All mandatory components ready".to_string(),
        });
        Ok(())
    } else {
        let err_msg = if errors.is_empty() {
            "Some mandatory components could not be cached".to_string()
        } else {
            errors.join("; ")
        };
        progress_fn(DownloadProgress {
            is_downloading: false,
            component_name: String::new(),
            component_id: String::new(),
            url: String::new(),
            downloaded_bytes: 0,
            total_bytes: None,
            percentage: 0.0,
            stage: DownloadStage::Downloading,
            message: err_msg.clone(),
        });
        Err(err_msg)
    }
}

