use crate::core::journal::HistoryRow;
use crate::core::scan::GameEntry;

pub const BRAND_BADGE_WEBP: &[u8] = include_bytes!("../../assets/brand-badge.webp");

pub fn copy_to_clipboard(text: &str) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let child = std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", "$input | Set-Clipboard"])
            .stdin(std::process::Stdio::piped())
            .creation_flags(0x08000000) // CREATE_NO_WINDOW
            .spawn();
        if let Ok(mut c) = child {
            if let Some(mut stdin) = c.stdin.take() {
                use std::io::Write;
                let _ = stdin.write_all(text.as_bytes());
            }
            let _ = c.wait();
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum AppStatus {
    Ready,
    DownloadingComponents,
    DownloadFailed(String),
    ScanningLibrary,
    ScanningFolder(String),
    ScanningLaunchers,
    FoundGames(usize),
    NoGamesFound(String),
    AddedGame(String),
}

pub fn format_status(lang: &str, status: &AppStatus) -> String {
    match status {
        AppStatus::Ready => crate::core::i18n::t(lang, "status_ready").to_string(),
        AppStatus::DownloadingComponents => crate::core::i18n::t(lang, "status_downloading_components").to_string(),
        AppStatus::DownloadFailed(_) => crate::core::i18n::t(lang, "status_download_failed").to_string(),
        AppStatus::ScanningLibrary => crate::core::i18n::t(lang, "status_scanning_library").to_string(),
        AppStatus::ScanningFolder(folder) => format!("{} {}...", crate::core::i18n::t(lang, "status_scanning_folder"), folder),
        AppStatus::ScanningLaunchers => crate::core::i18n::t(lang, "status_scanning_launchers").to_string(),
        AppStatus::FoundGames(count) => crate::core::i18n::t_param(lang, "status_found_games", &count.to_string()),
        AppStatus::NoGamesFound(folder) => crate::core::i18n::t_param(lang, "status_no_game_found", folder),
        AppStatus::AddedGame(name) => crate::core::i18n::t_param(lang, "status_added_game", name),
    }
}

pub fn clean_display_title(raw: &str) -> String {
    if raw.contains('_') && (raw.contains('.') || raw.contains("__")) {
        let base = raw.split('_').next().unwrap_or(raw);
        let name_part = base.split('.').last().unwrap_or(base);
        if !name_part.is_empty() {
            let mut spaced = String::new();
            let mut prev_is_lower = false;
            for ch in name_part.chars() {
                if ch.is_uppercase() && prev_is_lower {
                    spaced.push(' ');
                }
                prev_is_lower = ch.is_lowercase();
                spaced.push(ch);
            }
            return spaced;
        }
    }
    raw.replace('_', " ").replace('-', " ")
}

pub fn resolve_game_title(row: &HistoryRow, games: &[GameEntry]) -> String {
    if let Some(ref name) = row.game_name {
        if !name.trim().is_empty() {
            return name.clone();
        }
    }
    let row_p = std::path::Path::new(&row.dir);
    for g in games {
        let g_p = std::path::Path::new(&g.dir);
        if row_p == g_p || row.dir.eq_ignore_ascii_case(&g.dir.to_string_lossy()) || row_p.starts_with(g_p) || g_p.starts_with(row_p) {
            return g.name.clone();
        }
    }
    let manifests = [
        row_p.join("MicrosoftGame.config"),
        row_p.join("Content").join("MicrosoftGame.config"),
        row_p.join("appxmanifest.xml"),
    ];
    let display_name_re = regex::Regex::new(r#"(?i)DefaultDisplayName\s*=\s*"([^"]+)""#).unwrap();
    let display_name_tag_re = regex::Regex::new(r#"(?i)<DisplayName>\s*([^<]+)\s*</DisplayName>"#).unwrap();
    for m in &manifests {
        if let Ok(text) = std::fs::read_to_string(m) {
            if let Some(cap) = display_name_re.captures(&text).or_else(|| display_name_tag_re.captures(&text)) {
                let n = cap[1].trim();
                if !n.is_empty() && !n.starts_with("ms-resource:") {
                    return n.to_string();
                }
            }
        }
    }
    if let Some(fname) = row_p.file_name().and_then(|f| f.to_str()) {
        if fname.eq_ignore_ascii_case("content") || fname.eq_ignore_ascii_case("win64") || fname.eq_ignore_ascii_case("binaries") {
            if let Some(parent) = row_p.parent().and_then(|p| p.file_name()).and_then(|f| f.to_str()) {
                if parent.eq_ignore_ascii_case("binaries") {
                    if let Some(gp) = row_p.parent().and_then(|p| p.parent()).and_then(|p| p.file_name()).and_then(|f| f.to_str()) {
                        return clean_display_title(gp);
                    }
                }
                return clean_display_title(parent);
            }
        }
        return clean_display_title(fname);
    }
    row.dir.clone()
}

pub fn resolve_module_meta(rel: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    let lower = rel.to_ascii_lowercase();
    let file_name = std::path::Path::new(rel)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(rel)
        .to_ascii_lowercase();

    if file_name == "nvngx_dlss.dll" {
        ("module_dlss_sr", "NVIDIA", "vendor-nvidia", "DLSS")
    } else if file_name == "nvngx_dlssg.dll" {
        ("module_dlss_fg", "NVIDIA", "vendor-nvidia", "DLSS-G")
    } else if file_name == "nvngx_dlssd.dll" {
        ("module_dlss_rr", "NVIDIA", "vendor-nvidia", "DLSS-RR")
    } else if file_name.starts_with("amd_fidelityfx_framegeneration") || file_name.starts_with("amd_fidelityfx_dx12") {
        ("module_fsr_fg", "AMD", "vendor-amd", "FSR FG")
    } else if file_name == "sl.dlss.dll" {
        ("module_sl_dlss", "Streamline", "vendor-sl", "SL DLSS")
    } else if file_name == "sl.dlss_g.dll" {
        ("module_sl_fg", "Streamline", "vendor-sl", "SL FG")
    } else if file_name == "sl.common.dll" || file_name == "sl.interposer.dll" {
        ("module_sl_core", "Streamline", "vendor-sl", "SL Core")
    } else if file_name == "sl.reflex.dll" {
        ("module_sl_reflex", "Streamline", "vendor-sl", "Reflex")
    } else if file_name == "dxgi.dll" && (lower.contains("optiscaler") || lower.contains("nvngx")) {
        ("module_optiscaler", "OptiScaler", "vendor-opti", "OptiScaler")
    } else {
        ("module_generic_dll", "Runtime", "vendor-generic", "DLL")
    }
}
