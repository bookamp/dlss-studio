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
