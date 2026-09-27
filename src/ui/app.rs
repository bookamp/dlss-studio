#![allow(non_snake_case)]

use dioxus::prelude::*;
use dioxus::desktop::tao::window::ResizeDirection;
use dioxus::html::HasFileData;
use base64::Engine;
use crate::core::scan::{scan_game_directory, scan_library_root, discover_all_launchers, discover_drive_roots, dedupe_games, GameEntry};
use crate::core::gpu::{detect_gpus, GpuInfo};

const BRAND_BADGE_WEBP: &[u8] = include_bytes!("../../assets/brand-badge.webp");


use crate::core::journal::{restore_game, read_history, append_history, HistoryRow};
use crate::core::install_guards::assert_game_closed;
use crate::core::compatibility::managed_mod_root;
use crate::core::state::{load_state, save_state, touch, ago_localized, get_state_path, log_message, get_session_log, RecentEntry};
use std::fs;


fn copy_to_clipboard(text: &str) {
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

#[doc(hidden)]
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

#[doc(hidden)]
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

#[doc(hidden)]
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

#[doc(hidden)]
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

enum IngestResult {
    SingleGame(GameEntry, std::path::PathBuf),
    Library(Vec<GameEntry>, std::path::PathBuf),
    None(std::path::PathBuf),
}

fn ingest_game_or_library_path(
    path: std::path::PathBuf,
    mut games: Signal<Vec<GameEntry>>,
    mut sheet_game_idx: Signal<Option<usize>>,
    mut sheet_open: Signal<bool>,
    mut active_view: Signal<String>,
    mut managed_folders: Signal<Vec<String>>,
    mut status_state: Signal<AppStatus>,
    mut recents: Signal<Vec<RecentEntry>>,
) {
    spawn(async move {
        // 1. Resolve folder if user dropped an .exe or file
        let folder = if path.is_file() {
            path.parent().unwrap_or(&path).to_path_buf()
        } else {
            path
        };

        let folder_name = folder.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_else(|| folder.to_string_lossy().to_string());

        status_state.set(AppStatus::ScanningFolder(folder_name.clone()));

        let scan_result = tokio::task::spawn_blocking({
            let folder = folder.clone();
            move || {
                // Priority 1: Direct game directory scan
                if let Some(game) = scan_game_directory(&folder) {
                    return IngestResult::SingleGame(game, folder);
                }

                // Priority 2: If user dropped/selected a nested subfolder (e.g. bin, bin64, x64, win64, Content),
                // check parent directories up to 3 levels
                let mut curr = folder.as_path();
                for _ in 0..3 {
                    if let Some(parent) = curr.parent() {
                        if let Some(game) = scan_game_directory(parent) {
                            return IngestResult::SingleGame(game, parent.to_path_buf());
                        }
                        curr = parent;
                    } else {
                        break;
                    }
                }

                // Priority 3: Check if this is a library root containing multiple games (e.g. steamapps/common or D:\Games)
                let lib_games = scan_library_root(&folder);
                if !lib_games.is_empty() {
                    return IngestResult::Library(lib_games, folder);
                }

                IngestResult::None(folder)
            }
        }).await.unwrap_or_else(|_| IngestResult::None(folder.clone()));

        match scan_result {
            IngestResult::SingleGame(game, root_folder) => {
                let mut s = load_state();
                s.unhide_game(&game.dir);
                let f_str = root_folder.to_string_lossy().to_string();
                if !s.manual.contains(&f_str) {
                    s.manual.push(f_str.clone());
                }
                let mut current_games = games.read().clone();
                let idx = if let Some(pos) = current_games.iter().position(|g| g.dir == game.dir) {
                    current_games[pos] = game.clone();
                    pos
                } else {
                    current_games.push(game.clone());
                    current_games.len() - 1
                };
                s.cached_games = current_games.clone();
                let _ = save_state(&s);
                touch(&f_str);
                let fresh_state = load_state();
                recents.set(fresh_state.recents);
                games.set(current_games);
                sheet_game_idx.set(Some(idx));
                sheet_open.set(true);
                active_view.set("games".to_string());
                status_state.set(AppStatus::AddedGame(game.name.clone()));
                log_message(&format!("Added game: {}", game.name));
                if game.poster.is_none() {
                    trigger_artwork_resolution(games, vec![(game.name.clone(), game.dir.clone())]);
                }
            }
            IngestResult::Library(lib_games, root_folder) => {
                let count = lib_games.len();
                let mut s = load_state();
                for g in &lib_games {
                    s.unhide_game(&g.dir);
                }
                let f_str = root_folder.to_string_lossy().to_string();
                if !s.folders.contains(&f_str) {
                    s.folders.push(f_str);
                    managed_folders.set(s.folders.clone());
                }
                let mut current_games = games.read().clone();
                current_games.extend(lib_games);
                let deduped = dedupe_games(current_games);
                s.cached_games = deduped.clone();
                let _ = save_state(&s);
                games.set(deduped.clone());
                active_view.set("games".to_string());
                status_state.set(AppStatus::FoundGames(count));
                log_message(&format!("Added library folder: {} (found {} games)", root_folder.display(), count));
                let missing: Vec<(String, std::path::PathBuf)> = deduped.iter()
                    .filter(|g| g.poster.is_none())
                    .map(|g| (g.name.clone(), g.dir.clone()))
                    .collect();
                if !missing.is_empty() {
                    trigger_artwork_resolution(games, missing);
                }
            }
            IngestResult::None(folder) => {
                let name = folder.file_name()
                    .map(|f| f.to_string_lossy().to_string())
                    .unwrap_or_else(|| folder.to_string_lossy().to_string());
                status_state.set(AppStatus::NoGamesFound(name.clone()));
                log_message(&format!("No game found in {}", folder.display()));
            }
        }
    });
}

static RESOLVING_ART_DIRS: std::sync::Mutex<Option<std::collections::HashSet<std::path::PathBuf>>> =
    std::sync::Mutex::new(None);

pub fn trigger_artwork_resolution(
    mut games: Signal<Vec<GameEntry>>,
    targets: Vec<(String, std::path::PathBuf)>,
) {
    if targets.is_empty() {
        return;
    }

    let mut filtered = Vec::new();
    if let Ok(mut guard) = RESOLVING_ART_DIRS.lock() {
        let set = guard.get_or_insert_with(std::collections::HashSet::new);
        for (name, dir) in targets {
            if set.insert(dir.clone()) {
                filtered.push((name, dir));
            }
        }
    }

    if filtered.is_empty() {
        return;
    }

    spawn(async move {
        for (g_name, g_dir) in filtered {
            let art = crate::core::steamart::resolve_game_art(&g_name, &g_dir, None).await;
            if let Some(cover) = art.cover_path {
                let mut list = games.read().clone();
                if let Some(item) = list.iter_mut().find(|x| x.dir == g_dir) {
                    item.poster = Some(cover);
                    games.set(list.clone());
                    let mut s = load_state();
                    s.cached_games = list;
                    let _ = save_state(&s);
                }
            }
            if let Ok(mut guard) = RESOLVING_ART_DIRS.lock() {
                if let Some(set) = guard.as_mut() {
                    set.remove(&g_dir);
                }
            }
        }
    });
}

pub fn App() -> Element {
    let init_state = use_hook(|| {
        let mut state = load_state();
        let hidden = state.hidden.clone();
        state.cached_games.retain(|g| {
            let norm = crate::core::state::normalize_game_path(&g.dir);
            !hidden.iter().any(|h| crate::core::state::normalize_path_str(h) == norm)
        });
        let custom_names = state.custom_names.clone();
        let mut state_changed = false;
        for game in &mut state.cached_games {
            let norm = crate::core::state::normalize_game_path(&game.dir);
            if let Some(custom) = custom_names.get(&norm) {
                game.name = custom.clone();
            }
            // Normalize any legacy dlss-art:// or raw path
            if let Some(ref p) = game.poster {
                let norm_p = crate::core::steamart::normalize_art_uri(p);
                if norm_p != *p {
                    game.poster = Some(norm_p);
                    state_changed = true;
                }
            }

            let needs_art = match game.poster {
                None => true,
                Some(ref p) => p.starts_with("data:image/"),
            };
            if needs_art {
                if let Some(cached_uri) = crate::core::steamart::find_cached_art(&game.dir) {
                    game.poster = Some(cached_uri);
                    state_changed = true;
                }
            }
        }
        if state_changed {
            let _ = crate::core::state::save_state(&state);
        }
        log_message(&format!("@{{log_library_loaded|{}}}", state.cached_games.len()));
        state
    });

    let init_theme = init_state.theme.clone();
    let init_lang = init_state.lang.clone();
    let init_group = init_state.group_games_by_store;
    let init_auto_scan = init_state.auto_scan_drives;
    let init_rust_theme = init_state.rust_theme;
    let init_run_in_bg = init_state.run_in_background;
    let init_folders = init_state.folders.clone();
    let init_addons = init_state.addons.clone();
    let init_custom_addons = init_state.addon_files.clone();
    let init_overlay_theme = init_state.overlay_theme.clone();
    let init_overlay_hotkey = init_state.overlay_hotkey.clone();
    let init_overlay_enabled = init_state.overlay_enabled;
    let init_custom_overlay_themes = init_state.custom_overlay_themes.clone();
    let init_cached_games = init_state.cached_games.clone();
    let init_recents = init_state.recents.clone();
    let init_hidden_count = init_state.hidden.len();

    let mut gpus = use_signal(|| vec![GpuInfo {
        name: "DirectX 12 Physical Device".to_string(),
        vendor_id: 0x10DE,
        device_id: 0x2684,
        dedicated_video_memory: 24 * 1024 * 1024 * 1024,
        is_rtx_40: true,
    }]);
    let mut games = use_signal(move || init_cached_games);
    let recents = use_signal(move || init_recents);
    let mut is_drag_over = use_signal(|| false);
    let mut hidden_count = use_signal(move || init_hidden_count);
    let init_active_view = std::env::var("DLSS_TEST_VIEW").unwrap_or_else(|_| "home".to_string());
    let mut active_view = use_signal(move || init_active_view);
    let mut theme = use_signal(move || init_theme);
    let mut current_lang = use_signal(move || init_lang);
    let mut rust_theme = use_signal(move || init_rust_theme);
    let mut run_in_bg = use_signal(move || init_run_in_bg);
    let mut launch_at_startup = use_signal(|| crate::core::tray::is_startup_enabled());
    let mut lang_menu_open = use_signal(|| false);
    let start_in_bp = std::env::args().any(|a| a == "--big-picture" || a == "--tv" || a == "-bp");
    let mut big_picture_open = use_signal(move || start_in_bp);

    // Status bar state
    let mut status_text = use_signal(|| "Ready".to_string());
    let mut is_downloading_components = use_signal(|| !crate::core::downloader::are_all_mandatory_components_cached());
    let mut status_state = use_signal(|| {
        if crate::core::downloader::are_all_mandatory_components_cached() {
            AppStatus::Ready
        } else {
            AppStatus::DownloadingComponents
        }
    });
    let mut status_percent = use_signal(|| {
        if crate::core::downloader::are_all_mandatory_components_cached() {
            100.0f32
        } else {
            0.0f32
        }
    });

    let brand_badge_data_uri = use_hook(|| {
        let b64 = base64::engine::general_purpose::STANDARD.encode(BRAND_BADGE_WEBP);
        format!("data:image/webp;base64,{}", b64)
    });

    // Background System Tray message listener
    use_future(move || async move {
        let args: Vec<String> = std::env::args().collect();
        if args.iter().any(|a| a == "--background" || a == "--minimized") {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            crate::core::tray::trim_working_set();
        }
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            if crate::core::tray::take_show_big_picture_request() {
                crate::core::display::snap_window_to_streaming_display();
                big_picture_open.set(true);
                let win = dioxus::desktop::window();
                win.set_visible(true);
                win.set_minimized(false);
                win.set_fullscreen(true);
                win.set_focus();
            } else if crate::core::tray::take_show_window_request() {
                let win = dioxus::desktop::window();
                win.set_visible(true);
                win.set_minimized(false);
                win.set_focus();
            }
        }
    });

    // Automated background component downloader
    let mut component_download_progress = use_signal(|| {
        if crate::core::downloader::are_all_mandatory_components_cached() {
            None
        } else {
            Some(crate::core::downloader::DownloadProgress::checking())
        }
    });
    let mut trigger_component_download = use_signal(|| 0usize);

    use_future(move || {
        let _ = *trigger_component_download.read();
        async move {
            if !crate::core::downloader::are_all_mandatory_components_cached() {
                is_downloading_components.set(true);
                status_state.set(AppStatus::DownloadingComponents);
                status_percent.set(0.0);
                let res = crate::core::downloader::ensure_all_mandatory_components_with_progress(|prog| {
                    status_percent.set(prog.percentage);
                    if prog.is_downloading {
                        if *status_state.read() != AppStatus::DownloadingComponents {
                            status_state.set(AppStatus::DownloadingComponents);
                        }
                        component_download_progress.set(Some(prog));
                    } else {
                        component_download_progress.set(None);
                    }
                }).await;

                is_downloading_components.set(false);
                if crate::core::downloader::are_all_mandatory_components_cached() {
                    status_percent.set(100.0);
                    component_download_progress.set(None);
                    status_state.set(AppStatus::Ready);
                } else {
                    let err = res.err().unwrap_or_default();
                    status_percent.set(0.0);
                    component_download_progress.set(None);
                    status_state.set(AppStatus::DownloadFailed(err));
                }
            } else {
                is_downloading_components.set(false);
                status_percent.set(100.0);
                component_download_progress.set(None);
                status_state.set(AppStatus::Ready);
            }
        }
    });

    use_effect(move || {
        let t = theme.read().clone();
        let l = current_lang.read().clone();
        let rt = if *rust_theme.read() { "true" } else { "false" };
        let script = format!(
            "document.documentElement.setAttribute('data-theme', '{}'); document.documentElement.setAttribute('data-rust-theme', '{}'); document.documentElement.lang = '{}';",
            t, rt, l
        );
        let _ = dioxus::desktop::window().webview.evaluate_script(&script);
    });

    // Search and filters
    let mut search_query = use_signal(String::new);
    let mut filter_api = use_signal(|| "all".to_string());
    let mut filter_dlss = use_signal(|| "all".to_string());
    let mut filter_addon = use_signal(|| "all".to_string());

    // Sheet / Game detail modal state
    let init_sheet_open = std::env::var("DLSS_TEST_SHEET").is_ok();
    let mut sheet_open = use_signal(move || init_sheet_open);
    let mut sheet_game_idx = use_signal(|| if std::env::var("DLSS_TEST_SHEET").is_ok() { Some(0) } else { None });
    let mut last_sheet_game_dir = use_signal(|| None::<std::path::PathBuf>);
    let mut is_editing_game_name = use_signal(|| false);
    let mut edit_game_name_val = use_signal(|| String::new());
    let mut backend_choice = use_signal(|| "reshade".to_string());
    let mut route_choice = use_signal(|| "native".to_string());
    let mut opti_pre_sr = use_signal(|| true);
    let mut opti_passes = use_signal(|| 3u32);
    let mut mfg_choice = use_signal(|| false);
    let mut mfg_multiplier = use_signal(|| 4u32);
    let mut nr_style_choice = use_signal(|| true);
    let mut nr_style_preset = use_signal(|| 0usize);
    let mut job_lines = use_signal(|| vec!["@{status_ready}".to_string()]);
    let mut copy_toast = use_signal(|| false);
    let mut copy_toast_text = use_signal(|| "Copied to clipboard".to_string());
    let mut toast_generation = use_signal(|| 0u64);

    use_effect(move || {
        let gen = *toast_generation.read();
        if *copy_toast.read() {
            spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                if *toast_generation.peek() == gen {
                    copy_toast.set(false);
                }
            });
        }
    });
    let mut modules_expanded = use_signal(|| false);

    // Settings state
    let mut group_games_by_store = use_signal(move || init_group);
    let mut auto_scan_drives = use_signal(move || init_auto_scan);
    let mut managed_folders = use_signal(move || init_folders);
    let init_vibepollo_auto_add = init_state.vibepollo_auto_add;
    let vibepollo_detected = use_signal(|| crate::core::vibepollo::is_vibepollo_installed());
    let mut vibepollo_auto_add = use_signal(move || init_vibepollo_auto_add);
    let mut vibepollo_updating = use_signal(|| false);

    // Activity log for Home view
    let mut activity_log = use_signal(get_session_log);

    // Add-on management state
    let mut addons_active = use_signal(move || init_addons);
    let mut custom_addons = use_signal(move || init_custom_addons);
    let mut show_addon_dlg = use_signal(|| false);
    let mut dlg_path = use_signal(String::new);
    let mut dlg_file_info = use_signal(String::new);
    let mut dlg_name = use_signal(String::new);
    let mut dlg_desc = use_signal(String::new);
    let mut dlg_tag = use_signal(String::new);

    let _overlay_enabled = use_signal(move || init_overlay_enabled);
    let mut overlay_theme = use_signal(move || init_overlay_theme);
    let mut overlay_hotkey = use_signal(move || init_overlay_hotkey);
    let mut custom_overlay_themes = use_signal(move || init_custom_overlay_themes);
    let init_hotkey_open = std::env::var("DLSS_TEST_HOTKEY").is_ok();
    let mut overlay_hotkey_open = use_signal(move || init_hotkey_open);
    let mut overlay_create_open = use_signal(|| false);
    let init_preview_open = std::env::var("DLSS_TEST_PREVIEW").is_ok();
    let mut overlay_preview_open = use_signal(move || init_preview_open);
    let preview_theme_id = use_signal(|| "green".to_string());
    let mut custom_theme_name = use_signal(|| "My Theme".to_string());
    let mut custom_theme_color = use_signal(|| "#ff7a00".to_string());
    let mut is_busy = use_signal(|| false);

    // Interactive preview controls for the live RenoDX modal
    let mut prev_dlss_on = use_signal(|| true);
    let mut prev_structure = use_signal(|| 1.00f32);
    let mut prev_tone = use_signal(|| 1.00f32);
    let mut prev_char_mask = use_signal(|| true);
    let mut prev_char_structure = use_signal(|| 1.00f32);
    let mut prev_nr_style = use_signal(|| 0usize);
    let mut prev_overall_intensity = use_signal(|| 1.00f32);
    let mut prev_local_tone = use_signal(|| 0.00f32);
    let mut prev_diffuse_white = use_signal(|| 203.0f32);
    let mut prev_motion_x = use_signal(|| 1.00f32);
    let mut prev_motion_y = use_signal(|| 1.00f32);

    // History state
    let mut history_rows = use_signal(read_history);

    // Async GPU detection without blocking UI - runs exactly once on mount
    use_hook(|| {
        spawn(async move {
            if let Ok(detected) = tokio::task::spawn_blocking(detect_gpus).await {
                if !detected.is_empty() {
                    gpus.set(detected);
                }
            }
        });
    });

    // Async background library scan: completely non-blocking in background thread - runs exactly once on mount
    use_hook(|| {
        spawn(async move {
            static SCAN_RUNNING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            use std::sync::atomic::Ordering;
            if SCAN_RUNNING.swap(true, Ordering::SeqCst) {
                return;
            }

            if !*is_downloading_components.read() {
                status_text.set("Scanning library in background...".to_string());
                status_state.set(AppStatus::ScanningLibrary);
                status_percent.set(30.0);
            }
            log_message("@{log_scanning_background}");
            activity_log.set(get_session_log());

            let fresh = tokio::task::spawn_blocking(move || {
                let mut detected = discover_all_launchers();
                let state = load_state();
                for manual_path in &state.manual {
                    let p = std::path::Path::new(manual_path);
                    if p.exists() {
                        if let Some(game) = scan_game_directory(p) {
                            detected.push(game);
                        }
                    }
                }
                for folder_path in &state.folders {
                    let p = std::path::Path::new(folder_path);
                    if p.exists() {
                        detected.extend(scan_library_root(p));
                    }
                }
                if state.auto_scan_drives {
                    for root in discover_drive_roots(&state.excluded_roots) {
                        detected.extend(scan_library_root(&root));
                    }
                }
                let mut games = dedupe_games(detected);
                games.retain(|g| !state.is_hidden(&g.dir));
                games
            }).await.unwrap_or_default();

            SCAN_RUNNING.store(false, Ordering::SeqCst);

            if !fresh.is_empty() {
                games.set(fresh.clone());
                let mut s = load_state();
                s.cached_games = fresh.clone();
                let _ = save_state(&s);

                // Background Steam Art Resolver: resolve any games still missing posters
                let missing: Vec<(String, std::path::PathBuf)> = fresh.iter()
                    .filter(|g| g.poster.is_none())
                    .map(|g| (g.name.clone(), g.dir.clone()))
                    .collect();

                if !missing.is_empty() {
                    trigger_artwork_resolution(games, missing);
                }

                // Automatic sweep of unreferenced / orphaned artwork
                let s_art = load_state();
                let stats = crate::core::steamart::cleanup_unused_art(&fresh, &s_art.hidden, &s_art.posters);
                if stats.files_removed > 0 {
                    log_message(&format!("Cleaned {} orphaned art files (reclaimed {} KB)", stats.files_removed, stats.bytes_reclaimed / 1024));
                }
            }

            let g_count = fresh.len();
            let dx12_count = fresh.iter().filter(|g| g.api == "DirectX 12").count();
            let mut sources = std::collections::HashSet::new();
            for g in &fresh {
                sources.insert(&g.launcher);
            }
            log_message(&format!("@{{log_found_games|{}|{}}}", g_count, sources.len()));
            log_message(&format!("@{{log_library_ready|{}|{}}}", g_count, dx12_count));
            activity_log.set(get_session_log());

            if !*is_downloading_components.read() {
                status_text.set("Ready".to_string());
                status_state.set(AppStatus::Ready);
                status_percent.set(100.0);
            }
        });
    });

    let primary_gpu = gpus.read().first().cloned().unwrap_or_else(|| GpuInfo {
        name: "DirectX 12 Physical Device".to_string(),
        vendor_id: 0x10DE,
        device_id: 0x2684,
        dedicated_video_memory: 24 * 1024 * 1024 * 1024,
        is_rtx_40: true,
    });

    let selected_game = sheet_game_idx.read().and_then(|idx| games.read().get(idx).cloned());

    use_effect(move || {
        if *sheet_open.read() {
            if let Some(idx) = *sheet_game_idx.read() {
                if let Some(g) = games.read().get(idx) {
                    let cur_dir = Some(g.dir.clone());
                    if *last_sheet_game_dir.read() != cur_dir {
                        last_sheet_game_dir.set(cur_dir);

                        let is_deployed = g.installed_route.is_some()
                            || g.optiscaler_installed
                            || g.addon_installed
                            || g.reshade_installed
                            || g.has_backup;

                        if is_deployed {
                            // Reflect existing deployed configuration
                            let mult = if g.mfg_multiplier > 0 { g.mfg_multiplier } else { 4 };
                            mfg_multiplier.set(mult);
                            mfg_choice.set(g.mfg_unlock_installed);
                            nr_style_preset.set(g.nr_style);
                            nr_style_choice.set(g.nr_style_enabled);

                            if g.installed_route.as_deref() == Some("optiscaler") || (g.optiscaler_installed && g.installed_route.is_none()) {
                                backend_choice.set("optiscaler".to_string());
                                opti_pre_sr.set(g.optiscaler_presr);
                                opti_passes.set(if g.optiscaler_passes > 0 { g.optiscaler_passes } else { 1 });
                            } else if g.installed_route.as_deref() == Some("native") {
                                backend_choice.set("reshade".to_string());
                                route_choice.set("native".to_string());
                            } else if g.installed_route.as_deref() == Some("feeder") {
                                backend_choice.set("reshade".to_string());
                                route_choice.set("feeder".to_string());
                            } else if g.reshade_installed {
                                backend_choice.set("reshade".to_string());
                                let rec = crate::core::install_routes::recommended_route(g);
                                route_choice.set(rec.as_str().to_string());
                            }
                        } else {
                            // Unmodded / Vanilla: Automatically select the best compatible path!
                            let rec = crate::core::install_routes::recommended_route(g);
                            backend_choice.set("reshade".to_string());
                            route_choice.set(rec.as_str().to_string());
                            mfg_choice.set(false);
                            mfg_multiplier.set(4);
                            opti_pre_sr.set(true);
                            opti_passes.set(1);
                            nr_style_preset.set(0);
                            nr_style_choice.set(true);
                        }
                    }
                }
            }
        } else {
            last_sheet_game_dir.set(None);
        }
    });

    // Filter games list (lazy: only evaluate when games tab is active)
    let (visible_games, store_order): (Vec<(usize, GameEntry)>, Vec<String>) = if *active_view.read() == "games" {
        let q = search_query.read().to_lowercase();
        let api = filter_api.read().to_lowercase();
        let dlss = filter_dlss.read().clone();
        let addon = filter_addon.read().clone();

        let vg: Vec<(usize, GameEntry)> = games.read().iter().cloned().enumerate().filter(|(_, g)| {
            if !q.is_empty() && !g.name.to_lowercase().contains(&q) && !g.dir.to_string_lossy().to_lowercase().contains(&q) {
                return false;
            }
            if api != "all" {
                if api == "d3d12" && !g.api.contains("12") { return false; }
                if api == "d3d11" && !g.api.contains("11") { return false; }
                if api == "vulkan" && !g.api.to_lowercase().contains("vulkan") { return false; }
                if api == "d3d9" && !g.api.contains("9") { return false; }
                if api == "d3d8" && !g.api.contains("8") { return false; }
                if api == "opengl" && !g.api.to_lowercase().contains("opengl") { return false; }
            }
            if dlss == "ready" && g.dlss_version.is_none() && !g.api.contains("12") { return false; }
            if dlss == "has_dlss" && g.dlss_version.is_none() { return false; }
            if dlss == "absent" && g.dlss_version.is_some() { return false; }
            if dlss == "installed" && !g.has_backup { return false; }
            if addon == "installed" && !g.optiscaler_installed && !g.mfg_unlock_installed { return false; }

            true
        }).collect();

        let mut so = crate::core::scan::DEFAULT_STORE_ORDER
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        for (_, g) in &vg {
            if !so.contains(&g.launcher) && !g.launcher.is_empty() {
                so.push(g.launcher.clone());
            }
        }
        (vg, so)
    } else {
        (Vec::new(), Vec::new())
    };

    rsx! {
        div {
            class: "shell",
            onclick: move |_| {
                if *lang_menu_open.read() {
                    lang_menu_open.set(false);
                }
            },

            // ---------------- 8-ZONE PERIMETER WINDOW RESIZE HANDLES ----------------
            if !*big_picture_open.read() {
                div {
                    class: "win-resize-handle win-resize-top",
                    onmousedown: move |e| {
                        e.stop_propagation();
                        let _ = dioxus::desktop::window().drag_resize_window(ResizeDirection::North);
                    },
                }
                div {
                    class: "win-resize-handle win-resize-bottom",
                    onmousedown: move |e| {
                        e.stop_propagation();
                        let _ = dioxus::desktop::window().drag_resize_window(ResizeDirection::South);
                    },
                }
                div {
                    class: "win-resize-handle win-resize-left",
                    onmousedown: move |e| {
                        e.stop_propagation();
                        let _ = dioxus::desktop::window().drag_resize_window(ResizeDirection::West);
                    },
                }
                div {
                    class: "win-resize-handle win-resize-right",
                    onmousedown: move |e| {
                        e.stop_propagation();
                        let _ = dioxus::desktop::window().drag_resize_window(ResizeDirection::East);
                    },
                }
                div {
                    class: "win-resize-handle win-resize-top-left",
                    onmousedown: move |e| {
                        e.stop_propagation();
                        let _ = dioxus::desktop::window().drag_resize_window(ResizeDirection::NorthWest);
                    },
                }
                div {
                    class: "win-resize-handle win-resize-top-right",
                    onmousedown: move |e| {
                        e.stop_propagation();
                        let _ = dioxus::desktop::window().drag_resize_window(ResizeDirection::NorthEast);
                    },
                }
                div {
                    class: "win-resize-handle win-resize-bottom-left",
                    onmousedown: move |e| {
                        e.stop_propagation();
                        let _ = dioxus::desktop::window().drag_resize_window(ResizeDirection::SouthWest);
                    },
                }
                div {
                    class: "win-resize-handle win-resize-bottom-right",
                    onmousedown: move |e| {
                        e.stop_propagation();
                        let _ = dioxus::desktop::window().drag_resize_window(ResizeDirection::SouthEast);
                    },
                }
            }

            // ---------------- SIDEBAR ----------------
            aside {
                class: "sidebar",
                style: "cursor: default; user-select: none;",
                div {
                    class: "brand",
                    id: "brand",
                    style: "cursor: default; user-select: none; display: flex; align-items: center; justify-content: center; width: 100%;",
                    onmousedown: move |_| { dioxus::desktop::window().drag(); },
                    ondoubleclick: move |_| { dioxus::desktop::window().toggle_maximized(); },
                    img {
                        class: "brand-rust-badge",
                        src: "{brand_badge_data_uri}",
                        alt: "DLSS 5 STUDIO",
                    }
                }

                nav {
                    class: "nav",
                    id: "nav",
                    onmousedown: move |e| e.stop_propagation(),
                    button {
                        class: if *active_view.read() == "home" { "nav-item active" } else { "nav-item" },
                        onclick: move |_| active_view.set("home".to_string()),
                        svg { view_box: "0 0 24 24",
                            path { d: "M3 10.5 12 3l9 7.5" }
                            path { d: "M5 9.5V20h14V9.5" }
                        }
                        span { "{crate::core::i18n::t(&current_lang.read(), \"nav_home\")}" }
                        i { class: "dot" }
                    }
                    button {
                        class: if *active_view.read() == "games" { "nav-item active" } else { "nav-item" },
                        onclick: move |_| active_view.set("games".to_string()),
                        svg { view_box: "0 0 24 24",
                            rect { x: "2", y: "7", width: "20", height: "11", rx: "4" }
                            path { d: "M7 11v3m-1.5-1.5h3M16 12h.01M18.5 14h.01" }
                        }
                        span { "{crate::core::i18n::t(&current_lang.read(), \"nav_games\")}" }
                        i { class: "dot" }
                    }
                    button {
                        class: if *active_view.read() == "addons" { "nav-item active" } else { "nav-item" },
                        onclick: move |_| active_view.set("addons".to_string()),
                        svg { view_box: "0 0 24 24",
                            path { d: "M10 4.5a1.9 1.9 0 1 1 3.8 0V6H17a1 1 0 0 1 1 1v3.2h1.5a1.9 1.9 0 1 1 0 3.8H18V17a1 1 0 0 1-1 1h-3.2v1.5a1.9 1.9 0 1 1-3.8 0V18H7a1 1 0 0 1-1-1v-3.2H4.5a1.9 1.9 0 1 1 0-3.8H6V7a1 1 0 0 1 1-1h3z" }
                        }
                        span { "{crate::core::i18n::t(&current_lang.read(), \"nav_addons\")}" }
                        i { class: "dot" }
                    }
                    button {
                        class: if *active_view.read() == "history" { "nav-item active" } else { "nav-item" },
                        onclick: move |_| {
                            history_rows.set(read_history());
                            active_view.set("history".to_string());
                        },
                        svg { view_box: "0 0 24 24",
                            path { d: "M3 12a9 9 0 1 0 3-6.7" }
                            path { d: "M3 4v5h5M12 7v5l3.5 2" }
                        }
                        span { "{crate::core::i18n::t(&current_lang.read(), \"nav_history\")}" }
                        i { class: "dot" }
                    }
                    button {
                        class: if *active_view.read() == "settings" { "nav-item active" } else { "nav-item" },
                        onclick: move |_| active_view.set("settings".to_string()),
                        svg { view_box: "0 0 24 24",
                            circle { cx: "12", cy: "12", r: "3.2" }
                            path { d: "M19.5 14.5a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.9 1.7 1.7 0 0 0-1.6-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3H9a1.7 1.7 0 0 0 1-1.6V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.6 1.7 1.7 0 0 0 1.9-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.9V9a1.7 1.7 0 0 0 1.6 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.4 1z" }
                        }
                        span { "{crate::core::i18n::t(&current_lang.read(), \"nav_settings\")}" }
                        i { class: "dot" }
                    }
                    button {
                        class: if *active_view.read() == "about" { "nav-item active" } else { "nav-item" },
                        onclick: move |_| active_view.set("about".to_string()),
                        svg { view_box: "0 0 24 24",
                            circle { cx: "12", cy: "12", r: "9" }
                            path { d: "M12 17v-5M12 8h.01" }
                        }
                        span { "{crate::core::i18n::t(&current_lang.read(), \"nav_about\")}" }
                        i { class: "dot" }
                    }
                }

                div {
                    style: "flex: 1; min-height: 24px; cursor: default; user-select: none;",
                    onmousedown: move |_| { dioxus::desktop::window().drag(); },
                    ondoubleclick: move |_| { dioxus::desktop::window().toggle_maximized(); },
                }

                div {
                    class: "status-card",
                    style: if matches!(*status_state.read(), AppStatus::DownloadFailed(_)) { "cursor: pointer;" } else { "" },
                    title: if matches!(*status_state.read(), AppStatus::DownloadFailed(_)) { crate::core::i18n::t(&current_lang.read(), "tooltip_retry_download") } else { "" },
                    onclick: move |_| {
                        if matches!(*status_state.read(), AppStatus::DownloadFailed(_)) {
                            *trigger_component_download.write() += 1;
                        }
                    },
                    onmousedown: move |e| e.stop_propagation(),
                    div { class: "status-line",
                        i {
                            class: match *status_state.read() {
                                AppStatus::DownloadingComponents => "live downloading",
                                AppStatus::DownloadFailed(_) => "live error",
                                _ => "live",
                            }
                        }
                        b { id: "statusText", "{format_status(&current_lang.read(), &status_state.read())}" }
                    }
                    p {
                        class: "status-sub",
                        if matches!(*status_state.read(), AppStatus::DownloadingComponents) {
                            if let Some(ref prog) = *component_download_progress.read() {
                                "{crate::core::downloader::format_download_progress(&current_lang.read(), prog)}"
                            } else {
                                "{status_percent.read():.0}%"
                            }
                        } else {
                            "DLSS 5 Studio"
                        }
                    }
                    p { class: "status-sub", id: "statusVersion", "v{crate::core::APP_VERSION}" }
                    div { class: "bar",
                        i { id: "statusBar", style: "width: {status_percent.read()}%;" }
                    }
                }
            }

            // ---------------- MAIN WORKSPACE ----------------
            div { class: "main",
                // Toolbar
                div {
                    class: "toolbar",
                    style: "cursor: default; user-select: none;",
                    onmousedown: move |_| { dioxus::desktop::window().drag(); },
                    ondoubleclick: move |_| { dioxus::desktop::window().toggle_maximized(); },
                    div {
                        style: "flex: 1; height: 34px; cursor: default;",
                        onmousedown: move |_| { dioxus::desktop::window().drag(); },
                        ondoubleclick: move |_| { dioxus::desktop::window().toggle_maximized(); },
                    }
                    button {
                        id: "themeBtn",
                        class: "round",
                        title: if *theme.read() == "dark" { crate::core::i18n::t(&current_lang.read(), "tooltip_switch_light") } else { crate::core::i18n::t(&current_lang.read(), "tooltip_switch_dark") },
                        onmousedown: move |e| e.stop_propagation(),
                        onclick: move |e| {
                            e.stop_propagation();
                            let next = if *theme.read() == "dark" { "light" } else { "dark" };
                            theme.set(next.to_string());
                            let mut s = load_state();
                            s.theme = next.to_string();
                            let _ = save_state(&s);
                            let script = format!("document.documentElement.setAttribute('data-theme', '{}');", next);
                            let _ = dioxus::desktop::window().webview.evaluate_script(&script);
                        },
                        dangerous_inner_html: r#"<svg viewBox="0 0 24 24"><circle cx="12" cy="12" r="4.2"/><path d="M12 2v2.5M12 19.5V22M2 12h2.5M19.5 12H22M4.9 4.9l1.8 1.8M17.3 17.3l1.8 1.8M19.1 4.9l-1.8 1.8M6.7 17.3l-1.8 1.8"/></svg>"#
                    }
                    div {
                        class: "lang-wrap",
                        style: "position: relative;",
                        onmousedown: move |e| e.stop_propagation(),
                        button {
                            id: "langBtn",
                            class: "pill",
                            onmousedown: move |e| e.stop_propagation(),
                            onclick: move |e| {
                                e.stop_propagation();
                                lang_menu_open.toggle();
                            },
                            span { id: "langLabel", "{current_lang.read().to_uppercase()}" }
                            svg { view_box: "0 0 24 24", path { d: "M6 9l6 6 6-6" } }
                        }
                        if *lang_menu_open.read() {
                            div {
                                class: "lang-menu",
                                id: "langMenu",
                                style: "max-height: 380px; overflow-y: auto;",
                                onmousedown: move |e| e.stop_propagation(),
                                onclick: move |e| e.stop_propagation(),
                                for item in crate::core::i18n::SUPPORTED_LANGS {
                                    {
                                        let code = item.code;
                                        let is_active = *current_lang.read() == code;
                                        rsx! {
                                            button {
                                                class: if is_active { "lang-item active" } else { "lang-item" },
                                                onmousedown: move |e| e.stop_propagation(),
                                                onclick: move |e| {
                                                    e.stop_propagation();
                                                    current_lang.set(code.to_string());
                                                    let mut s = load_state();
                                                    s.lang = code.to_string();
                                                    let _ = save_state(&s);
                                                    let script = format!("document.documentElement.lang = '{}';", code);
                                                    let _ = dioxus::desktop::window().webview.evaluate_script(&script);
                                                    lang_menu_open.set(false);
                                                },
                                                span { "{item.native}" }
                                                span { class: "code", "{item.code.to_uppercase()}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    button {
                        class: "win",
                        id: "winBigPicture",
                        title: "Big Picture Mode (Gamepad & TV)",
                        onmousedown: move |e| e.stop_propagation(),
                        onclick: move |e| {
                            e.stop_propagation();
                            crate::core::display::snap_window_to_streaming_display();
                            dioxus::desktop::window().set_fullscreen(true);
                            big_picture_open.set(true);
                        },
                        dangerous_inner_html: r#"<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="2" y="6" width="20" height="12" rx="2"/><path d="M6 12h4m-2-2v4m9-2h.01m3 0h.01"/></svg>"#
                    }
                    button {
                        class: "win",
                        id: "winMin",
                        title: "{crate::core::i18n::t(&current_lang.read(), \"win_min\")}",
                        onmousedown: move |e| e.stop_propagation(),
                        onclick: move |e| {
                            e.stop_propagation();
                            dioxus::desktop::window().set_minimized(true);
                        },
                        dangerous_inner_html: r#"<svg viewBox="0 0 24 24"><path d="M6 12h12"/></svg>"#
                    }
                    button {
                        class: "win",
                        id: "winMax",
                        title: "{crate::core::i18n::t(&current_lang.read(), \"win_max\")}",
                        onmousedown: move |e| e.stop_propagation(),
                        onclick: move |e| {
                            e.stop_propagation();
                            dioxus::desktop::window().toggle_maximized();
                        },
                        dangerous_inner_html: r#"<svg viewBox="0 0 24 24"><rect x="5" y="5" width="14" height="14" rx="2" fill="none" stroke="currentColor" stroke-width="2"/></svg>"#
                    }
                    button {
                        class: "win close",
                        id: "winClose",
                        title: "{crate::core::i18n::t(&current_lang.read(), \"win_close\")}",
                        onmousedown: move |e| e.stop_propagation(),
                        onclick: move |e| {
                            e.stop_propagation();
                            if *run_in_bg.read() {
                                dioxus::desktop::window().set_visible(false);
                                crate::core::tray::show_background_notification();
                                crate::core::tray::trim_working_set();
                            } else {
                                dioxus::desktop::window().close();
                                std::process::exit(0);
                            }
                        },
                        dangerous_inner_html: r#"<svg viewBox="0 0 24 24"><path d="M7 7l10 10M17 7L7 17"/></svg>"#
                    }
                }

                // ---------------- HOME VIEW ----------------
                if *active_view.read() == "home" {
                    section {
                        class: "view active",
                        id: "view-home",
                        style: "cursor: default;",
                        div {
                            class: if *is_drag_over.read() { "glass dropzone over" } else { "glass dropzone" },
                            id: "dropZone",
                            style: "cursor: pointer;",
                            onmousedown: move |e| e.stop_propagation(),
                            ondragover: move |e| {
                                e.prevent_default();
                                is_drag_over.set(true);
                            },
                            ondragenter: move |e| {
                                e.prevent_default();
                                is_drag_over.set(true);
                            },
                            ondragleave: move |_| {
                                is_drag_over.set(false);
                            },
                            ondrop: move |evt: DragEvent| {
                                is_drag_over.set(false);
                                if let Some(engine) = evt.files() {
                                    let files = engine.files();
                                    if let Some(first_path) = files.into_iter().next() {
                                        ingest_game_or_library_path(
                                            std::path::PathBuf::from(first_path),
                                            games,
                                            sheet_game_idx,
                                            sheet_open,
                                            active_view,
                                            managed_folders,
                                            status_state,
                                            recents,
                                        );
                                    }
                                }
                            },
                            onclick: move |_| {
                                spawn(async move {
                                    if let Some(handle) = rfd::AsyncFileDialog::new().set_title("Browse for game folder").pick_folder().await {
                                        ingest_game_or_library_path(
                                            handle.path().to_path_buf(),
                                            games,
                                            sheet_game_idx,
                                            sheet_open,
                                            active_view,
                                            managed_folders,
                                            status_state,
                                            recents,
                                        );
                                    }
                                });
                            },
                            div { class: "dz-circle",
                                dangerous_inner_html: r#"<svg viewBox="0 0 24 24"><path d="M3 8a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/></svg>"#
                            }
                            h2 { "{crate::core::i18n::t(&current_lang.read(), \"drop_title\")}" }
                            p { class: "or", "{crate::core::i18n::t(&current_lang.read(), \"home_or\")}" }
                            button {
                                class: "glass-btn",
                                id: "browseBtn",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    spawn(async move {
                                        if let Some(handle) = rfd::AsyncFileDialog::new().set_title("Browse for game folder").pick_folder().await {
                                            ingest_game_or_library_path(
                                                handle.path().to_path_buf(),
                                                games,
                                                sheet_game_idx,
                                                sheet_open,
                                                active_view,
                                                managed_folders,
                                                status_state,
                                                recents,
                                            );
                                        }
                                    });
                                },
                                "{crate::core::i18n::t(&current_lang.read(), \"home_browse_folder\")}"
                            }
                        }

                        div {
                            class: "section-head",
                            style: "cursor: default; user-select: none;",
                            h3 { "{crate::core::i18n::t(&current_lang.read(), \"home_recent_games\")}" }
                            button {
                                class: "link",
                                onmousedown: move |e| e.stop_propagation(),
                                onclick: move |_| active_view.set("games".to_string()),
                                span { "{crate::core::i18n::t(&current_lang.read(), \"home_view_all\")}" }
                                svg { view_box: "0 0 24 24", path { d: "M9 6l6 6-6 6" } }
                            }
                        }
                        div {
                            class: "recents",
                            id: "recents",
                            style: "cursor: default; user-select: none;",
                            {
                                let recent_list: Vec<(usize, GameEntry, String)> = recents.read().iter().filter_map(|r| {
                                    let time_ago = ago_localized(&current_lang.read(), r.at);
                                    let matched = games.read().iter().enumerate().find(|(_, g)| {
                                        g.dir.to_string_lossy().to_lowercase() == r.dir.to_lowercase()
                                    }).map(|(idx, g)| (idx, g.clone(), time_ago.clone()));
                                    matched
                                }).take(12).collect();

                                if recent_list.is_empty() {
                                    rsx! {
                                        p { class: "empty", "{crate::core::i18n::t(&current_lang.read(), \"home_recents_empty\")}" }
                                    }
                                } else {
                                    rsx! {
                                        for (orig_idx, game, time_str) in recent_list {
                                            {
                                                let idx = orig_idx;
                                                let g_name = game.name.clone();
                                                let g_dir = game.dir.clone();
                                                let is_ready = game.optiscaler_installed || game.mfg_unlock_installed;
                                                let poster_opt = game.poster.as_ref().map(|p| crate::core::steamart::normalize_art_uri(p));
                                                let is_dlss5 = game.is_dlss5_patched();
                                                let launch_tooltip = if is_dlss5 {
                                                    crate::core::i18n::t_params(&current_lang.read(), "tooltip_launch_game_modded", &[&g_name, game.route_display_name()])
                                                } else {
                                                    crate::core::i18n::t_param(&current_lang.read(), "tooltip_launch_game_vanilla", &g_name)
                                                };
                                                rsx! {
                                                    article {
                                                        class: "rcard",
                                                        tabindex: "0",
                                                        onmousedown: move |e| e.stop_propagation(),
                                                        onclick: move |_| {
                                                            sheet_game_idx.set(Some(idx));
                                                            sheet_open.set(true);
                                                        },
                                                        div { class: "tools",
                                                            button {
                                                                class: "tool cover-tool",
                                                                title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_change_cover\")}",
                                                                onclick: {
                                                                    let dir = g_dir.clone();
                                                                    move |e: MouseEvent| {
                                                                        e.stop_propagation();
                                                                        pick_and_set_cover(&dir, games);
                                                                    }
                                                                },
                                                                svg {
                                                                    view_box: "0 0 24 24",
                                                                    style: "width: 13px; height: 13px; fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round;",
                                                                    rect { x: "3", y: "3", width: "18", height: "18", rx: "2", ry: "2" }
                                                                    circle { cx: "8.5", cy: "8.5", r: "1.5" }
                                                                    polyline { points: "21 15 16 10 5 21" }
                                                                }
                                                            }
                                                            button {
                                                                class: "tool",
                                                                title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_open_explorer\")}",
                                                                onclick: {
                                                                    let dir = g_dir.clone();
                                                                    move |e: MouseEvent| {
                                                                        e.stop_propagation();
                                                                        let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                                                                    }
                                                                },
                                                                "📁"
                                                            }
                                                        }
                                                        button {
                                                            class: if is_dlss5 { "card-center-play modded" } else { "card-center-play vanilla" },
                                                            title: "{launch_tooltip}",
                                                            onclick: {
                                                                let g = game.clone();
                                                                move |e: MouseEvent| {
                                                                    e.stop_propagation();
                                                                    launch_game(&g);
                                                                }
                                                            },
                                                            svg {
                                                                view_box: "0 0 24 24",
                                                                polygon { points: "6,4 20,12 6,20" }
                                                            }
                                                            span { "{crate::core::i18n::t(&current_lang.read(), \"home_play\")}" }
                                                        }
                                                        if let Some(ref p_url) = poster_opt {
                                                            img { src: "{p_url}", alt: "{g_name}" }
                                                        } else {
                                                            div { class: "initials", "{&g_name[0..2.min(g_name.len())]}" }
                                                        }
                                                        div { class: "meta",
                                                            div { class: "title", "{g_name}" }
                                                            div { class: "when",
                                                                span { class: "ago", "{time_str}" }
                                                                i { class: if is_ready { "pip on" } else { "pip" } }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                            }
                        }
                            }
                                }

                        div { class: "section-head",
                            h3 { "{crate::core::i18n::t(&current_lang.read(), \"home_activity\")}" }
                            div { class: "row",
                                button {
                                    class: "ghost sm",
                                    id: "copyLog",
                                    onclick: move |_| {
                                        let lang = current_lang.read().clone();
                                        let text = activity_log.read()
                                            .iter()
                                            .map(|line| {
                                                let l = line.as_str();
                                                if l.starts_with('[') && l.len() > 10 && l.chars().nth(9) == Some(']') {
                                                    format!("{}{}", &l[0..10], crate::core::i18n::format_log_entry(&lang, &l[10..]))
                                                } else {
                                                    crate::core::i18n::format_log_entry(&lang, l)
                                                }
                                            })
                                            .collect::<Vec<String>>()
                                            .join("\r\n");
                                        copy_to_clipboard(&text);
                                        copy_toast_text.set(crate::core::i18n::t(&current_lang.read(), "toast_activity_copied").to_string());
                                        *toast_generation.write() += 1;
                                        copy_toast.set(true);
                                    },
                                    "{crate::core::i18n::t(&current_lang.read(), \"btn_copy_log\")}"
                                }
                                button {
                                    class: "ghost sm",
                                    id: "openLogFile",
                                    title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_open_desktop_log\")}",
                                    onclick: move |_| {
                                        let path = crate::core::logger::get_log_file_path();
                                        let mut cmd = std::process::Command::new("explorer");
                                        #[cfg(windows)]
                                        {
                                            use std::os::windows::process::CommandExt;
                                            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                                        }
                                        let _ = cmd.arg(&path).spawn();
                                        crate::core::logger::info("ui", &format!("Opened desktop log file: {}", path.display()));
                                    },
                                    "{crate::core::i18n::t(&current_lang.read(), \"btn_open_desktop_log\")}"
                                }
                                button {
                                    class: "ghost sm",
                                    id: "openBpLogFile",
                                    title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_open_bp_log\")}",
                                    onclick: move |_| {
                                        let path = crate::big_picture::logger::get_bp_log_path();
                                        if !path.exists() {
                                            crate::big_picture::logger::info("ui", "Big Picture log initialized");
                                        }
                                        let mut cmd = std::process::Command::new("explorer");
                                        #[cfg(windows)]
                                        {
                                            use std::os::windows::process::CommandExt;
                                            cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
                                        }
                                        let _ = cmd.arg(&path).spawn();
                                        crate::core::logger::info("ui", &format!("Opened Big Picture log file: {}", path.display()));
                                    },
                                    "{crate::core::i18n::t(&current_lang.read(), \"btn_open_bp_log\")}"
                                }
                                button {
                                    class: "ghost sm",
                                    id: "clearLog",
                                    onclick: move |_| activity_log.set(Vec::new()),
                                    "{crate::core::i18n::t(&current_lang.read(), \"btn_clear_log\")}"
                                }
                            }
                        }
                        div { class: "glass log", id: "log",
                            for line in activity_log.read().iter() {
                                {
                                    let l = line.as_str();
                                    if l.starts_with('[') && l.len() > 10 && l.chars().nth(9) == Some(']') {
                                        let time_part = &l[0..10];
                                        let msg_part = &l[10..];
                                        let rendered_msg = crate::core::i18n::format_log_entry(&current_lang.read(), msg_part);
                                        rsx! {
                                            div { class: "log-line",
                                                span { class: "log-time", "{time_part}" }
                                                span { class: "log-msg", "{rendered_msg}" }
                                            }
                                        }
                                    } else {
                                        let rendered_l = crate::core::i18n::format_log_entry(&current_lang.read(), l);
                                        rsx! {
                                            div { class: "log-line",
                                                span { class: "log-msg", "{rendered_l}" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ---------------- GAMES VIEW ----------------
                if *active_view.read() == "games" {
                    section {
                        class: "view active",
                        id: "view-games",
                        style: "cursor: default;",
                        div {
                            class: "section-head games-heading",
                            style: "cursor: default; user-select: none;",
                            h3 {
                                style: "cursor: default;",
                                span { id: "gamesTitle", "{crate::core::i18n::t(&current_lang.read(), \"nav_games\")}" }
                                span { class: "count", id: "gamesCount", " {visible_games.len()} / {games.read().len()}" }
                            }
                            div {
                                class: "row",
                                onmousedown: move |e| e.stop_propagation(),
                                button {
                                      class: "ghost sm",
                                      id: "addGame",
                                      onclick: move |_| {
                                          spawn(async move {
                                              if let Some(handle) = rfd::AsyncFileDialog::new().set_title("Add one game").pick_folder().await {
                                                  ingest_game_or_library_path(
                                                      handle.path().to_path_buf(),
                                                      games,
                                                      sheet_game_idx,
                                                      sheet_open,
                                                      active_view,
                                                      managed_folders,
                                                      status_state,
                                                      recents,
                                                  );
                                              }
                                          });
                                      },
                                      "{crate::core::i18n::t(&current_lang.read(), \"add_game\")}"
                                  }
                                 button {
                                     class: "ghost sm",
                                     id: "addFolder",
                                     onclick: move |_| {
                                         spawn(async move {
                                             if let Some(handle) = rfd::AsyncFileDialog::new().set_title("Scan this folder for games").pick_folder().await {
                                                 let folder = handle.path().to_path_buf();
                                                 status_text.set("Scanning selected folder for games...".to_string());
                                                  status_state.set(AppStatus::ScanningFolder(folder.file_name().unwrap_or_default().to_string_lossy().to_string()));
                                                 status_percent.set(50.0);
                                                 let folder_games = tokio::task::spawn_blocking({
                                                     let folder = folder.clone();
                                                     move || scan_library_root(&folder)
                                                 }).await.unwrap_or_default();

                                                 let mut s = load_state();
                                                 for g in &folder_games {
                                                     s.unhide_game(&g.dir);
                                                 }
                                                 let f_str = folder.to_string_lossy().to_string();
                                                 if !s.folders.contains(&f_str) {
                                                     s.folders.push(f_str);
                                                     managed_folders.set(s.folders.clone());
                                                 }
                                                 let mut current_games = games.read().clone();
                                                 current_games.extend(folder_games);
                                                 let deduped = dedupe_games(current_games);
                                                 s.cached_games = deduped.clone();
                                                 let _ = save_state(&s);
                                                 games.set(deduped.clone());
                                                 let missing: Vec<(String, std::path::PathBuf)> = deduped.iter()
                                                     .filter(|g| g.poster.is_none())
                                                     .map(|g| (g.name.clone(), g.dir.clone()))
                                                     .collect();
                                                 if !missing.is_empty() {
                                                     trigger_artwork_resolution(games, missing);
                                                 }
                                                 status_text.set("Ready".to_string());
                                                 status_state.set(AppStatus::Ready);
                                                 status_percent.set(100.0);
                                             }
                                         });
                                     },
                                     "{crate::core::i18n::t(&current_lang.read(), \"add_folder\")}"
                                 }
                                 button {
                                     class: "glass-btn sm",
                                     id: "rescan",
                                     onclick: move |_| {
                                         spawn(async move {
                                             if !*is_downloading_components.read() {
                                                 status_text.set("Scanning launchers and custom folders...".to_string());
                                                 status_state.set(AppStatus::ScanningLaunchers);
                                                 status_percent.set(30.0);
                                             }
                                             let deduped = tokio::task::spawn_blocking(move || {
                                                 let mut discovered = discover_all_launchers();
                                                 let s = load_state();
                                                 for folder in &s.folders {
                                                     discovered.extend(scan_library_root(folder));
                                                 }
                                                 for manual in &s.manual {
                                                     if let Some(g) = scan_game_directory(manual) {
                                                         discovered.push(g);
                                                     }
                                                 }
                                                 if s.auto_scan_drives {
                                                     for root in discover_drive_roots(&s.excluded_roots) {
                                                         discovered.extend(scan_library_root(&root));
                                                     }
                                                 }
                                                 let mut g_list = dedupe_games(discovered);
                                                 g_list.retain(|g| !s.is_hidden(&g.dir));
                                                 g_list
                                             }).await.unwrap_or_default();

                                             if !*is_downloading_components.read() {
                                                 status_percent.set(100.0);
                                                 status_text.set(crate::core::i18n::t_param(&current_lang.read(), "status_found_games", &deduped.len().to_string()));
                                                 status_state.set(AppStatus::FoundGames(deduped.len()));
                                             }
                                             games.set(deduped.clone());

                                            let mut s = load_state();
                                            s.cached_games = deduped.clone();
                                            let _ = save_state(&s);
                                            let missing: Vec<(String, std::path::PathBuf)> = deduped.iter()
                                                .filter(|g| g.poster.is_none())
                                                .map(|g| (g.name.clone(), g.dir.clone()))
                                                .collect();
                                            if !missing.is_empty() {
                                                trigger_artwork_resolution(games, missing);
                                            }

                                            // Automatic sweep of unreferenced / orphaned artwork
                                            let s_art = load_state();
                                            let stats = crate::core::steamart::cleanup_unused_art(&deduped, &s_art.hidden, &s_art.posters);
                                            if stats.files_removed > 0 {
                                                log_message(&format!("Cleaned {} orphaned art files (reclaimed {} KB)", stats.files_removed, stats.bytes_reclaimed / 1024));
                                            }
                                        });
                                    },
                                    "{crate::core::i18n::t(&current_lang.read(), \"rescan\")}"
                                }
                            }
                        }

                        div {
                            class: "game-filters",
                            role: "search",
                            onmousedown: move |e| e.stop_propagation(),
                            label { class: "game-search",
                                span { "{crate::core::i18n::t(&current_lang.read(), \"search_games\")}" }
                                input {
                                    id: "gameSearch",
                                    type: "search",
                                    placeholder: "{crate::core::i18n::t(&current_lang.read(), \"search_games\")}",
                                    value: "{search_query}",
                                    oninput: move |e| search_query.set(e.value())
                                }
                            }
                            label {
                                span { "{crate::core::i18n::t(&current_lang.read(), \"rendering_api\")}" }
                                select {
                                    id: "gameApi",
                                    value: "{filter_api}",
                                    onchange: move |e| filter_api.set(e.value()),
                                    option { value: "all", "{crate::core::i18n::t(&current_lang.read(), \"filter_all_apis\")}" }
                                    option { value: "d3d12", "DirectX 12" }
                                    option { value: "d3d11", "DirectX 11" }
                                    option { value: "vulkan", "Vulkan" }
                                    option { value: "d3d9", "DirectX 9" }
                                    option { value: "d3d8", "DirectX 8" }
                                    option { value: "opengl", "OpenGL" }
                                }
                            }
                            label {
                                span { "{crate::core::i18n::t(&current_lang.read(), \"dlss_status\")}" }
                                select {
                                    id: "gameDlss",
                                    value: "{filter_dlss}",
                                    onchange: move |e| filter_dlss.set(e.value()),
                                    option { value: "all", "{crate::core::i18n::t(&current_lang.read(), \"filter_all_status\")}" }
                                    option { value: "ready", "{crate::core::i18n::t(&current_lang.read(), \"filter_ready_dlss5\")}" }
                                    option { value: "has_dlss", "{crate::core::i18n::t(&current_lang.read(), \"filter_has_dlss\")}" }
                                    option { value: "absent", "{crate::core::i18n::t(&current_lang.read(), \"filter_no_dlss\")}" }
                                    option { value: "installed", "{crate::core::i18n::t(&current_lang.read(), \"filter_dlss5_installed\")}" }
                                }
                            }
                            label {
                                span { "{crate::core::i18n::t(&current_lang.read(), \"nav_addons\")}" }
                                select {
                                    id: "gameAddon",
                                    value: "{filter_addon}",
                                    onchange: move |e| filter_addon.set(e.value()),
                                    option { value: "all", "{crate::core::i18n::t(&current_lang.read(), \"filter_all\")}" }
                                    option { value: "installed", "{crate::core::i18n::t(&current_lang.read(), \"filter_installed\")}" }
                                }
                            }
                            button {
                                class: "ghost sm",
                                id: "clearGameFilters",
                                onclick: move |_| {
                                    search_query.set(String::new());
                                    filter_api.set("all".to_string());
                                    filter_dlss.set("all".to_string());
                                    filter_addon.set("all".to_string());
                                },
                                "{crate::core::i18n::t(&current_lang.read(), \"clear_filters\")}"
                            }
                        }

                        div { id: "groups",
                            if visible_games.is_empty() {
                                if games.read().is_empty() {
                                    div { class: "glass games-empty", style: "padding: 40px; text-align: center; color: var(--dim);",
                                        h4 { style: "font-size: 16px; margin-bottom: 8px; color: var(--text);", "{crate::core::i18n::t(&current_lang.read(), \"empty_library\")}" }
                                        p { "{crate::core::i18n::t(&current_lang.read(), \"empty_hint\")}" }
                                    }
                                } else {
                                    div { class: "glass games-empty", style: "padding: 40px; text-align: center; color: var(--dim);",
                                        h4 { style: "font-size: 16px; margin-bottom: 8px; color: var(--text);", "{crate::core::i18n::t(&current_lang.read(), \"games_no_matches\")}" }
                                        p { "{crate::core::i18n::t(&current_lang.read(), \"games_no_matches_hint\")}" }
                                    }
                                }
                            } else if *group_games_by_store.read() {
                                for store in &store_order {
                                    {
                                        let store_matches: Vec<(usize, GameEntry)> = visible_games.iter()
                                            .filter(|(_, g)| &g.launcher == store)
                                            .cloned()
                                            .collect();

                                        if store_matches.is_empty() {
                                            rsx! {}
                                        } else {
                                            let ready_count = store_matches.iter().filter(|(_, g)| g.dlss_version.is_some() || g.api.contains("12")).count();
                                            let store_display = match store.as_str() {
                                                "Added by hand" => crate::core::i18n::t(&current_lang.read(), "store_manual"),
                                                "My folders" => crate::core::i18n::t(&current_lang.read(), "store_folders"),
                                                s => s,
                                            };
                                            rsx! {
                                                section { class: "group",
                                                    div { class: "group-head",
                                                        h4 { "{store_display}" }
                                                        span { class: "count", "{store_matches.len()}" }
                                                        button {
                                                            class: if *filter_dlss.read() == "ready" { "ready filter-chip active" } else { "ready filter-chip" },
                                                            onclick: move |_| {
                                                                if *filter_dlss.read() == "ready" {
                                                                    filter_dlss.set("all".to_string());
                                                                } else {
                                                                    filter_dlss.set("ready".to_string());
                                                                }
                                                            },
                                                            "{crate::core::i18n::t(&current_lang.read(), \"filter_ready_dlss5\")} ({ready_count})"
                                                        }
                                                    }
                                                    div { class: "grid",
                                                        for (orig_idx, game) in store_matches {
                                                            {
                                                                let idx = orig_idx;
                                                                let g_name = game.name.clone();
                                                                let g_api = game.api.clone();
                                                                let g_dir = game.dir.clone();
                                                                let has_dlss = game.dlss_version.is_some();
                                                                let dlss_label = if let Some(ref v) = game.dlss_version {
                                                                    crate::core::scan::short_version(v)
                                                                } else {
                                                                    crate::core::i18n::t(&current_lang.read(), "badge_no_dlss").to_string()
                                                                };
                                                                let g_opti = game.optiscaler_installed;
                                                                let has_addon = game.mfg_unlock_installed || game.has_backup;
                                                                let is_dx12 = game.api == "DirectX 12";
                                                                let poster_opt = game.poster.as_ref().map(|p| crate::core::steamart::normalize_art_uri(p));
                                                                let is_dlss5 = game.is_dlss5_patched();
                                                                let launch_tooltip = if is_dlss5 {
                                                                    crate::core::i18n::t_params(&current_lang.read(), "tooltip_launch_game_modded", &[&g_name, game.route_display_name()])
                                                                } else {
                                                                    crate::core::i18n::t_param(&current_lang.read(), "tooltip_launch_game_vanilla", &g_name)
                                                                };
                                                                rsx! {
                                                                    article {
                                                                        class: if is_dx12 { "card dx12" } else { "card" },
                                                                        tabindex: "0",
                                                                        onmousedown: move |e| e.stop_propagation(),
                                                                        onclick: move |_| {
                                                                            sheet_game_idx.set(Some(idx));
                                                                            sheet_open.set(true);
                                                                        },
                                                                        div { class: "tools",
                                                                            button {
                                                                                class: "tool cover-tool",
                                                                                title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_change_cover\")}",
                                                                                onclick: {
                                                                                    let dir = g_dir.clone();
                                                                                    move |e: MouseEvent| {
                                                                                        e.stop_propagation();
                                                                                        pick_and_set_cover(&dir, games);
                                                                                    }
                                                                                },
                                                                                svg {
                                                                                    view_box: "0 0 24 24",
                                                                                    style: "width: 13px; height: 13px; fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round;",
                                                                                    rect { x: "3", y: "3", width: "18", height: "18", rx: "2", ry: "2" }
                                                                                    circle { cx: "8.5", cy: "8.5", r: "1.5" }
                                                                                    polyline { points: "21 15 16 10 5 21" }
                                                                                }
                                                                            }
                                                                            button {
                                                                                class: "tool",
                                                                                title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_open_explorer\")}",
                                                                                onclick: {
                                                                                    let dir = g_dir.clone();
                                                                                    move |e: MouseEvent| {
                                                                                        e.stop_propagation();
                                                                                        let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                                                                                    }
                                                                                },
                                                                                "📂"
                                                                            }
                                                                            button {
                                                                                class: "tool",
                                                                                title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_hide_game\")}",
                                                                                onclick: {
                                                                                    let dir = g_dir.clone();
                                                                                    let name = g_name.clone();
                                                                                    move |e: MouseEvent| {
                                                                                        e.stop_propagation();
                                                                                        let mut s = load_state();
                                                                                        s.hide_game(&dir);
                                                                                        let _ = save_state(&s);
                                                                                        let mut cur = games.read().clone();
                                                                                        cur.retain(|g| !s.is_hidden(&g.dir));
                                                                                        games.set(cur);
                                                                                        hidden_count.set(s.hidden.len());
                                                                                        crate::core::logger::info("library", &format!("Hidden game from library: {} ({})", name, dir.display()));
                                                                                    }
                                                                                },
                                                                                "✕"
                                                                            }
                                                                        }
                                                                        button {
                                                                            class: if is_dlss5 { "card-center-play modded" } else { "card-center-play vanilla" },
                                                                            title: "{launch_tooltip}",
                                                                            onclick: {
                                                                                let g = game.clone();
                                                                                move |e: MouseEvent| {
                                                                                    e.stop_propagation();
                                                                                    launch_game(&g);
                                                                                }
                                                                            },
                                                                            svg {
                                                                                view_box: "0 0 24 24",
                                                                                polygon { points: "6,4 20,12 6,20" }
                                                                            }
                                                                            span { "{crate::core::i18n::t(&current_lang.read(), \"home_play\")}" }
                                                                        }
                                                                        span { class: if is_dx12 { "badge dx12" } else { "badge" }, "{g_api}" }
                                                                        div { class: "poster",
                                                                            if let Some(ref p_url) = poster_opt {
                                                                                img { src: "{p_url}", alt: "{g_name}" }
                                                                            } else {
                                                                                div { class: "placeholder", "{&g_name[0..2.min(g_name.len())]}" }
                                                                            }
                                                                            div { class: "status",
                                                                                span { class: if has_dlss { "dot-s on" } else { "dot-s" } }
                                                                                "{dlss_label}"
                                                                                span { class: if g_opti || has_addon { "dot-s on" } else { "dot-s" }, style: "margin-inline-start:8px" }
                                                                                if g_opti { "OptiScaler" } else { "add-on" }
                                                                            }
                                                                        }
                                                                        div { class: "name", "{g_name}" }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                // Flat alphabetical sort
                                div { class: "grid",
                                    for (orig_idx, game) in visible_games.iter() {
                                        {
                                            let idx = *orig_idx;
                                            let g_name = game.name.clone();
                                            let g_api = game.api.clone();
                                            let g_dir = game.dir.clone();
                                            let has_dlss = game.dlss_version.is_some();
                                            let dlss_label = if let Some(ref v) = game.dlss_version {
                                                crate::core::scan::short_version(v)
                                            } else {
                                                crate::core::i18n::t(&current_lang.read(), "badge_no_dlss").to_string()
                                            };
                                            let g_opti = game.optiscaler_installed;
                                            let has_addon = game.mfg_unlock_installed || game.has_backup;
                                            let is_dx12 = game.api == "DirectX 12";
                                            let poster_opt = game.poster.as_ref().map(|p| crate::core::steamart::normalize_art_uri(p));
                                            let is_dlss5 = game.is_dlss5_patched();
                                            let launch_tooltip = if is_dlss5 {
                                                crate::core::i18n::t_params(&current_lang.read(), "tooltip_launch_game_modded", &[&g_name, game.route_display_name()])
                                            } else {
                                                crate::core::i18n::t_param(&current_lang.read(), "tooltip_launch_game_vanilla", &g_name)
                                            };
                                            rsx! {
                                                article {
                                                    class: if is_dx12 { "card dx12" } else { "card" },
                                                    tabindex: "0",
                                                    onmousedown: move |e| e.stop_propagation(),
                                                    onclick: move |_| {
                                                        sheet_game_idx.set(Some(idx));
                                                        sheet_open.set(true);
                                                    },
                                                    div { class: "tools",
                                                        button {
                                                            class: "tool cover-tool",
                                                            title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_change_cover\")}",
                                                            onclick: {
                                                                let dir = g_dir.clone();
                                                                move |e: MouseEvent| {
                                                                    e.stop_propagation();
                                                                    pick_and_set_cover(&dir, games);
                                                                }
                                                            },
                                                            svg {
                                                                view_box: "0 0 24 24",
                                                                style: "width: 13px; height: 13px; fill: none; stroke: currentColor; stroke-width: 2; stroke-linecap: round; stroke-linejoin: round;",
                                                                rect { x: "3", y: "3", width: "18", height: "18", rx: "2", ry: "2" }
                                                                circle { cx: "8.5", cy: "8.5", r: "1.5" }
                                                                polyline { points: "21 15 16 10 5 21" }
                                                            }
                                                        }
                                                        button {
                                                            class: "tool",
                                                            title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_open_explorer\")}",
                                                            onclick: {
                                                                let dir = g_dir.clone();
                                                                move |e: MouseEvent| {
                                                                    e.stop_propagation();
                                                                    let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                                                                }
                                                            },
                                                            "📂"
                                                        }
                                                        button {
                                                            class: "tool",
                                                            title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_hide_game\")}",
                                                            onclick: {
                                                                let dir = g_dir.clone();
                                                                let name = g_name.clone();
                                                                move |e: MouseEvent| {
                                                                    e.stop_propagation();
                                                                    let mut s = load_state();
                                                                    s.hide_game(&dir);
                                                                    let _ = save_state(&s);
                                                                    let mut cur = games.read().clone();
                                                                    cur.retain(|g| !s.is_hidden(&g.dir));
                                                                    games.set(cur);
                                                                    hidden_count.set(s.hidden.len());
                                                                    crate::core::logger::info("library", &format!("Hidden game from library: {} ({})", name, dir.display()));
                                                                }
                                                            },
                                                            "✕"
                                                        }
                                                    }
                                                    button {
                                                        class: if is_dlss5 { "card-center-play modded" } else { "card-center-play vanilla" },
                                                        title: "{launch_tooltip}",
                                                        onclick: {
                                                            let g = game.clone();
                                                            move |e: MouseEvent| {
                                                                e.stop_propagation();
                                                                launch_game(&g);
                                                            }
                                                        },
                                                        svg {
                                                            view_box: "0 0 24 24",
                                                            polygon { points: "6,4 20,12 6,20" }
                                                        }
                                                        span { "{crate::core::i18n::t(&current_lang.read(), \"home_play\")}" }
                                                    }
                                                    span { class: if is_dx12 { "badge dx12" } else { "badge" }, "{g_api}" }
                                                    div { class: "poster",
                                                        if let Some(ref p_url) = poster_opt {
                                                            img { src: "{p_url}", alt: "{g_name}" }
                                                        } else {
                                                            div { class: "placeholder", "{&g_name[0..2.min(g_name.len())]}" }
                                                        }
                                                        div { class: "status",
                                                            span { class: if has_dlss { "dot-s on" } else { "dot-s" } }
                                                            "{dlss_label}"
                                                            span { class: if g_opti || has_addon { "dot-s on" } else { "dot-s" }, style: "margin-inline-start:8px" }
                                                            if g_opti { "OptiScaler" } else { "add-on" }
                                                        }
                                                    }
                                                    div { class: "name", "{g_name}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ---------------- ADD-ONS VIEW ----------------
                if *active_view.read() == "addons" {
                    section {
                        class: "view active",
                        id: "view-addons",
                        style: "cursor: default;",
                        div { class: "section-head",
                            h3 { "{crate::core::i18n::t(&current_lang.read(), \"nav_addons\")}" }
                            button {
                                class: "ghost sm",
                                id: "addonAdd",
                                onclick: move |_| {
                                    spawn(async move {
                                        if let Some(handle) = rfd::AsyncFileDialog::new()
                                            .set_title("Add an add-on build")
                                            .add_filter("ReShade add-on", &["addon64", "addon"])
                                            .pick_file()
                                            .await
                                        {
                                            let file = handle.path().to_path_buf();
                                            let fname = file.file_name().and_then(|n| n.to_str()).unwrap_or("addon").to_string();
                                            let size = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
                                            let mb_size = format!("{:.2} MB", size as f64 / (1024.0 * 1024.0));
                                            let pe_ver = crate::core::pe::inspect_pe(&file).and_then(|p| p.version);
                                            let info_str = if let Some(ref v) = pe_ver {
                                                format!("{} · {} · {}", fname, v, mb_size)
                                            } else {
                                                format!("{} · {}", fname, mb_size)
                                            };
                                            let suggested_name = file.file_stem().and_then(|s| s.to_str()).unwrap_or(&fname).to_string();

                                            dlg_path.set(file.to_string_lossy().to_string());
                                            dlg_file_info.set(info_str);
                                            dlg_name.set(suggested_name);
                                            dlg_desc.set(String::new());
                                            dlg_tag.set(String::new());
                                            show_addon_dlg.set(true);
                                        }
                                    });
                                },
                                "{crate::core::i18n::t(&current_lang.read(), \"btn_add\")}"
                            }
                        }
                        p { class: "hint", "{crate::core::i18n::t(&current_lang.read(), \"addons_hint\")}" }
                        if let Some(ref prog) = *component_download_progress.read() {
                            div { style: "margin-bottom: 16px; padding: 12px 16px; display: flex; flex-direction: column; gap: 8px; border: 1px solid rgba(249, 115, 22, 0.4); border-radius: 8px; background: rgba(249, 115, 22, 0.12);",
                                div { style: "display: flex; justify-content: space-between; align-items: center;",
                                    div { style: "font-size: 0.88rem; color: var(--text-primary); font-weight: 500;", "⚡ {crate::core::downloader::format_download_progress(&current_lang.read(), prog)}" }
                                    if matches!(*status_state.read(), AppStatus::DownloadingComponents) {
                                        div { style: "font-size: 0.82rem; color: var(--accent); font-weight: 600;", "{status_percent.read():.0}%" }
                                    }
                                }
                                if matches!(*status_state.read(), AppStatus::DownloadingComponents) {
                                    div { class: "bar", style: "margin-top: 0; height: 4px;",
                                        i { style: "width: {status_percent.read()}%;" }
                                    }
                                }
                            }
                        }
                        div { class: "addons", id: "addonList",
                            // 1. Mandatory Core: RenoDX v4.7
                            div { class: if crate::core::downloader::is_renodx_engine_cached() { "addon on" } else { "addon" },
                                div { class: "mark",
                                    if crate::core::downloader::is_renodx_engine_cached() {
                                        svg { style: "width:18px; height:18px; fill:currentColor;", view_box: "0 0 24 24",
                                            path { d: "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" }
                                        }
                                    }
                                }
                                div { class: "body",
                                    div { class: "t",
                                        "RenoDX v4.7 (Full ReShade Add-on Engine)"
                                        span { class: "tag accent", "{crate::core::i18n::t(&current_lang.read(), \"tag_mandatory_core\")}" }
                                    }
                                    div { class: "d", "renodx-dlss5.addon64 · {crate::core::i18n::t(&current_lang.read(), \"meta_sha256_verified\")} · 1.65 MB" }
                                    div { class: "dim", style: "font-size:0.75em; margin-top:3px; opacity:0.75; font-family:monospace; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"label_source\")} {crate::core::downloader::RENODX_DLSS5_URL}"
                                    }
                                    div { class: "dim", style: "font-size:0.82em; margin-top:4px;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"addon_renodx_desc\")}"
                                    }
                                }
                                div { class: "addon-status-core", style: "display:flex; align-items:center; gap:8px;",
                                    if crate::core::downloader::is_renodx_engine_cached() {
                                        span { class: "tag accent", style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px;",
                                            "✓ {crate::core::i18n::t(&current_lang.read(), \"addon_core_badge\")}"
                                        }
                                    } else {
                                        button {
                                            class: "tag warn",
                                            style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px; cursor: pointer; border: none; background: rgba(245, 158, 11, 0.2); color: #f59e0b;",
                                            title: crate::core::i18n::t(&current_lang.read(), "tooltip_retry_download"),
                                            onclick: move |_| {
                                                *trigger_component_download.write() += 1;
                                            },
                                            if matches!(*status_state.read(), AppStatus::DownloadingComponents) {
                                                "{status_percent.read():.0}% ..."
                                            } else {
                                                "⚡ {crate::core::i18n::t(&current_lang.read(), \"btn_retry_download\")}"
                                            }
                                        }
                                    }
                                }
                            }

                            // 2. NIGos DLSS 5 D3D12 Mip Fix v2.6.1
                            div { class: if crate::core::downloader::is_dlss5_d3d12_fix_cached() { "addon on" } else { "addon" },
                                div { class: "mark",
                                    if crate::core::downloader::is_dlss5_d3d12_fix_cached() {
                                        svg { style: "width:18px; height:18px; fill:currentColor;", view_box: "0 0 24 24",
                                            path { d: "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" }
                                        }
                                    }
                                }
                                div { class: "body",
                                    div { class: "t",
                                        "DLSS Studio D3D12 Mip Companion"
                                        span { class: "tag accent", "v1.0.0" }
                                    }
                                    div { class: "d", "dlss-mip-fix.addon64 · Pure Rust Add-on · 147 KB" }
                                    div { class: "dim", style: "font-size:0.75em; margin-top:3px; opacity:0.75; font-family:monospace; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"label_source\")} {crate::core::downloader::DLSS_MIP_FIX_URL}"
                                    }
                                    div { class: "dim", style: "font-size:0.82em; margin-top:4px;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"addon_dlss5_d3d12_fix_desc\")}"
                                    }
                                }
                                div { class: "addon-status-core", style: "display:flex; align-items:center; gap:8px;",
                                    if crate::core::downloader::is_dlss5_d3d12_fix_cached() {
                                        span { class: "tag accent", style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px;",
                                            "✓ {crate::core::i18n::t(&current_lang.read(), \"addon_core_badge\")}"
                                        }
                                    } else {
                                        button {
                                            class: "tag warn",
                                            style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px; cursor: pointer; border: none; background: rgba(245, 158, 11, 0.2); color: #f59e0b;",
                                            title: crate::core::i18n::t(&current_lang.read(), "tooltip_retry_download"),
                                            onclick: move |_| {
                                                *trigger_component_download.write() += 1;
                                            },
                                            if matches!(*status_state.read(), AppStatus::DownloadingComponents) {
                                                "{status_percent.read():.0}% ..."
                                            } else {
                                                "⚡ {crate::core::i18n::t(&current_lang.read(), \"btn_retry_download\")}"
                                            }
                                        }
                                    }
                                }
                            }

                            // 3. Mandatory Core: RenoDX 4x MFG Unlock v1.0
                            div { class: if crate::core::downloader::is_mfg_addon_cached() { "addon on" } else { "addon" },
                                div { class: "mark",
                                    if crate::core::downloader::is_mfg_addon_cached() {
                                        svg { style: "width:18px; height:18px; fill:currentColor;", view_box: "0 0 24 24",
                                            path { d: "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" }
                                        }
                                    }
                                }
                                div { class: "body",
                                    div { class: "t",
                                        "RenoDX 4x MFG Unlock v1.0"
                                        span { class: "tag warn", "{crate::core::i18n::t(&current_lang.read(), \"tag_rtx40\")}" }
                                    }
                                    div { class: "d", "renodx-mfgunlock.addon64 · v1.0 · {crate::core::i18n::t(&current_lang.read(), \"meta_sha256_verified\")} · 528 KB" }
                                    div { class: "dim", style: "font-size:0.75em; margin-top:3px; opacity:0.75; font-family:monospace; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"label_source\")} {crate::core::downloader::MFG_10_URL}"
                                    }
                                    div { class: "dim", style: "font-size:0.82em; margin-top:4px;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"addon_mfg_desc\")}"
                                    }
                                }
                                div { class: "addon-status-core", style: "display:flex; align-items:center; gap:8px;",
                                    if crate::core::downloader::is_mfg_addon_cached() {
                                        span { class: "tag accent", style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px;",
                                            "✓ {crate::core::i18n::t(&current_lang.read(), \"addon_core_badge\")}"
                                        }
                                    } else {
                                        button {
                                            class: "tag warn",
                                            style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px; cursor: pointer; border: none; background: rgba(245, 158, 11, 0.2); color: #f59e0b;",
                                            title: crate::core::i18n::t(&current_lang.read(), "tooltip_retry_download"),
                                            onclick: move |_| {
                                                *trigger_component_download.write() += 1;
                                            },
                                            if matches!(*status_state.read(), AppStatus::DownloadingComponents) {
                                                "{status_percent.read():.0}% ..."
                                            } else {
                                                "⚡ {crate::core::i18n::t(&current_lang.read(), \"btn_retry_download\")}"
                                            }
                                        }
                                    }
                                }
                            }

                            // 3. DLSS 5 Feeder & Motion Shaders
                            div { class: if crate::core::downloader::is_feeder_cached() { "addon on" } else { "addon" },
                                div { class: "mark",
                                    if crate::core::downloader::is_feeder_cached() {
                                        svg { style: "width:18px; height:18px; fill:currentColor;", view_box: "0 0 24 24",
                                            path { d: "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" }
                                        }
                                    }
                                }
                                div { class: "body",
                                    div { class: "t",
                                        "DLSS 5 Feeder (Neural Pipeline Interceptor)"
                                        span { class: "tag", "{crate::core::i18n::t(&current_lang.read(), \"tag_universal_intercept\")}" }
                                    }
                                    div { class: "d", "dlss5-feed.addon64 + DLSS5_Feed.fx + vort_Motion.fx · {crate::core::i18n::t(&current_lang.read(), \"meta_sha256_verified\")} · 4.8 MB" }
                                    div { class: "dim", style: "font-size:0.75em; margin-top:3px; opacity:0.75; font-family:monospace; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"label_source\")} {crate::core::downloader::FEEDER_ARCHIVE_URL}"
                                    }
                                    div { class: "dim", style: "font-size:0.82em; margin-top:4px;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"addon_feeder_desc\")}"
                                    }
                                }
                                div { class: "addon-status-core", style: "display:flex; align-items:center; gap:8px;",
                                    if crate::core::downloader::is_feeder_cached() {
                                        span { class: "tag accent", style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px;",
                                            "✓ {crate::core::i18n::t(&current_lang.read(), \"addon_core_badge\")}"
                                        }
                                    } else {
                                        button {
                                            class: "tag warn",
                                            style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px; cursor: pointer; border: none; background: rgba(245, 158, 11, 0.2); color: #f59e0b;",
                                            title: crate::core::i18n::t(&current_lang.read(), "tooltip_retry_download"),
                                            onclick: move |_| {
                                                *trigger_component_download.write() += 1;
                                            },
                                            if matches!(*status_state.read(), AppStatus::DownloadingComponents) {
                                                "{status_percent.read():.0}% ..."
                                            } else {
                                                "⚡ {crate::core::i18n::t(&current_lang.read(), \"btn_retry_download\")}"
                                            }
                                        }
                                    }
                                }
                            }

                            // 4. Mandatory Core: OptiScaler DLSS-NR
                            div { class: if crate::core::downloader::is_optiscaler_cached() { "addon on" } else { "addon" },
                                div { class: "mark",
                                    if crate::core::downloader::is_optiscaler_cached() {
                                        svg { style: "width:18px; height:18px; fill:currentColor;", view_box: "0 0 24 24",
                                            path { d: "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" }
                                        }
                                    }
                                }
                                div { class: "body",
                                    div { class: "t",
                                        "OptiScaler DLSS-NR (Neural Reconstruction & Pre-SR)"
                                        span { class: "tag", "{crate::core::i18n::t(&current_lang.read(), \"tag_neural_reconstruction\")}" }
                                    }
                                    div { class: "d", "OptiScaler.dll + nvngx.ini · v0.8.4 · {crate::core::i18n::t(&current_lang.read(), \"meta_native_backend\")} · 18.4 MB" }
                                    div { class: "dim", style: "font-size:0.75em; margin-top:3px; opacity:0.75; font-family:monospace; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"label_source\")} {crate::core::downloader::OPTISCALER_084_URL}"
                                    }
                                    div { class: "dim", style: "font-size:0.82em; margin-top:4px;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"addon_optiscaler_desc\")}"
                                    }
                                }
                                div { class: "addon-status-core", style: "display:flex; align-items:center; gap:8px;",
                                    if crate::core::downloader::is_optiscaler_cached() {
                                        span { class: "tag accent", style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px;",
                                            "✓ {crate::core::i18n::t(&current_lang.read(), \"addon_core_badge\")}"
                                        }
                                    } else {
                                        button {
                                            class: "tag warn",
                                            style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px; cursor: pointer; border: none; background: rgba(245, 158, 11, 0.2); color: #f59e0b;",
                                            title: crate::core::i18n::t(&current_lang.read(), "tooltip_retry_download"),
                                            onclick: move |_| {
                                                *trigger_component_download.write() += 1;
                                            },
                                            if matches!(*status_state.read(), AppStatus::DownloadingComponents) {
                                                "{status_percent.read():.0}% ..."
                                            } else {
                                                "⚡ {crate::core::i18n::t(&current_lang.read(), \"btn_retry_download\")}"
                                            }
                                        }
                                    }
                                }
                            }

                            // 5. Mandatory Core: ReShade 6.8.0 Add-on Runtime
                            div { class: if crate::core::downloader::is_reshade_cached() { "addon on" } else { "addon" },
                                div { class: "mark",
                                    if crate::core::downloader::is_reshade_cached() {
                                        svg { style: "width:18px; height:18px; fill:currentColor;", view_box: "0 0 24 24",
                                            path { d: "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" }
                                        }
                                    }
                                }
                                div { class: "body",
                                    div { class: "t",
                                        "ReShade 6.8.0 (Add-on Host Runtime)"
                                        span { class: "tag", "{crate::core::i18n::t(&current_lang.read(), \"tag_graphics_hook\")}" }
                                    }
                                    div { class: "d", "ReShade64.dll · v6.8.0 · {crate::core::i18n::t(&current_lang.read(), \"meta_full_addon_support\")} · 5.3 MB" }
                                    div { class: "dim", style: "font-size:0.75em; margin-top:3px; opacity:0.75; font-family:monospace; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"label_source\")} {crate::core::downloader::RESHADE_SETUP_URL}"
                                    }
                                    div { class: "dim", style: "font-size:0.82em; margin-top:4px;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"addon_reshade_desc\")}"
                                    }
                                }
                                div { class: "addon-status-core", style: "display:flex; align-items:center; gap:8px;",
                                    if crate::core::downloader::is_reshade_cached() {
                                        span { class: "tag accent", style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px;",
                                            "✓ {crate::core::i18n::t(&current_lang.read(), \"addon_core_badge\")}"
                                        }
                                    } else {
                                        button {
                                            class: "tag warn",
                                            style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px; cursor: pointer; border: none; background: rgba(245, 158, 11, 0.2); color: #f59e0b;",
                                            title: crate::core::i18n::t(&current_lang.read(), "tooltip_retry_download"),
                                            onclick: move |_| {
                                                *trigger_component_download.write() += 1;
                                            },
                                            if matches!(*status_state.read(), AppStatus::DownloadingComponents) {
                                                "{status_percent.read():.0}% ..."
                                            } else {
                                                "⚡ {crate::core::i18n::t(&current_lang.read(), \"btn_retry_download\")}"
                                            }
                                        }
                                    }
                                }
                            }

                            // 6. Mandatory Core: NVIDIA Streamline Runtime v2.14.1
                            div { class: if crate::core::downloader::is_streamline_cached() { "addon on" } else { "addon" },
                                div { class: "mark",
                                    if crate::core::downloader::is_streamline_cached() {
                                        svg { style: "width:18px; height:18px; fill:currentColor;", view_box: "0 0 24 24",
                                            path { d: "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" }
                                        }
                                    }
                                }
                                div { class: "body",
                                    div { class: "t",
                                        "NVIDIA Streamline Runtime v2.14.1"
                                        span { class: "tag", "{crate::core::i18n::t(&current_lang.read(), \"tag_interposer\")}" }
                                    }
                                    div { class: "d", "sl.interposer.dll · v2.14.1 · {crate::core::i18n::t(&current_lang.read(), \"meta_streamline_interposer\")} · 1.2 MB" }
                                    div { class: "dim", style: "font-size:0.75em; margin-top:3px; opacity:0.75; font-family:monospace; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"label_source\")} {crate::core::downloader::STREAMLINE_ZIP_URL}"
                                    }
                                    div { class: "dim", style: "font-size:0.82em; margin-top:4px;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"addon_streamline_desc\")}"
                                    }
                                }
                                div { class: "addon-status-core", style: "display:flex; align-items:center; gap:8px;",
                                    if crate::core::downloader::is_streamline_cached() {
                                        span { class: "tag accent", style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px;",
                                            "✓ {crate::core::i18n::t(&current_lang.read(), \"addon_core_badge\")}"
                                        }
                                    } else {
                                        button {
                                            class: "tag warn",
                                            style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px; cursor: pointer; border: none; background: rgba(245, 158, 11, 0.2); color: #f59e0b;",
                                            title: crate::core::i18n::t(&current_lang.read(), "tooltip_retry_download"),
                                            onclick: move |_| {
                                                *trigger_component_download.write() += 1;
                                            },
                                            if matches!(*status_state.read(), AppStatus::DownloadingComponents) {
                                                "{status_percent.read():.0}% ..."
                                            } else {
                                                "⚡ {crate::core::i18n::t(&current_lang.read(), \"btn_retry_download\")}"
                                            }
                                        }
                                    }
                                }
                            }

                            // 7. dgVoodoo2 Legacy DirectX Wrapper v2.87.5
                            div { class: if crate::core::downloader::is_dgvoodoo_cached() { "addon on" } else { "addon" },
                                div { class: "mark",
                                    if crate::core::downloader::is_dgvoodoo_cached() {
                                        svg { style: "width:18px; height:18px; fill:currentColor;", view_box: "0 0 24 24",
                                            path { d: "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" }
                                        }
                                    }
                                }
                                div { class: "body",
                                    div { class: "t",
                                        "dgVoodoo2 (Legacy DirectX Wrapper)"
                                        span { class: "tag", "{crate::core::i18n::t(&current_lang.read(), \"tag_legacy_wrapper\")}" }
                                    }
                                    div { class: "d", "d3d9.dll / d3d8.dll · v2.87.5 · D3D -> D3D11 Translation · 1.4 MB" }
                                    div { class: "dim", style: "font-size:0.75em; margin-top:3px; opacity:0.75; font-family:monospace; overflow:hidden; text-overflow:ellipsis; white-space:nowrap;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"label_source\")} {crate::core::downloader::DGVOODOO_URL}"
                                    }
                                    div { class: "dim", style: "font-size:0.82em; margin-top:4px;",
                                        "{crate::core::i18n::t(&current_lang.read(), \"addon_dgvoodoo_desc\")}"
                                    }
                                }
                                div { class: "addon-status-core", style: "display:flex; align-items:center; gap:8px;",
                                    if crate::core::downloader::is_dgvoodoo_cached() {
                                        span { class: "tag accent", style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px;",
                                            "✓ {crate::core::i18n::t(&current_lang.read(), \"addon_core_badge\")}"
                                        }
                                    } else {
                                        button {
                                            class: "tag warn",
                                            style: "font-weight:600; font-size:0.75rem; padding: 4px 10px; border-radius: 6px; letter-spacing: 0.5px; cursor: pointer; border: none; background: rgba(245, 158, 11, 0.2); color: #f59e0b;",
                                            title: crate::core::i18n::t(&current_lang.read(), "tooltip_retry_download"),
                                            onclick: move |_| {
                                                *trigger_component_download.write() += 1;
                                            },
                                            if matches!(*status_state.read(), AppStatus::DownloadingComponents) {
                                                "{status_percent.read():.0}% ..."
                                            } else {
                                                "⚡ {crate::core::i18n::t(&current_lang.read(), \"btn_retry_download\")}"
                                            }
                                        }
                                    }
                                }
                            }

                            // 8. Custom user-added add-ons
                            for custom in custom_addons.read().clone().into_iter() {
                                {
                                    let path_str = custom.path.clone();
                                    let is_act = addons_active.read().contains(&path_str);
                                    let name_display = custom.name.clone().unwrap_or_else(|| {
                                        std::path::Path::new(&path_str).file_name().and_then(|n| n.to_str()).unwrap_or("Custom Add-on").to_string()
                                    });
                                    let fname = std::path::Path::new(&path_str).file_name().and_then(|n| n.to_str()).unwrap_or("addon").to_string();
                                    let tag_display = custom.tag.clone();
                                    let desc_display = custom.description.clone();
                                    rsx! {
                                        div { key: "{path_str}", class: if is_act { "addon on" } else { "addon" },
                                            div { class: "mark",
                                                if is_act {
                                                    svg { style: "width:18px; height:18px; fill:currentColor;", view_box: "0 0 24 24",
                                                        path { d: "M9 16.17L4.83 12l-1.42 1.41L9 19 21 7l-1.41-1.41z" }
                                                    }
                                                }
                                            }
                                            div { class: "body",
                                                div { class: "t",
                                                    "{name_display}"
                                                    if let Some(ref t) = tag_display {
                                                        span { class: "tag", "{t}" }
                                                    } else {
                                                        span { class: "tag", "{crate::core::i18n::t(&current_lang.read(), \"tag_custom\")}" }
                                                    }
                                                }
                                                div { class: "d", "{fname} · {path_str}" }
                                                if let Some(ref d) = desc_display {
                                                    div { class: "dim", style: "font-size:0.82em; margin-top:4px;", "{d}" }
                                                }
                                            }
                                            button {
                                                class: "toggle",
                                                role: "switch",
                                                aria_checked: if is_act { "true" } else { "false" },
                                                title: if is_act { crate::core::i18n::t(&current_lang.read(), "tooltip_deactivate_addon") } else { crate::core::i18n::t(&current_lang.read(), "tooltip_activate_addon") },
                                                onclick: {
                                                    let p = path_str.clone();
                                                    move |_| {
                                                        let mut cur = addons_active.read().clone();
                                                        let was_on = cur.contains(&p);
                                                        if was_on {
                                                            cur.retain(|x| x != &p);
                                                        } else {
                                                            cur.push(p.clone());
                                                        }
                                                        addons_active.set(cur.clone());
                                                        let mut s = crate::core::state::load_state();
                                                        s.addons = cur;
                                                        let _ = crate::core::state::save_state(&s);
                                                        log_message(&format!("@{{log_addon_status|{}|@{}}}", p, if was_on { "log_addon_deactivated" } else { "log_addon_activated" }));
                                                        activity_log.set(get_session_log());
                                                    }
                                                },
                                                span { class: "knob" }
                                            }
                                            button {
                                                class: "drop",
                                                title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_remove_addon\")}",
                                                onclick: {
                                                    let p = path_str.clone();
                                                    move |_| {
                                                        let mut list = custom_addons.read().clone();
                                                        list.retain(|x| x.path != p);
                                                        custom_addons.set(list.clone());
                                                        let mut cur_act = addons_active.read().clone();
                                                        cur_act.retain(|x| x != &p);
                                                        addons_active.set(cur_act.clone());
                                                        let mut s = crate::core::state::load_state();
                                                        s.addon_files = list;
                                                        s.addons = cur_act;
                                                        let _ = crate::core::state::save_state(&s);
                                                        copy_toast_text.set(crate::core::i18n::t(&current_lang.read(), "toast_addon_removed").to_string());
                                                        *toast_generation.write() += 1;
                                                        copy_toast.set(true);
                                                    }
                                                },
                                                svg { style: "width:15px; height:15px; fill:currentColor;", view_box: "0 0 24 24",
                                                    path { d: "M6 19c0 1.1.9 2 2 2h8c1.1 0 2-.9 2-2V7H6v12zM19 4h-3.5l-1-1h-5l-1 1H5v2h14V4z" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Add-on Dialog Modal
                        if *show_addon_dlg.read() {
                            div {
                                class: "overlay",
                                id: "dlgOverlay",
                                onmousedown: move |e| e.stop_propagation(),
                                onclick: move |_| show_addon_dlg.set(false),
                                div {
                                    class: "dialog",
                                    onmousedown: move |e| e.stop_propagation(),
                                    onclick: move |e| e.stop_propagation(),
                                    h3 { "{crate::core::i18n::t(&current_lang.read(), \"dlg_add_addon_title\")}" }
                                    div { class: "dlg-file", id: "dlgFile", "{dlg_file_info.read()}" }
                                    label { class: "dlg-label", "{crate::core::i18n::t(&current_lang.read(), \"dlg_field_name\")}" }
                                    input {
                                        class: "dlg-input",
                                        id: "dlgName",
                                        r#type: "text",
                                        value: "{dlg_name.read()}",
                                        oninput: move |e| dlg_name.set(e.value())
                                    }
                                    label { class: "dlg-label", "{crate::core::i18n::t(&current_lang.read(), \"dlg_field_desc\")}" }
                                    textarea {
                                        class: "dlg-input",
                                        id: "dlgDesc",
                                        rows: "3",
                                        value: "{dlg_desc.read()}",
                                        oninput: move |e| dlg_desc.set(e.value())
                                    }
                                    div { class: "dlg-hint", "{crate::core::i18n::t(&current_lang.read(), \"dlg_hint_notes\")}" }
                                    label { class: "dlg-label", "{crate::core::i18n::t(&current_lang.read(), \"dlg_field_tag\")}" }
                                    input {
                                        class: "dlg-input",
                                        id: "dlgTag",
                                        r#type: "text",
                                        placeholder: "{crate::core::i18n::t(&current_lang.read(), \"dlg_placeholder_tag\")}",
                                        value: "{dlg_tag.read()}",
                                        oninput: move |e| dlg_tag.set(e.value())
                                    }
                                    div { class: "dlg-actions",
                                        button {
                                            class: "ghost sm",
                                            id: "dlgCancel",
                                            onclick: move |_| show_addon_dlg.set(false),
                                            "{crate::core::i18n::t(&current_lang.read(), \"btn_cancel\")}"
                                        }
                                        button {
                                            class: "primary sm",
                                            id: "dlgSave",
                                            onclick: move |_| {
                                                let p_val = dlg_path.read().clone();
                                                let n_val = dlg_name.read().trim().to_string();
                                                let d_val = dlg_desc.read().trim().to_string();
                                                let t_val = dlg_tag.read().trim().to_string();

                                                let entry = crate::core::state::AddonFileEntry {
                                                    path: p_val.clone(),
                                                    name: if n_val.is_empty() { None } else { Some(n_val) },
                                                    description: if d_val.is_empty() { None } else { Some(d_val) },
                                                    tag: if t_val.is_empty() { None } else { Some(t_val) },
                                                };

                                                let mut list = custom_addons.read().clone();
                                                list.retain(|x| x.path != p_val);
                                                list.push(entry);
                                                custom_addons.set(list.clone());

                                                let mut cur_act = addons_active.read().clone();
                                                if !cur_act.contains(&p_val) {
                                                    cur_act.push(p_val.clone());
                                                }
                                                addons_active.set(cur_act.clone());

                                                let mut s = crate::core::state::load_state();
                                                s.addon_files = list;
                                                s.addons = cur_act;
                                                let _ = crate::core::state::save_state(&s);

                                                show_addon_dlg.set(false);
                                                copy_toast_text.set(crate::core::i18n::t(&current_lang.read(), "toast_addon_registered").to_string());
                                                *toast_generation.write() += 1;
                                                copy_toast.set(true);
                                                log_message(&format!("@{{log_addon_imported|{}}}", p_val));
                                                activity_log.set(get_session_log());
                                            },
                                            "{crate::core::i18n::t(&current_lang.read(), \"btn_save\")}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ---------------- HISTORY VIEW ----------------
                if *active_view.read() == "history" {
                    section {
                        class: "view active",
                        id: "view-history",
                        style: "cursor: default;",
                        div { class: "section-head",
                            h3 { "{crate::core::i18n::t(&current_lang.read(), \"nav_history\")}" }
                            button {
                                class: "ghost sm",
                                id: "copyHistory",
                                onclick: move |_| {
                                    let g_list = games.read().clone();
                                    let header_date = crate::core::i18n::t(&current_lang.read(), "col_date");
                                    let header_game = crate::core::i18n::t(&current_lang.read(), "col_game");
                                    let header_action = crate::core::i18n::t(&current_lang.read(), "col_action");
                                    let header_files = crate::core::i18n::t(&current_lang.read(), "col_replaced");
                                    let header_path = crate::core::i18n::t(&current_lang.read(), "col_path");
                                    let mut out = format!("{}\t{}\t{}\t{}\t{}\r\n", header_date, header_game, header_action, header_files, header_path);
                                    for r in history_rows.read().iter().rev() {
                                        let title = resolve_game_title(r, &g_list);
                                        let changes = if r.action == "restore" {
                                            crate::core::i18n::t(&current_lang.read(), "history_orig_restored").to_string()
                                        } else if r.action == "clean" {
                                            crate::core::i18n::t(&current_lang.read(), "history_mods_cleaned").to_string()
                                        } else {
                                            crate::core::i18n::t(&current_lang.read(), "history_changes_format")
                                                .replace("{replaced}", &r.replaced.to_string())
                                                .replace("{added}", &r.added.to_string())
                                        };
                                        let action_str = match r.action.as_str() {
                                            "install_native" => crate::core::i18n::t(&current_lang.read(), "history_action_install_native"),
                                            "install_feeder" => crate::core::i18n::t(&current_lang.read(), "history_action_install_feeder"),
                                            "install_optiscaler" => crate::core::i18n::t(&current_lang.read(), "history_action_install_optiscaler"),
                                            "install" => crate::core::i18n::t(&current_lang.read(), "history_action_install"),
                                            "restore" => crate::core::i18n::t(&current_lang.read(), "history_action_restore"),
                                            "clean" => crate::core::i18n::t(&current_lang.read(), "history_action_clean"),
                                            _ => r.action.as_str(),
                                        };
                                        out.push_str(&format!("{}\t{}\t{}\t{}\t{}\r\n", r.date, title, action_str, changes, r.dir));
                                    }
                                    copy_to_clipboard(&out);
                                    copy_toast_text.set(crate::core::i18n::t(&current_lang.read(), "toast_history_copied").to_string());
                                    *toast_generation.write() += 1;
                                    copy_toast.set(true);
                                },
                                "{crate::core::i18n::t(&current_lang.read(), \"btn_copy_history\")}"
                            }
                        }
                        div { class: "glass pad", id: "history",
                            if history_rows.read().is_empty() {
                                p { class: "dim", "{crate::core::i18n::t(&current_lang.read(), \"history_empty\")}" }
                            } else {
                                div { class: "history-table-container",
                                    table { class: "history-table",
                                        thead {
                                            tr {
                                                th { class: "col-date", "{crate::core::i18n::t(&current_lang.read(), \"col_date\")}" }
                                                th { class: "col-game", "{crate::core::i18n::t(&current_lang.read(), \"col_game\")}" }
                                                th { class: "col-action", "{crate::core::i18n::t(&current_lang.read(), \"col_action\")}" }
                                                th { class: "col-changes", "{crate::core::i18n::t(&current_lang.read(), \"col_replaced\")}" }
                                                th { class: "col-path", "{crate::core::i18n::t(&current_lang.read(), \"col_path\")}" }
                                            }
                                        }
                                        tbody {
                                            for row in history_rows.read().iter().rev() {
                                                {
                                                    let title = resolve_game_title(row, &games.read());
                                                    let is_install = row.action.starts_with("install");
                                                    let r_date = if row.date.starts_with("SystemTime") {
                                                        crate::core::i18n::t(&current_lang.read(), "history_date_recently")
                                                    } else {
                                                        row.date.as_str()
                                                    };
                                                    let action_display = match row.action.as_str() {
                                                        "install_native" => crate::core::i18n::t(&current_lang.read(), "history_action_install_native"),
                                                        "install_feeder" => crate::core::i18n::t(&current_lang.read(), "history_action_install_feeder"),
                                                        "install_optiscaler" => crate::core::i18n::t(&current_lang.read(), "history_action_install_optiscaler"),
                                                        "install" => crate::core::i18n::t(&current_lang.read(), "history_action_install"),
                                                        "restore" => crate::core::i18n::t(&current_lang.read(), "history_action_restore"),
                                                        "clean" => crate::core::i18n::t(&current_lang.read(), "history_action_clean"),
                                                        _ => row.action.as_str(),
                                                    };
                                                    let changes_label = if row.action == "restore" {
                                                        crate::core::i18n::t(&current_lang.read(), "history_orig_restored").to_string()
                                                    } else if row.action == "clean" {
                                                        crate::core::i18n::t(&current_lang.read(), "history_mods_cleaned").to_string()
                                                    } else {
                                                        crate::core::i18n::t(&current_lang.read(), "history_changes_format")
                                                            .replace("{replaced}", &row.replaced.to_string())
                                                            .replace("{added}", &row.added.to_string())
                                                    };
                                                    rsx! {
                                                        tr {
                                                            td { class: "col-date", "{r_date}" }
                                                            td { class: "col-game", title: "{title}", "{title}" }
                                                            td { class: "col-action",
                                                                span { class: if is_install { "badge dx12" } else { "badge" }, "{action_display}" }
                                                            }
                                                            td { class: "col-changes", "{changes_label}" }
                                                            td { class: "col-path", title: "{row.dir}", "{row.dir}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ---------------- SETTINGS VIEW ----------------
                if *active_view.read() == "settings" {
                    section {
                        class: "view active",
                        id: "view-settings",
                        style: "cursor: default;",
                        div { class: "section-head",
                            h3 { "{crate::core::i18n::t(&current_lang.read(), \"nav_settings\")}" }
                        }
                        div { class: "glass pad", id: "settings",
                            // Group games by store
                            div { class: "set-row", style: "display:flex; justify-content:space-between; align-items:center; padding:12px 0; border-bottom:1px solid var(--line);",
                                div {
                                    div { class: "k", style: "font-weight:600;", "{crate::core::i18n::t(&current_lang.read(), \"settings_scanners\")}" }
                                    div { class: "v", style: "font-size:0.85em; color:var(--dim);", "{crate::core::i18n::t(&current_lang.read(), \"settings_scanners_desc\")}" }
                                }
                                button {
                                    class: "setting-switch",
                                    id: "setGroupGames",
                                    type: "button",
                                    role: "switch",
                                    "aria-checked": if *group_games_by_store.read() { "true" } else { "false" },
                                    onclick: move |_| {
                                        let next = !*group_games_by_store.read();
                                        group_games_by_store.set(next);
                                        let mut s = load_state();
                                        s.group_games_by_store = next;
                                        let _ = save_state(&s);
                                    },
                                    span { class: "knob" }
                                }
                            }

                            // Auto scan fixed drives
                            div { class: "set-row", style: "display:flex; justify-content:space-between; align-items:center; padding:12px 0; border-bottom:1px solid var(--line);",
                                div {
                                    div { class: "k", style: "font-weight:600;", "{crate::core::i18n::t(&current_lang.read(), \"settings_autoscan\")}" }
                                    div { class: "v", style: "font-size:0.85em; color:var(--dim);", "{crate::core::i18n::t(&current_lang.read(), \"settings_autoscan_desc\")}" }
                                }
                                button {
                                    class: "setting-switch",
                                    id: "setAutoScan",
                                    type: "button",
                                    role: "switch",
                                    "aria-checked": if *auto_scan_drives.read() { "true" } else { "false" },
                                    onclick: move |_| {
                                        let next = !*auto_scan_drives.read();
                                        auto_scan_drives.set(next);
                                        let mut s = load_state();
                                        s.auto_scan_drives = next;
                                        let _ = save_state(&s);
                                    },
                                    span { class: "knob" }
                                }
                            }

                            // Enable Rust Theme
                            div { class: "set-row", style: "display:flex; justify-content:space-between; align-items:center; padding:12px 0; border-bottom:1px solid var(--line);",
                                div {
                                    div { class: "k", style: "font-weight:600;", "{crate::core::i18n::t(&current_lang.read(), \"rust_theme_label\")}" }
                                    div { class: "v", style: "font-size:0.85em; color:var(--dim);", "{crate::core::i18n::t(&current_lang.read(), \"rust_theme_desc\")}" }
                                }
                                button {
                                    class: "setting-switch",
                                    id: "setRustTheme",
                                    type: "button",
                                    role: "switch",
                                    "aria-checked": if *rust_theme.read() { "true" } else { "false" },
                                    onclick: move |_| {
                                        let next = !*rust_theme.read();
                                        rust_theme.set(next);
                                        let mut s = load_state();
                                        s.rust_theme = next;
                                        let _ = save_state(&s);
                                        let script = format!("document.documentElement.setAttribute('data-rust-theme', '{}');", if next { "true" } else { "false" });
                                        let _ = dioxus::desktop::window().webview.evaluate_script(&script);
                                    },
                                    span { class: "knob" }
                                }
                            }

                            // Run in background
                            div { class: "set-row", style: "display:flex; justify-content:space-between; align-items:center; padding:12px 0; border-bottom:1px solid var(--line);",
                                div {
                                    div { class: "k", style: "font-weight:600;", "{crate::core::i18n::t(&current_lang.read(), \"tray_minimize_label\")}" }
                                    div { class: "v", style: "font-size:0.85em; color:var(--dim);", "{crate::core::i18n::t(&current_lang.read(), \"tray_minimize_desc\")}" }
                                }
                                button {
                                    class: "setting-switch",
                                    id: "setRunInBg",
                                    type: "button",
                                    role: "switch",
                                    "aria-checked": if *run_in_bg.read() { "true" } else { "false" },
                                    onclick: move |_| {
                                        let next = !*run_in_bg.read();
                                        run_in_bg.set(next);
                                        let mut s = load_state();
                                        s.run_in_background = next;
                                        let _ = save_state(&s);
                                    },
                                    span { class: "knob" }
                                }
                            }

                            // Launch on startup
                            div { class: "set-row", style: "display:flex; justify-content:space-between; align-items:center; padding:12px 0; border-bottom:1px solid var(--line);",
                                div {
                                    div { class: "k", style: "font-weight:600;", "{crate::core::i18n::t(&current_lang.read(), \"startup_boot_label\")}" }
                                    div { class: "v", style: "font-size:0.85em; color:var(--dim);", "{crate::core::i18n::t(&current_lang.read(), \"startup_boot_desc\")}" }
                                }
                                button {
                                    class: "setting-switch",
                                    id: "setStartup",
                                    type: "button",
                                    role: "switch",
                                    "aria-checked": if *launch_at_startup.read() { "true" } else { "false" },
                                    onclick: move |_| {
                                        let next = !*launch_at_startup.read();
                                        let _ = crate::core::tray::set_startup_enabled(next);
                                        launch_at_startup.set(crate::core::tray::is_startup_enabled());
                                    },
                                    span { class: "knob" }
                                }
                            }

                            // Auto-add to Vibepollo / Apollo
                            {
                                let is_vibe_detected = *vibepollo_detected.read();
                                let is_updating = *vibepollo_updating.read();
                                let row_style: &'static str = if is_vibe_detected {
                                    "display:flex; justify-content:space-between; align-items:center; padding:12px 0; border-bottom:1px solid var(--line);"
                                } else {
                                    "display:flex; justify-content:space-between; align-items:center; padding:12px 0; border-bottom:1px solid var(--line); opacity:0.55;"
                                };
                                let btn_style: &'static str = if !is_vibe_detected {
                                    "cursor: not-allowed;"
                                } else if is_updating {
                                    "cursor: wait; opacity: 0.6;"
                                } else {
                                    ""
                                };
                                let is_disabled = !is_vibe_detected || is_updating;
                                let is_checked = if *vibepollo_auto_add.read() { "true" } else { "false" };
                                let vibe_status_tag = if is_vibe_detected {
                                    crate::core::i18n::t(&current_lang.read(), "settings_vibepollo_detected")
                                } else {
                                    crate::core::i18n::t(&current_lang.read(), "settings_vibepollo_not_detected")
                                };
                                let is_uac_target = if let Some(p) = crate::core::vibepollo::get_vibepollo_apps_path() {
                                    crate::core::state::is_path_protected(&p)
                                } else {
                                    true
                                };
                                rsx! {
                                    div {
                                        class: "set-row",
                                        style: "{row_style}",
                                        div {
                                            div {
                                                class: "k",
                                                style: "font-weight:600; display:flex; align-items:center; gap:8px;",
                                                span { "{crate::core::i18n::t(&current_lang.read(), \"settings_vibepollo_label\")}" }
                                                span {
                                                    class: if is_vibe_detected { "badge-tag ok" } else { "badge-tag dim" },
                                                    "{vibe_status_tag}"
                                                }
                                                if is_vibe_detected && is_uac_target {
                                                    span {
                                                        class: "badge-tag warn",
                                                        "🛡️ {crate::core::i18n::t(&current_lang.read(), \"settings_vibe_badge_uac\")}"
                                                    }
                                                }
                                            }
                                            div { class: "v", style: "font-size:0.85em; color:var(--dim);", "{crate::core::i18n::t(&current_lang.read(), \"settings_vibepollo_desc\")}" }
                                        }
                                        button {
                                            class: "setting-switch",
                                            id: "setVibepollo",
                                            type: "button",
                                            role: "switch",
                                            disabled: is_disabled,
                                            style: "{btn_style}",
                                            "aria-checked": is_checked,
                                            onclick: move |_| {
                                                if !is_vibe_detected || *vibepollo_updating.read() { return; }
                                                let next = !*vibepollo_auto_add.read();
                                                vibepollo_updating.set(true);

                                                spawn(async move {
                                                    let res = tokio::task::spawn_blocking(move || {
                                                        if next {
                                                            crate::core::vibepollo::register_app()
                                                        } else {
                                                            crate::core::vibepollo::unregister_app()
                                                        }
                                                    }).await;

                                                    vibepollo_updating.set(false);

                                                    match res {
                                                        Ok(Ok(())) => {
                                                            vibepollo_auto_add.set(next);
                                                            let mut s = load_state();
                                                            s.vibepollo_auto_add = next;
                                                            let _ = save_state(&s);
                                                            let key = if next { "toast_vibepollo_added" } else { "toast_vibepollo_removed" };
                                                            copy_toast_text.set(crate::core::i18n::t(&current_lang.read(), key).to_string());
                                                            *toast_generation.write() += 1;
                                                            copy_toast.set(true);
                                                        }
                                                        _ => {
                                                            if next {
                                                                vibepollo_auto_add.set(false);
                                                            }
                                                            copy_toast_text.set(crate::core::i18n::t(&current_lang.read(), "toast_vibepollo_uac_cancelled").to_string());
                                                            *toast_generation.write() += 1;
                                                            copy_toast.set(true);
                                                        }
                                                    }
                                                });
                                            },
                                            span { class: "knob" }
                                        }
                                    }
                                }
                            }

                            // Storage & Cache Cleanup
                            {
                                let cache_size = crate::core::steamart::get_art_cache_size();
                                let size_str = if cache_size >= 1024 * 1024 {
                                    format!("{:.1} MB", cache_size as f64 / (1024.0 * 1024.0))
                                } else {
                                    format!("{} KB", cache_size / 1024)
                                };
                                rsx! {
                                    div { class: "set-row", style: "display:flex; justify-content:space-between; align-items:center; padding:12px 0; border-bottom:1px solid var(--line);",
                                        div {
                                            div { class: "k", style: "font-weight:600; display:flex; align-items:center; gap:8px;",
                                                span { "{crate::core::i18n::t(&current_lang.read(), \"settings_storage_cache\")}" }
                                                span { class: "badge-tag dim", "{size_str}" }
                                            }
                                            div { class: "v", style: "font-size:0.85em; color:var(--dim);", "{crate::core::i18n::t(&current_lang.read(), \"settings_storage_cache_desc\")}" }
                                        }
                                        button {
                                            class: "ghost sm",
                                            id: "cleanArtCache",
                                            title: "{crate::core::i18n::t(&current_lang.read(), \"settings_clean_art_cache\")}",
                                            onclick: move |_| {
                                                let g_list = games.read().clone();
                                                let s = load_state();
                                                let stats = crate::core::steamart::cleanup_unused_art(&g_list, &s.hidden, &s.posters);
                                                let reclaimed_mb = stats.bytes_reclaimed as f64 / (1024.0 * 1024.0);
                                                let toast = if stats.files_removed > 0 {
                                                    format!("Cleaned {} orphaned art files ({:.2} MB reclaimed)", stats.files_removed, reclaimed_mb)
                                                } else {
                                                    "Artwork cache is already clean (0 orphaned files)".to_string()
                                                };
                                                copy_toast_text.set(toast);
                                                *toast_generation.write() += 1;
                                                copy_toast.set(true);
                                            },
                                            span { "🧹 " }
                                            "{crate::core::i18n::t(&current_lang.read(), \"settings_clean_art_btn\")}"
                                        }
                                    }
                                }
                            }

                            // Managed folders list
                            div { class: "set-row", style: "padding:12px 0; border-bottom:1px solid var(--line);",
                                div { style: "display:flex; justify-content:space-between; align-items:center; margin-bottom:8px;",
                                    div {
                                        div { class: "k", style: "font-weight:600;", "{crate::core::i18n::t(&current_lang.read(), \"settings_managed_folders\")}" }
                                        div { class: "v", style: "font-size:0.85em; color:var(--dim);", "{crate::core::i18n::t(&current_lang.read(), \"settings_managed_folders_desc\")}" }
                                    }
                                    button {
                                        class: "ghost sm",
                                        id: "setAddFolder",
                                        title: "{crate::core::i18n::t(&current_lang.read(), \"settings_add_folder\")}",
                                        onclick: move |_| {
                                            spawn(async move {
                                                if let Some(handle) = rfd::AsyncFileDialog::new().set_title("Scan this folder for games").pick_folder().await {
                                                    let folder = handle.path().to_path_buf();
                                                    let folder_games = tokio::task::spawn_blocking({
                                                        let folder = folder.clone();
                                                        move || scan_library_root(&folder)
                                                    }).await.unwrap_or_default();

                                                    let mut s = load_state();
                                                    for g in &folder_games {
                                                        s.unhide_game(&g.dir);
                                                    }
                                                    let f_str = folder.to_string_lossy().to_string();
                                                    if !s.folders.contains(&f_str) {
                                                        s.folders.push(f_str);
                                                        managed_folders.set(s.folders.clone());
                                                    }
                                                    let mut current_games = games.read().clone();
                                                    for g in folder_games {
                                                        if !current_games.iter().any(|existing| existing.dir == g.dir) {
                                                            current_games.push(g);
                                                        }
                                                    }
                                                    let deduped = dedupe_games(current_games);
                                                    s.cached_games = deduped.clone();
                                                    let _ = save_state(&s);
                                                    games.set(deduped.clone());
                                                    let missing: Vec<(String, std::path::PathBuf)> = deduped.iter()
                                                        .filter(|g| g.poster.is_none())
                                                        .map(|g| (g.name.clone(), g.dir.clone()))
                                                        .collect();
                                                    if !missing.is_empty() {
                                                        trigger_artwork_resolution(games, missing);
                                                    }
                                                }
                                            });
                                        },
                                        "{crate::core::i18n::t(&current_lang.read(), \"btn_add\")}"
                                    }
                                }
                                if managed_folders.read().is_empty() {
                                    p { class: "dim", style: "font-size:0.85em;", "{crate::core::i18n::t(&current_lang.read(), \"settings_none_configured\")}" }
                                } else {
                                    div { class: "paths",
                                        for f_path in managed_folders.read().iter() {
                                            {
                                                let p_str = f_path.clone();
                                                rsx! {
                                                    div { class: "path-row", style: "display:flex; justify-content:space-between; align-items:center; padding:4px 0;",
                                                        span { style: "font-family:monospace; font-size:0.85em;", "{p_str}" }
                                                        button {
                                                            class: "drop",
                                                            title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_remove_folder\")}",
                                                            onclick: move |_| {
                                                                let mut s = load_state();
                                                                s.folders.retain(|f| f != &p_str);
                                                                managed_folders.set(s.folders.clone());
                                                                let mut cur_games = games.read().clone();
                                                                let p_buf = std::path::PathBuf::from(&p_str);
                                                                cur_games.retain(|g| !g.dir.starts_with(&p_buf));
                                                                s.cached_games = cur_games.clone();
                                                                let _ = save_state(&s);
                                                                games.set(cur_games);
                                                            },
                                                            "✕"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }

                            // Hidden games management
                            if *hidden_count.read() > 0 {
                                div { class: "set-row", style: "display:flex; justify-content:space-between; align-items:center; padding:12px 0; border-bottom:1px solid var(--line);",
                                    div {
                                        div { class: "k", style: "font-weight:600;", "{crate::core::i18n::t(&current_lang.read(), \"settings_hidden_games\")}" }
                                        div { class: "v", style: "font-size:0.85em; color:var(--dim);", "{hidden_count.read()} {crate::core::i18n::t(&current_lang.read(), \"settings_hidden_count_suffix\")}" }
                                    }
                                    button {
                                        class: "ghost sm",
                                        id: "setUnhideAll",
                                        onclick: move |_| {
                                            let mut s = load_state();
                                            let count = s.hidden.len();
                                            s.unhide_all();
                                            let _ = save_state(&s);
                                            let discovered = discover_all_launchers();
                                            let deduped = dedupe_games(discovered);
                                            let mut current_s = load_state();
                                            current_s.cached_games = deduped.clone();
                                            let _ = save_state(&current_s);
                                            games.set(deduped);
                                            hidden_count.set(0);
                                            crate::core::logger::info("library", &format!("Restored {} hidden game(s)", count));
                                            copy_toast_text.set(format!("Restored {} hidden game(s)", count));
                                            *toast_generation.write() += 1;
                                            copy_toast.set(true);
                                        },
                                        "{crate::core::i18n::t(&current_lang.read(), \"settings_unhide_all\")}"
                                    }
                                }
                            }

                            // Library configuration file & reset
                            div { class: "set-row", style: "display:flex; justify-content:space-between; align-items:center; padding:12px 0; border-bottom:1px solid var(--line);",
                                div {
                                    div { class: "k", style: "font-weight:600;", "{crate::core::i18n::t(&current_lang.read(), \"settings_lib_config\")}" }
                                    div { class: "v", style: "font-size:0.85em; font-family:monospace; color:var(--dim);", "{get_state_path().to_string_lossy()}" }
                                }
                                button {
                                    class: "ghost sm",
                                    id: "setReset",
                                    onclick: move |_| {
                                        let _ = fs::remove_file(get_state_path());
                                        managed_folders.set(Vec::new());
                                        games.set(Vec::new());
                                        copy_toast_text.set(crate::core::i18n::t(&current_lang.read(), "toast_config_reset").to_string());
                                        *toast_generation.write() += 1;
                                        copy_toast.set(true);
                                    },
                                    "{crate::core::i18n::t(&current_lang.read(), \"settings_reset\")}"
                                }
                            }

                            // GPU diagnostics
                            div { style: "display:flex; justify-content:space-between; align-items:center; padding:12px 0; border-bottom:1px solid var(--line);",
                                span { "{crate::core::i18n::t(&current_lang.read(), \"diag_primary_gpu\")}" }
                                b { "{primary_gpu.name}" }
                            }
                            div { style: "display:flex; justify-content:space-between; align-items:center; padding:12px 0; border-bottom:1px solid var(--line);",
                                span { "{crate::core::i18n::t(&current_lang.read(), \"diag_vram\")}" }
                                b { "{primary_gpu.dedicated_video_memory / 1024 / 1024} MB" }
                            }
                            div { style: "display:flex; justify-content:space-between; align-items:center; padding:12px 0;",
                                span { "{crate::core::i18n::t(&current_lang.read(), \"diag_ada_arch\")}" }
                                if primary_gpu.is_rtx_40 {
                                    span { style: "color:var(--accent);font-weight:700;", "{crate::core::i18n::t(&current_lang.read(), \"diag_ada_supported\")}" }
                                } else {
                                    span { "{crate::core::i18n::t(&current_lang.read(), \"diag_ada_standard\")}" }
                                }
                            }
                        }
                    }
                }

                // ---------------- ABOUT VIEW ----------------
                if *active_view.read() == "about" {
                    section {
                        class: "view active",
                        id: "view-about",
                        style: "cursor: default;",
                        div { class: "section-head",
                            h3 { "{crate::core::i18n::t(&current_lang.read(), \"nav_about\")}" }
                        }
                        div { class: "glass pad about",
                            p { b { "DLSS 5 Studio v{crate::core::APP_VERSION}" } }
                            p { "{crate::core::i18n::t(&current_lang.read(), \"about_desc_main\")}" }
                            div { class: "about-links", style: "display:flex; flex-wrap:wrap; gap:10px; margin: 16px 0;",
                                a { class: "glass-btn", href: "https://github.com/bookamp/dlss-studio", target: "_blank", "{crate::core::i18n::t(&current_lang.read(), \"about_github_repo\")}" }
                                a { class: "glass-btn", href: "https://github.com/bookamp/dlss-studio/releases", target: "_blank", "{crate::core::i18n::t(&current_lang.read(), \"about_releases\")}" }
                                a { class: "glass-btn", href: "https://github.com/bookamp/dlss-studio/issues", target: "_blank", "{crate::core::i18n::t(&current_lang.read(), \"about_report_issue\")}" }
                            }
                            p { class: "dim", "{crate::core::i18n::t(&current_lang.read(), \"about_desc_sub\")}" }
                        }
                    }
                }
            }

            // ---------------- GAME DETAIL SHEET MODAL ----------------
            if *sheet_open.read() {
                if let Some(mut target_game) = selected_game {
                    {
                        if target_game.poster.is_none() {
                            trigger_artwork_resolution(games, vec![(target_game.name.clone(), target_game.dir.clone())]);
                        }
                        let hero_p = crate::core::state::get_appdata_dir()
                            .join("art")
                            .join(format!("{}-hero.jpg", crate::core::steamart::key_for_dir(&target_game.dir)));
                        let hero_key = crate::core::steamart::key_for_dir(&target_game.dir);
                        let hero_url = if hero_p.exists() {
                            Some(format!("http://dlss-art.localhost/art/{}-hero.jpg", hero_key))
                        } else {
                            target_game.poster.as_ref().map(|p| crate::core::steamart::normalize_art_uri(p))
                        };
                        let cover_url = target_game.poster.as_ref().map(|p| crate::core::steamart::normalize_art_uri(p)).or(hero_url.clone());
                        let reshade_label = if target_game.reshade_installed {
                            let v_str = target_game.reshade_version.as_deref().unwrap_or("6.8.0");
                            let add_suffix = if target_game.reshade_addon_support { " + add-on" } else { "" };
                            format!("{}{}", v_str, add_suffix)
                        } else {
                            crate::core::i18n::t(&current_lang.read(), "val_not_installed").to_string()
                        };
                        let mut available_exes = if !target_game.available_exes.is_empty() {
                            target_game.available_exes.clone()
                        } else {
                            crate::core::scan::discover_game_exes(&target_game.dir)
                        };
                        available_exes.retain(|e| !crate::core::scan::is_helper_or_tool_path(&e.path));

                        let is_bad_exe = crate::core::scan::is_helper_or_tool_path(&target_game.exe_path)
                            || !target_game.exe_path.is_file()
                            || target_game.exe_path.as_os_str().is_empty();

                        if (is_bad_exe || !available_exes.iter().any(|e| e.path == target_game.exe_path)) && !available_exes.is_empty() {
                            target_game.exe_path = available_exes[0].path.clone();
                            target_game.exe_rel = available_exes[0].rel.clone();
                            target_game.api = available_exes[0].api.clone();
                            target_game.bitness = available_exes[0].bitness;
                            target_game.available_exes = available_exes.clone();

                            let dir = target_game.dir.clone();
                            let updated = target_game.clone();
                            let mut current_games = games.read().clone();
                            if let Some(pos) = current_games.iter().position(|g| g.dir == dir) {
                                current_games[pos] = updated;
                                let mut s = load_state();
                                s.cached_games = current_games.clone();
                                let _ = save_state(&s);
                                games.set(current_games);
                            }
                        }
                        let has_native_dlss = target_game.has_native_dlss();
                        let api_lower = target_game.api.to_lowercase();
                        let is_dx12 = api_lower.contains("12") || api_lower.contains("d3d12");
                        let is_vulkan = api_lower.contains("vulkan");
                        let is_dx11 = (api_lower.contains("11") || api_lower == "d3d11") && !is_dx12;
                        target_game.can_inject_fg = target_game.bitness == 64 && has_native_dlss && !target_game.has_frame_generation && (is_dx12 || is_vulkan);
                        let opti_advisory = crate::core::install_routes::get_optiscaler_advisory(&target_game);
                        let native_advisory = crate::core::install_routes::get_native_dlss_advisory(&target_game);
                        let mfg_advisory = crate::core::install_routes::get_mfg_advisory(&target_game, primary_gpu.is_rtx_40);
                        let ac_warning = target_game.has_anti_cheat;
                        let _has_dlss = target_game.dlss_version.is_some();
                        let cur_backend = backend_choice.read().clone();
                        let effective_backend = if cur_backend == "optiscaler" {
                            "optiscaler".to_string()
                        } else {
                            "reshade".to_string()
                        };
                        let cur_route = route_choice.read().clone();
                        let effective_route = if cur_route == "native" {
                            "native".to_string()
                        } else {
                            "feeder".to_string()
                        };

                        let mut active_advisories: Vec<crate::core::install_routes::RouteAdvisory> = Vec::new();
                        if effective_backend == "optiscaler" {
                            if let Some(adv) = &opti_advisory {
                                active_advisories.push(adv.clone());
                            }
                        } else if effective_route == "native" {
                            if let Some(adv) = &native_advisory {
                                active_advisories.push(adv.clone());
                            }
                        }
                        if *mfg_choice.read() {
                            if let Some(adv) = &mfg_advisory {
                                active_advisories.push(adv.clone());
                            }
                        }
                        let has_override = !active_advisories.is_empty();

                        let show_mfg = true;
                        let show_pre_sr = effective_backend == "optiscaler";
                        let show_nr_style = effective_backend == "reshade" || (effective_backend == "optiscaler" && *opti_pre_sr.read());
                        let feeder_label = if is_dx11 {
                            crate::core::i18n::t(&current_lang.read(), "feeder_label_dx11")
                        } else if is_vulkan {
                            crate::core::i18n::t(&current_lang.read(), "feeder_label_vulkan")
                        } else if is_dx12 {
                            crate::core::i18n::t(&current_lang.read(), "feeder_label_dx12")
                        } else if api_lower.contains("opengl") {
                            crate::core::i18n::t(&current_lang.read(), "feeder_label_opengl")
                        } else {
                            crate::core::i18n::t(&current_lang.read(), "feeder_label_general")
                        };
                        let opti_display = if opti_advisory.is_some() {
                            format!("{} ⚠️", crate::core::i18n::t(&current_lang.read(), "opt_optiscaler_dlssnr"))
                        } else {
                            crate::core::i18n::t(&current_lang.read(), "opt_optiscaler_dlssnr").to_string()
                        };
                        let native_display = if native_advisory.is_some() {
                            format!("{} ⚠️", crate::core::i18n::t(&current_lang.read(), "opt_native_dlss"))
                        } else {
                            crate::core::i18n::t(&current_lang.read(), "opt_native_dlss").to_string()
                        };
                        let laa_status = if target_game.bitness == 32 {
                            if target_game.is_laa {
                                " • 4GB Patch (LAA): Active"
                            } else {
                                " • 4GB Patch (LAA): Auto on Deploy"
                            }
                        } else {
                            ""
                        };
                        let is_dlss5 = target_game.is_dlss5_patched();
                        let launch_tooltip = if is_dlss5 {
                            crate::core::i18n::t_params(&current_lang.read(), "tooltip_launch_game_modded", &[&target_game.name, target_game.route_display_name()])
                        } else {
                            crate::core::i18n::t_param(&current_lang.read(), "tooltip_launch_game_vanilla", &target_game.name)
                        };

                        rsx! {
                            div {
                                class: "overlay",
                                id: "overlay",
                                onmousedown: move |e| e.stop_propagation(),
                                onclick: move |_| sheet_open.set(false),
                                div {
                                    class: "sheet",
                                    id: "sheet",
                                    onmousedown: move |e| e.stop_propagation(),
                                    onclick: move |e: MouseEvent| e.stop_propagation(),

                                    div { class: "hero",
                                        if let Some(ref h_url) = hero_url {
                                            img { src: "{h_url}", alt: "{target_game.name}" }
                                        }
                                        button {
                                            class: "close",
                                            id: "sheetClose",
                                            onmousedown: move |e| e.stop_propagation(),
                                            onclick: move |e: MouseEvent| {
                                                e.stop_propagation();
                                                sheet_open.set(false);
                                            },
                                            dangerous_inner_html: r#"<svg viewBox="0 0 24 24"><path d="M6 6l12 12M18 6L6 18"/></svg>"#
                                        }
                                    }

                                    div { class: "sheet-body",
                                div { class: "head",
                                    div {
                                        class: "cover",
                                        style: "cursor: pointer;",
                                        title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_click_change_cover\")}",
                                        onclick: {
                                            let dir = target_game.dir.clone();
                                            move |e: MouseEvent| {
                                                e.stop_propagation();
                                                pick_and_set_cover(&dir, games);
                                            }
                                        },
                                        if let Some(ref p_url) = cover_url {
                                            img { src: "{p_url}", alt: "{target_game.name}" }
                                        } else {
                                            div { class: "placeholder", "{&target_game.name[0..2.min(target_game.name.len())]}" }
                                        }
                                    }
                                    div { class: "who",
                                        if !*is_editing_game_name.read() {
                                            div { class: "sheet-title-row",
                                                h3 { "{target_game.name}" }
                                                button {
                                                    class: "btn-rename-pencil",
                                                    title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_rename_game\")}",
                                                    onclick: {
                                                        let cur_name = target_game.name.clone();
                                                        move |e: MouseEvent| {
                                                            e.stop_propagation();
                                                            edit_game_name_val.set(cur_name.clone());
                                                            is_editing_game_name.set(true);
                                                        }
                                                    },
                                                    dangerous_inner_html: r#"<svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>"#
                                                }
                                            }
                                        } else {
                                            div { class: "sheet-title-edit-wrap",
                                                input {
                                                    r#type: "text",
                                                    class: "sheet-title-input",
                                                    value: "{edit_game_name_val.read()}",
                                                    autofocus: true,
                                                    oninput: move |e| edit_game_name_val.set(e.value()),
                                                    onkeydown: {
                                                        let dir = target_game.dir.clone();
                                                        let cur_idx = sheet_game_idx.read().unwrap_or(0);
                                                        move |e: KeyboardEvent| {
                                                            if e.key() == Key::Enter {
                                                                let val = edit_game_name_val.read().trim().to_string();
                                                                let mut s = load_state();
                                                                s.set_custom_name(&dir, &val);
                                                                let _ = save_state(&s);
                                                                let final_name = if val.is_empty() {
                                                                    crate::core::scan::scan_game_directory(&dir).map(|g| g.name).unwrap_or_else(|| dir.file_name().unwrap_or_default().to_string_lossy().to_string())
                                                                } else {
                                                                    val
                                                                };
                                                                let mut cur = games.read().clone();
                                                                if cur_idx < cur.len() {
                                                                    cur[cur_idx].name = final_name;
                                                                    s.cached_games = cur.clone();
                                                                    let _ = save_state(&s);
                                                                    games.set(cur);
                                                                }
                                                                is_editing_game_name.set(false);
                                                            } else if e.key() == Key::Escape {
                                                                is_editing_game_name.set(false);
                                                            }
                                                        }
                                                    }
                                                }
                                                button {
                                                    class: "sheet-title-btn save",
                                                    title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_save_name\")}",
                                                    onclick: {
                                                        let dir = target_game.dir.clone();
                                                        let cur_idx = sheet_game_idx.read().unwrap_or(0);
                                                        move |e: MouseEvent| {
                                                            e.stop_propagation();
                                                            let val = edit_game_name_val.read().trim().to_string();
                                                            let mut s = load_state();
                                                            s.set_custom_name(&dir, &val);
                                                            let _ = save_state(&s);
                                                            let final_name = if val.is_empty() {
                                                                crate::core::scan::scan_game_directory(&dir).map(|g| g.name).unwrap_or_else(|| dir.file_name().unwrap_or_default().to_string_lossy().to_string())
                                                            } else {
                                                                val
                                                            };
                                                            let mut cur = games.read().clone();
                                                            if cur_idx < cur.len() {
                                                                cur[cur_idx].name = final_name;
                                                                s.cached_games = cur.clone();
                                                                let _ = save_state(&s);
                                                                games.set(cur);
                                                            }
                                                            is_editing_game_name.set(false);
                                                        }
                                                    },
                                                    dangerous_inner_html: r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>"#
                                                }
                                                button {
                                                    class: "sheet-title-btn cancel",
                                                    title: "{crate::core::i18n::t(&current_lang.read(), \"btn_cancel\")}",
                                                    onclick: move |e: MouseEvent| {
                                                        e.stop_propagation();
                                                        is_editing_game_name.set(false);
                                                    },
                                                    dangerous_inner_html: r#"<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>"#
                                                }
                                            }
                                        }
                                        div { class: "meta", "{target_game.launcher} • {target_game.api} • {target_game.bitness}-bit{laa_status}" }
                                        div {
                                            class: "path",
                                            style: "cursor:pointer;",
                                            onclick: {
                                                let dir = target_game.dir.clone();
                                                move |_| {
                                                    let _ = std::process::Command::new("explorer").arg(&dir).spawn();
                                                }
                                            },
                                            "{crate::core::state::clean_path_separators(&target_game.dir).display()}"
                                        }
                                        if available_exes.len() > 1 {
                                            div { class: "exe-switch-row", style: "display:flex; align-items:center; gap:8px; margin:8px 0 10px 0;",
                                                span { style: "font-size:0.8em; color:var(--dim); font-weight:600;", "{crate::core::i18n::t(&current_lang.read(), \"sheet_executable\")}" }
                                                select {
                                                    class: "select sm",
                                                    style: "color-scheme:dark; background:#18181b; color:#f4f4f5; border:1px solid var(--line); border-radius:4px; padding:3px 8px; font-size:0.85em; cursor:pointer;",
                                                    value: "{target_game.exe_path.display()}",
                                                    onchange: {
                                                        let dir = target_game.dir.clone();
                                                        let exes = available_exes.clone();
                                                        move |e: Event<FormData>| {
                                                            let selected_path_str = e.value();
                                                            if let Some(opt) = exes.iter().find(|x| x.path.to_string_lossy() == selected_path_str) {
                                                                let mut current_games = games.read().clone();
                                                                if let Some(pos) = current_games.iter().position(|g| g.dir == dir) {
                                                                    current_games[pos].exe_path = opt.path.clone();
                                                                    current_games[pos].exe_rel = opt.rel.clone();
                                                                    current_games[pos].api = opt.api.clone();
                                                                    current_games[pos].bitness = opt.bitness;
                                                                    current_games[pos].is_laa = opt.is_laa;
                                                                    if current_games[pos].available_exes.is_empty() {
                                                                        current_games[pos].available_exes = exes.clone();
                                                                    }
                                                                    let api_lower_opt = opt.api.to_lowercase();
                                                                    let is_dx12_opt = api_lower_opt.contains("12") || api_lower_opt.contains("d3d12");
                                                                    let is_vulkan_opt = api_lower_opt.contains("vulkan");
                                                                    current_games[pos].can_inject_fg = opt.bitness == 64 && has_native_dlss && !current_games[pos].has_frame_generation && (is_dx12_opt || is_vulkan_opt);

                                                                    // Only auto-adjust route on exe switch if no deploys have been made at all (unpatched)
                                                                    let is_deployed = current_games[pos].installed_route.is_some()
                                                                        || current_games[pos].optiscaler_installed
                                                                        || current_games[pos].addon_installed
                                                                        || current_games[pos].reshade_installed
                                                                        || current_games[pos].has_backup;
                                                                    if !is_deployed {
                                                                        let rec = crate::core::install_routes::recommended_route(&current_games[pos]);
                                                                        route_choice.set(rec.as_str().to_string());
                                                                    }

                                                                    let mut s = load_state();
                                                                    s.cached_games = current_games.clone();
                                                                    let _ = save_state(&s);
                                                                    games.set(current_games);
                                                                }
                                                            }
                                                        }
                                                    },
                                                    for opt in &available_exes {
                                                        option {
                                                            style: "background:#18181b; color:#f4f4f5;",
                                                            value: "{opt.path.display()}",
                                                            selected: opt.path == target_game.exe_path,
                                                            "{opt.name} ({opt.api})"
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        button {
                                            class: if is_dlss5 { "btn-launch-hero modded" } else { "btn-launch-hero vanilla" },
                                            title: "{launch_tooltip}",
                                            onclick: {
                                                let g = target_game.clone();
                                                move |e: MouseEvent| {
                                                    e.stop_propagation();
                                                    launch_game(&g);
                                                }
                                            },
                                            svg {
                                                view_box: "0 0 24 24",
                                                polygon { points: "6,4 20,12 6,20" }
                                            }
                                            span { "{crate::core::i18n::t(&current_lang.read(), \"sheet_launch_game\")}" }
                                        }
                                    }
                                }



                                // Specs Grid
                                div { class: "specs",
                                    div { class: "spec",
                                        span { class: "k", "{crate::core::i18n::t(&current_lang.read(), \"spec_target_exe\")}" }
                                        span { class: "v", "{target_game.exe_rel}" }
                                    }
                                    div { class: "spec",
                                        span { class: "k", "{crate::core::i18n::t(&current_lang.read(), \"spec_architecture\")}" }
                                        span { class: "v", "{target_game.bitness}-bit" }
                                    }
                                    div { class: "spec",
                                        span { class: "k", "{crate::core::i18n::t(&current_lang.read(), \"spec_rendering_api\")}" }
                                        span { class: "v on", "{target_game.api}" }
                                    }
                                    div { class: "spec",
                                        span { class: "k", "{crate::core::i18n::t(&current_lang.read(), \"spec_installed_backend\")}" }
                                        if target_game.installed_route.as_deref() == Some("optiscaler") || (target_game.optiscaler_installed && target_game.installed_route.is_none()) {
                                            span { class: "v on", "{crate::core::i18n::t(&current_lang.read(), \"opt_optiscaler_dlssnr\")}" }
                                        } else if target_game.installed_route.as_deref() == Some("native") {
                                            span { class: "v on", "{crate::core::i18n::t(&current_lang.read(), \"opt_native_dlss\")}" }
                                        } else if target_game.installed_route.as_deref() == Some("feeder") {
                                            span { class: "v on", "{crate::core::i18n::t(&current_lang.read(), \"opt_dlss5_feeder\")}" }
                                        } else if target_game.installed_route.is_some() || target_game.reshade_installed {
                                            span { class: "v on", "ReShade" }
                                        } else {
                                            span { class: "v", "{crate::core::i18n::t(&current_lang.read(), \"val_none\")}" }
                                        }
                                    }
                                    div { class: "spec",
                                        span { class: "k", "DLSS" }
                                        if let Some(ref v) = target_game.dlss_version {
                                            span { class: "v on", "{v}" }
                                        } else {
                                            span { class: "v", "{crate::core::i18n::t(&current_lang.read(), \"val_none\")}" }
                                        }
                                    }
                                    div { class: "spec",
                                        span { class: "k", "{crate::core::i18n::t(&current_lang.read(), \"spec_frame_generation\")}" }
                                        if target_game.has_frame_generation {
                                            span { class: "v on", "{crate::core::i18n::t(&current_lang.read(), \"val_fg_supported\")}" }
                                        } else {
                                            span { class: "v", "{crate::core::i18n::t(&current_lang.read(), \"val_fg_unsupported\")}" }
                                        }
                                    }
                                    div { class: "spec",
                                        span { class: "k", "{crate::core::i18n::t(&current_lang.read(), \"spec_addon\")}" }
                                        if target_game.addon_installed {
                                            span { class: "v on", "{crate::core::i18n::t(&current_lang.read(), \"filter_installed\")}" }
                                        } else {
                                            span { class: "v", "{crate::core::i18n::t(&current_lang.read(), \"val_not_present\")}" }
                                        }
                                    }
                                    div { class: "spec",
                                        span { class: "k", "ReShade" }
                                        if target_game.reshade_installed {
                                            span { class: "v on", "{reshade_label}" }
                                        } else {
                                            span { class: "v", "{reshade_label}" }
                                        }
                                    }
                                    if target_game.optiscaler_installed {
                                        div { class: "spec",
                                            span { class: "k", "{crate::core::i18n::t(&current_lang.read(), \"spec_presr_passes\")}" }
                                            span { class: "v", "{target_game.optiscaler_passes}x" }
                                        }
                                    }
                                }

                                // Backend Compatibility Check & Route Arbitration
                                if ac_warning {
                                    div {
                                        class: "emu-note anti-cheat-warning",
                                        b { "{crate::core::i18n::t(&current_lang.read(), \"sheet_anticheat_title\")}" }
                                        span { "{crate::core::i18n::t(&current_lang.read(), \"sheet_anticheat_desc\")}" }
                                    }
                                }

                                div { class: "install-options",
                                    label {
                                        span { "{crate::core::i18n::t(&current_lang.read(), \"sheet_backend_label\")}" }
                                        select {
                                            id: "backendChoice",
                                            value: "{effective_backend}",
                                            onchange: move |e| backend_choice.set(e.value()),
                                            option { value: "reshade", "{crate::core::i18n::t(&current_lang.read(), \"backend_reshade_default\")}" }
                                            option { value: "optiscaler", "{opti_display}" }
                                        }
                                    }
                                    if effective_backend != "optiscaler" {
                                        label {
                                            span { "{crate::core::i18n::t(&current_lang.read(), \"sheet_route_label\")}" }
                                            select {
                                                id: "routeChoice",
                                                value: "{effective_route}",
                                                onchange: move |e| route_choice.set(e.value()),
                                                option { value: "native", "{native_display}" }
                                                option { value: "feeder", "{feeder_label}" }
                                            }
                                        }
                                    }
                                }

                                for adv in &active_advisories {
                                    div {
                                        class: "emu-note incompatibility-warning",
                                        b { "{crate::core::i18n::t_param(&current_lang.read(), \"advisory_warning_high\", &adv.title)}" }
                                        p { class: "advisory-intro", "{crate::core::i18n::t(&current_lang.read(), \"advisory_intro_restrictions\")}" }
                                        ul { class: "advisory-reasons",
                                            for reason in &adv.reasons {
                                                li { "{crate::core::i18n::translate_advisory_reason(&current_lang.read(), reason)}" }
                                            }
                                        }
                                        div { class: "advisory-footer",
                                            span { "{crate::core::i18n::translate_advisory_recommendation(&current_lang.read(), &adv.recommendation)}" }
                                        }
                                    }
                                }

                                        if show_pre_sr {
                                            div { class: if *opti_pre_sr.read() { "sheet-feature-card on" } else { "sheet-feature-card" },
                                                input {
                                                    type: "checkbox",
                                                    id: "chkPreSr",
                                                    checked: *opti_pre_sr.read(),
                                                    onchange: move |e| opti_pre_sr.set(e.checked())
                                                }
                                                div { class: "body",
                                                    div { class: "t",
                                                        label {
                                                            r#for: "chkPreSr",
                                                            class: "t-left",
                                                            style: "cursor: pointer;",
                                                            span { "{crate::core::i18n::t(&current_lang.read(), \"feature_presr_title\")}" }
                                                            span { class: "tag accent", "{crate::core::i18n::t(&current_lang.read(), \"feature_presr_tag\")}" }
                                                        }
                                                        div { class: "passes-ctrl",
                                                            span { "{crate::core::i18n::t(&current_lang.read(), \"feature_presr_passes\")}" }
                                                            select {
                                                                class: "passes-select",
                                                                value: "{opti_passes}",
                                                                disabled: !*opti_pre_sr.read(),
                                                                onchange: move |e| opti_passes.set(e.value().parse::<u32>().unwrap_or(1)),
                                                                option { value: "1", "1x" }
                                                                option { value: "2", "2x" }
                                                                option { value: "3", "3x" }
                                                            }
                                                        }
                                                    }
                                                    div { class: "d", "{crate::core::i18n::t(&current_lang.read(), \"feature_presr_desc\")}" }
                                                }
                                            }
                                        }

                                        if show_nr_style {
                                            div { class: if *nr_style_choice.read() { "sheet-feature-card on" } else { "sheet-feature-card" },
                                                input {
                                                    type: "checkbox",
                                                    id: "chkNrStyle",
                                                    checked: *nr_style_choice.read(),
                                                    onchange: move |e| nr_style_choice.set(e.checked())
                                                }
                                                div { class: "body",
                                                    div { class: "t",
                                                        label {
                                                            r#for: "chkNrStyle",
                                                            class: "t-left",
                                                            style: "cursor: pointer;",
                                                            span { "{crate::core::i18n::t(&current_lang.read(), \"feature_nr_style_title\")}" }
                                                            span { class: "tag accent", "{crate::core::i18n::t(&current_lang.read(), \"feature_nr_style_tag\")}" }
                                                        }
                                                        div { class: "passes-ctrl",
                                                            span { "{crate::core::i18n::t(&current_lang.read(), \"feature_nr_style_label\")}" }
                                                            select {
                                                                class: "passes-select",
                                                                style: "min-width: 140px;",
                                                                value: "{nr_style_preset}",
                                                                disabled: !*nr_style_choice.read(),
                                                                onchange: move |e| nr_style_preset.set(e.value().parse::<usize>().unwrap_or(0)),
                                                                option { value: "0", "{crate::core::i18n::t(&current_lang.read(), \"preview_model_a\")}" }
                                                                option { value: "1", "{crate::core::i18n::t(&current_lang.read(), \"preview_model_b\")}" }
                                                                option { value: "2", "{crate::core::i18n::t(&current_lang.read(), \"preview_model_c\")}" }
                                                            }
                                                        }
                                                    }
                                                    div { class: "d", "{crate::core::i18n::t(&current_lang.read(), \"feature_nr_style_desc\")}" }
                                                }
                                            }
                                        }

                                        if show_mfg {
                                            div { class: if *mfg_choice.read() { "sheet-feature-card on" } else { "sheet-feature-card" },
                                                input {
                                                    type: "checkbox",
                                                    id: "chkMfg",
                                                    checked: *mfg_choice.read(),
                                                    onchange: move |e| mfg_choice.set(e.checked())
                                                }
                                                div { class: "body",
                                                    div { class: "t",
                                                        label {
                                                            r#for: "chkMfg",
                                                            class: "t-left",
                                                            style: "cursor: pointer;",
                                                            span { "{crate::core::i18n::t(&current_lang.read(), \"feature_mfg_title\")}" }
                                                            if mfg_advisory.is_some() {
                                                                span { class: "tag advisory", "{crate::core::i18n::t(&current_lang.read(), \"advisory_tag_warn\")}" }
                                                            } else {
                                                                span { class: "tag warn", "{crate::core::i18n::t(&current_lang.read(), \"feature_mfg_tag\")}" }
                                                            }
                                                        }
                                                        div { class: "passes-ctrl",
                                                            span { "{crate::core::i18n::t(&current_lang.read(), \"sheet_mfg_multiplier\")}" }
                                                            select {
                                                                class: "passes-select",
                                                                value: "{mfg_multiplier}",
                                                                disabled: !*mfg_choice.read(),
                                                                onchange: move |e| mfg_multiplier.set(e.value().parse::<u32>().unwrap_or(4)),
                                                                option { value: "1", "1x (Auto)" }
                                                                option { value: "2", "2x" }
                                                                option { value: "3", "3x" }
                                                                option { value: "4", "4x (Default)" }
                                                            }
                                                        }
                                                    }
                                                    div { class: "d", "{crate::core::i18n::t(&current_lang.read(), \"feature_mfg_desc\")}" }
                                                }
                                            }
                                        }

                                        // Detected Graphics Modules (Collapsible)
                                        {
                                            if !target_game.files.is_empty() {
                                                let is_exp = *modules_expanded.read();
                                                let mod_count = target_game.files.len();
                                                rsx! {
                                                    div { class: "modules-collapsible",
                                                        button {
                                                            class: "modules-header",
                                                            r#type: "button",
                                                            title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_toggle_modules\")}",
                                                            onclick: move |evt: MouseEvent| {
                                                                evt.stop_propagation();
                                                                let current = *modules_expanded.read();
                                                                modules_expanded.set(!current);
                                                            },
                                                            span {
                                                                class: if is_exp { "modules-chevron is-expanded" } else { "modules-chevron" },
                                                                svg {
                                                                    view_box: "0 0 24 24",
                                                                    fill: "none",
                                                                    stroke: "currentColor",
                                                                    stroke_width: "2.8",
                                                                    stroke_linecap: "round",
                                                                    stroke_linejoin: "round",
                                                                    polyline { points: "9 18 15 12 9 6" }
                                                                }
                                                            }
                                                            span { class: "modules-title",
                                                                "{crate::core::i18n::t(&current_lang.read(), \"sheet_detected_modules\")} ({mod_count})"
                                                            }
                                                            if !is_exp {
                                                                div { class: "summary-badges",
                                                                    for f in target_game.files.iter().take(4) {
                                                                        {
                                                                            let (_, _, vendor_class, badge_text) = resolve_module_meta(&f.rel);
                                                                            let ver_short = f.version.as_deref().map(|v| {
                                                                                let parts: Vec<&str> = v.split('.').take(2).collect();
                                                                                if parts.len() == 2 {
                                                                                    format!(" {}.{}", parts[0], parts[1])
                                                                                } else {
                                                                                    format!(" {}", v)
                                                                                }
                                                                            }).unwrap_or_default();
                                                                            rsx! {
                                                                                span { class: "summary-pill {vendor_class}",
                                                                                    "{badge_text}{ver_short}"
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                    if mod_count > 4 {
                                                                        span { class: "summary-pill", "+{mod_count - 4}" }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                        if is_exp {
                                                            div { class: "modules-body",
                                                                for f in &target_game.files {
                                                                    {
                                                                        let (name_key, vendor_tag, vendor_class, _) = resolve_module_meta(&f.rel);
                                                                        let title = crate::core::i18n::t(&current_lang.read(), name_key);
                                                                        let ver_text = f.version.as_deref().unwrap_or("-");
                                                                        let ver_class = if f.version.is_some() { "module-version-pill" } else { "module-version-pill unknown" };
                                                                        rsx! {
                                                                            div { class: "module-card",
                                                                                div { class: "module-main",
                                                                                    div { class: "module-title-row",
                                                                                        span { class: "vendor-badge {vendor_class}", "{vendor_tag}" }
                                                                                        span { class: "module-name", "{title}" }
                                                                                    }
                                                                                    span { class: "module-path", "{f.rel}" }
                                                                                }
                                                                                span { class: "{ver_class}", "{ver_text}" }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            } else {
                                                rsx! {}
                                            }
                                        }

                                        // Actions
                                        {
                                            let is_deployed = target_game.optiscaler_installed
                                                || target_game.addon_installed
                                                || target_game.installed_route.is_some()
                                                || target_game.has_backup;
                                            rsx! {
                                                div { class: "sheet-actions",
                                                    button {
                                                        class: if has_override { "btn-install override-mode" } else { "btn-install" },
                                                        id: "doInstall",
                                                        disabled: *is_busy.read(),
                                                        onclick: {
                                                            let target_game = target_game.clone();
                                                            let cur_eff_backend = effective_backend.clone();
                                                            let cur_eff_route = effective_route.clone();
                                                            let opti_pre_sr_val = *opti_pre_sr.read();
                                                            let opti_passes_val = *opti_passes.read();
                                                            let mfg_choice_val = *mfg_choice.read();
                                                            let mfg_multiplier_val = *mfg_multiplier.read();
                                                            let nr_style_choice_val = *nr_style_choice.read();
                                                            let nr_style_val = if nr_style_choice_val { *nr_style_preset.read() } else { 0 };
                                                            move |_| {
                                                                if *is_busy.read() {
                                                                    return;
                                                                }
                                                                is_busy.set(true);

                                                                let target_game = target_game.clone();
                                                                let cur_eff_backend = cur_eff_backend.clone();
                                                                let cur_eff_route = cur_eff_route.clone();

                                                                dioxus::prelude::spawn(async move {
                                                                    let mut lines = Vec::new();
                                                                    lines.push(format!("@{{log_deploy_start|{}}}", target_game.name));
                                                                    job_lines.set(lines.clone());

                                                                    let game_dir = target_game.dir.clone();
                                                                    let exe_path = target_game.exe_path.clone();

                                                                    let closed_check = tokio::task::spawn_blocking({
                                                                        let gd = game_dir.clone();
                                                                        let ep = exe_path.clone();
                                                                        move || assert_game_closed(&gd, Some(&ep))
                                                                    }).await;

                                                                    if let Ok(Err(e)) = closed_check {
                                                                        lines.push(format!("[ERROR] {}", e));
                                                                        job_lines.set(lines);
                                                                        is_busy.set(false);
                                                                        return;
                                                                    }

                                                                    let mod_root = managed_mod_root(&game_dir, Some(&exe_path)).unwrap_or_else(|| game_dir.clone());
                                                                    lines.push(format!("@{{log_routing_target|{}}}", mod_root.display()));
                                                                    job_lines.set(lines.clone());

                                                                    let deploy_opts = crate::core::optiscaler::DeployOptions {
                                                                        game_name: Some(target_game.name.clone()),
                                                                        game_dir: target_game.dir.clone(),
                                                                        exe_path: target_game.exe_path.clone(),
                                                                        api: target_game.api.clone(),
                                                                        pre_sr: opti_pre_sr_val,
                                                                        passes: opti_passes_val,
                                                                        mfg_unlock: mfg_choice_val,
                                                                        mfg_multiplier: mfg_multiplier_val,
                                                                        nr_style: nr_style_val,
                                                                        nr_style_enabled: nr_style_choice_val,
                                                                    };

                                                                    // Asynchronously fetch/verify Feeder components and latest MFG unlock if required
                                                                    let mut payloads = match crate::core::optiscaler::PayloadBundle::from_system() {
                                                                        Ok(p) => p,
                                                                        Err(e) => {
                                                                            lines.push(format!("[ERROR] System payload resolution failed: {}", e));
                                                                            job_lines.set(lines);
                                                                            is_busy.set(false);
                                                                            return;
                                                                        }
                                                                    };
                                                                    if cur_eff_backend != "optiscaler" && cur_eff_route == "feeder" {
                                                                        if payloads.feeder_components.is_none() {
                                                                            match crate::core::downloader::ensure_feeder_components(&mut lines).await {
                                                                                Ok(fc) => {
                                                                                    payloads.feeder_components = Some(fc);
                                                                                    job_lines.set(lines.clone());
                                                                                }
                                                                                Err(e) => {
                                                                                    lines.push(format!("[ERROR] Feeder component resolution failed: {}", e));
                                                                                    job_lines.set(lines);
                                                                                    is_busy.set(false);
                                                                                    return;
                                                                                }
                                                                            }
                                                                        }
                                                                        let api_lower = target_game.api.to_lowercase();
                                                                        let is_legacy_dx = api_lower.contains('9') || api_lower.contains('8') || api_lower.contains("d3d9") || api_lower.contains("d3d8");
                                                                        if is_legacy_dx && payloads.dgvoodoo.is_none() {
                                                                            match crate::core::downloader::ensure_dgvoodoo_components(&mut lines).await {
                                                                                Ok(dg) => {
                                                                                    payloads.dgvoodoo = Some(dg);
                                                                                    job_lines.set(lines.clone());
                                                                                }
                                                                                Err(e) => {
                                                                                    lines.push(format!("[ERROR] dgVoodoo component resolution failed: {}", e));
                                                                                    job_lines.set(lines);
                                                                                    is_busy.set(false);
                                                                                    return;
                                                                                }
                                                                            }
                                                                        }
                                                                        if mfg_choice_val && payloads.renodx_mfgunlock_addon.is_none() {
                                                                            match crate::core::downloader::ensure_mfg_v09_addon(&mut lines).await {
                                                                                Ok(addon_p) => {
                                                                                    payloads.renodx_mfgunlock_addon = Some(addon_p);
                                                                                    job_lines.set(lines.clone());
                                                                                }
                                                                                Err(e) => {
                                                                                    lines.push(format!("[WARN] MFG Unlock addon download: {}", e));
                                                                                    job_lines.set(lines.clone());
                                                                                }
                                                                            }
                                                                        }
                                                                    }

                                                                    // Perform deployment in blocking task
                                                                    let b_task = cur_eff_backend.clone();
                                                                    let r_task = cur_eff_route.clone();
                                                                    let result = tokio::task::spawn_blocking(move || {
                                                                        if b_task == "optiscaler" {
                                                                            crate::core::optiscaler::deploy_optiscaler(&deploy_opts)
                                                                        } else if r_task == "native" {
                                                                            crate::core::optiscaler::deploy_native_dlss5(&deploy_opts)
                                                                        } else {
                                                                            crate::core::optiscaler::deploy_feeder_with_bundle(&deploy_opts, &payloads)
                                                                        }
                                                                    }).await;

                                                                    match result {
                                                                        Ok(Ok(res)) => {
                                                                            lines.extend(res.log_lines);
                                                                            touch(&target_game.dir.to_string_lossy());

                                                                            let refreshed_opt = tokio::task::spawn_blocking({
                                                                                let gd = target_game.dir.clone();
                                                                                move || crate::core::scan::scan_game_directory(&gd)
                                                                            }).await.ok().flatten();

                                                                            let mut updated_game = target_game.clone();
                                                                            if let Some(mut refreshed) = refreshed_opt {
                                                                                refreshed.launcher = updated_game.launcher.clone();
                                                                                refreshed.exe_path = updated_game.exe_path.clone();
                                                                                refreshed.exe_rel = updated_game.exe_rel.clone();
                                                                                refreshed.api = updated_game.api.clone();
                                                                                refreshed.bitness = updated_game.bitness;
                                                                                if refreshed.poster.is_none() {
                                                                                    refreshed.poster = updated_game.poster.clone();
                                                                                }
                                                                                let s = load_state();
                                                                                if let Some(custom) = s.get_custom_name(&updated_game.dir) {
                                                                                    refreshed.name = custom.clone();
                                                                                } else if refreshed.name.is_empty() || refreshed.name == "Unknown" {
                                                                                    refreshed.name = updated_game.name.clone();
                                                                                }
                                                                                updated_game = refreshed;
                                                                            }

                                                                            if cur_eff_backend == "optiscaler" {
                                                                                updated_game.optiscaler_installed = true;
                                                                                updated_game.optiscaler_presr = opti_pre_sr_val;
                                                                                updated_game.optiscaler_passes = opti_passes_val;
                                                                                updated_game.mfg_unlock_installed = mfg_choice_val;
                                                                                updated_game.addon_installed = false;
                                                                                updated_game.reshade_installed = false;
                                                                                updated_game.installed_route = Some("optiscaler".to_string());
                                                                            } else if cur_eff_route == "native" {
                                                                                updated_game.optiscaler_installed = false;
                                                                                updated_game.optiscaler_presr = false;
                                                                                updated_game.optiscaler_passes = 1;
                                                                                updated_game.mfg_unlock_installed = mfg_choice_val;
                                                                                updated_game.addon_installed = true;
                                                                                updated_game.reshade_installed = true;
                                                                                updated_game.installed_route = Some("native".to_string());
                                                                            } else {
                                                                                updated_game.optiscaler_installed = false;
                                                                                updated_game.optiscaler_presr = false;
                                                                                updated_game.optiscaler_passes = 1;
                                                                                updated_game.mfg_unlock_installed = mfg_choice_val;
                                                                                updated_game.addon_installed = true;
                                                                                updated_game.reshade_installed = true;
                                                                                updated_game.installed_route = Some("feeder".to_string());
                                                                            }
                                                                            updated_game.nr_style = nr_style_val;
                                                                            updated_game.nr_style_enabled = nr_style_choice_val;
                                                                            updated_game.mfg_multiplier = mfg_multiplier_val;
                                                                            updated_game.has_backup = true;

                                                                            let mut current_games = games.read().clone();
                                                                            if let Some(pos) = current_games.iter().position(|g| g.dir == updated_game.dir) {
                                                                                current_games[pos] = updated_game.clone();
                                                                            }
                                                                            games.set(current_games.clone());

                                                                            let mut s = load_state();
                                                                            s.cached_games = current_games;
                                                                            let _ = save_state(&s);
                                                                        }
                                                                        Ok(Err(err)) => {
                                                                            lines.push(format!("[ERROR] Deployment failed: {}", err));
                                                                        }
                                                                        Err(join_err) => {
                                                                            lines.push(format!("[ERROR] Task execution failed: {}", join_err));
                                                                        }
                                                                    }
                                                                    job_lines.set(lines);
                                                                    is_busy.set(false);
                                                                });
                                                            }
                                                        },
                                                        {
                                                             if *is_busy.read() {
                                                                 crate::core::i18n::t(&current_lang.read(), "btn_working").to_string()
                                                             } else if has_override {
                                                                 crate::core::i18n::t(&current_lang.read(), "btn_deploy_override").to_string()
                                                             } else if is_deployed {
                                                                 crate::core::i18n::t(&current_lang.read(), "sheet_deploy").to_string()
                                                             } else {
                                                                 crate::core::i18n::t(&current_lang.read(), "install").to_string()
                                                             }
                                                         }
                                                    }

                                                    if target_game.has_backup {
                                                        button {
                                                            class: "btn-restore",
                                                            id: "doRestore",
                                                            disabled: *is_busy.read(),
                                                            onclick: {
                                                                let target_game = target_game.clone();
                                                                move |_| {
                                                                    if *is_busy.read() {
                                                                        return;
                                                                    }
                                                                    is_busy.set(true);
                                                                    let target_game = target_game.clone();

                                                                    dioxus::prelude::spawn(async move {
                                                                        let mut lines = Vec::new();
                                                                        let gd = target_game.dir.clone();
                                                                        let ep = target_game.exe_path.clone();

                                                                        let closed_check = tokio::task::spawn_blocking({
                                                                            let gd = gd.clone();
                                                                            let ep = ep.clone();
                                                                            move || crate::core::install_guards::assert_game_closed(&gd, Some(&ep))
                                                                        }).await;

                                                                        if let Ok(Err(e)) = closed_check {
                                                                            lines.push(format!("[ERROR] {}", e));
                                                                            job_lines.set(lines);
                                                                            is_busy.set(false);
                                                                            return;
                                                                        }

                                                                        lines.push(format!("@{{log_restore_start|{}}}", target_game.name));
                                                                        job_lines.set(lines.clone());

                                                                        let res = tokio::task::spawn_blocking({
                                                                            let gd = gd.clone();
                                                                            move || restore_game(&gd)
                                                                        }).await;

                                                                        match res {
                                                                            Ok(Ok(true)) => {
                                                                                lines.push("@{log_restore_success}".to_string());
                                                                                let _ = append_history(&HistoryRow {
                                                                                    date: crate::core::journal::now_timestamp_str(),
                                                                                    dir: target_game.dir.to_string_lossy().to_string(),
                                                                                    game_name: Some(target_game.name.clone()),
                                                                                    action: "restore".to_string(),
                                                                                    replaced: 0,
                                                                                    added: 0,
                                                                                });
                                                                                touch(&target_game.dir.to_string_lossy());

                                                                                let refreshed_opt = tokio::task::spawn_blocking({
                                                                                    let gd = gd.clone();
                                                                                    move || crate::core::scan::scan_game_directory(&gd)
                                                                                }).await.ok().flatten();

                                                                                let mut updated_game = target_game.clone();
                                                                                if let Some(mut refreshed) = refreshed_opt {
                                                                                    refreshed.launcher = updated_game.launcher.clone();
                                                                                    if refreshed.poster.is_none() {
                                                                                        refreshed.poster = updated_game.poster.clone();
                                                                                    }
                                                                                    let s = load_state();
                                                                                    if let Some(custom) = s.get_custom_name(&updated_game.dir) {
                                                                                        refreshed.name = custom.clone();
                                                                                    } else if refreshed.name.is_empty() || refreshed.name == "Unknown" {
                                                                                        refreshed.name = updated_game.name.clone();
                                                                                    }
                                                                                    updated_game = refreshed;
                                                                                } else {
                                                                                    updated_game.optiscaler_installed = false;
                                                                                    updated_game.optiscaler_presr = false;
                                                                                    updated_game.optiscaler_passes = 1;
                                                                                    updated_game.mfg_unlock_installed = false;
                                                                                    updated_game.reshade_installed = false;
                                                                                    updated_game.addon_installed = false;
                                                                                    updated_game.installed_route = None;
                                                                                    updated_game.has_backup = false;
                                                                                }

                                                                                let mut current_games = games.read().clone();
                                                                                if let Some(pos) = current_games.iter().position(|g| g.dir == updated_game.dir) {
                                                                                    current_games[pos] = updated_game.clone();
                                                                                }
                                                                                games.set(current_games.clone());

                                                                                let mut s = load_state();
                                                                                s.cached_games = current_games;
                                                                                let _ = save_state(&s);
                                                                            }
                                                                            _ => {
                                                                                lines.push("@{log_restore_no_backup}".to_string());
                                                                            }
                                                                        }
                                                                        job_lines.set(lines);
                                                                        is_busy.set(false);
                                                                    });
                                                                }
                                                            },
                                                            "{crate::core::i18n::t(&current_lang.read(), \"restore\")}"
                                                        }
                                                    } else if is_deployed {
                                                        button {
                                                            class: "btn-restore",
                                                            id: "doClean",
                                                            disabled: *is_busy.read(),
                                                            onclick: {
                                                                let target_game = target_game.clone();
                                                                move |_| {
                                                                    if *is_busy.read() {
                                                                        return;
                                                                    }
                                                                    is_busy.set(true);
                                                                    let target_game = target_game.clone();

                                                                    dioxus::prelude::spawn(async move {
                                                                        let mut lines = Vec::new();
                                                                        let gd = target_game.dir.clone();
                                                                        let ep = target_game.exe_path.clone();

                                                                        let closed_check = tokio::task::spawn_blocking({
                                                                            let gd = gd.clone();
                                                                            let ep = ep.clone();
                                                                            move || crate::core::install_guards::assert_game_closed(&gd, Some(&ep))
                                                                        }).await;

                                                                        if let Ok(Err(e)) = closed_check {
                                                                            lines.push(format!("[ERROR] {}", e));
                                                                            job_lines.set(lines);
                                                                            is_busy.set(false);
                                                                            return;
                                                                        }

                                                                        lines.push(format!("@{{log_clean_start|{}}}", target_game.name));
                                                                        job_lines.set(lines.clone());

                                                                        let res = tokio::task::spawn_blocking({
                                                                            let gd = gd.clone();
                                                                            let ep = ep.clone();
                                                                            move || crate::core::journal::clean_untracked_mods_with_exe(&gd, Some(&ep))
                                                                        }).await;

                                                                        match res {
                                                                            Ok(Ok(removed)) => {
                                                                                for r in &removed {
                                                                                    lines.push(format!("@{{log_clean_removed|{}}}", r));
                                                                                }
                                                                                lines.push(format!("@{{log_clean_purged|{}}}", removed.len()));
                                                                                let _ = append_history(&HistoryRow {
                                                                                    date: crate::core::journal::now_timestamp_str(),
                                                                                    dir: target_game.dir.to_string_lossy().to_string(),
                                                                                    game_name: Some(target_game.name.clone()),
                                                                                    action: "clean".to_string(),
                                                                                    replaced: 0,
                                                                                    added: 0,
                                                                                });
                                                                                touch(&target_game.dir.to_string_lossy());

                                                                                let refreshed_opt = tokio::task::spawn_blocking({
                                                                                    let gd = gd.clone();
                                                                                    move || crate::core::scan::scan_game_directory(&gd)
                                                                                }).await.ok().flatten();

                                                                                let mut updated_game = target_game.clone();
                                                                                if let Some(mut refreshed) = refreshed_opt {
                                                                                    refreshed.launcher = updated_game.launcher.clone();
                                                                                    if refreshed.poster.is_none() {
                                                                                        refreshed.poster = updated_game.poster.clone();
                                                                                    }
                                                                                    let s = load_state();
                                                                                    if let Some(custom) = s.get_custom_name(&updated_game.dir) {
                                                                                        refreshed.name = custom.clone();
                                                                                    } else if refreshed.name.is_empty() || refreshed.name == "Unknown" {
                                                                                        refreshed.name = updated_game.name.clone();
                                                                                    }
                                                                                    updated_game = refreshed;
                                                                                } else {
                                                                                    updated_game.optiscaler_installed = false;
                                                                                    updated_game.reshade_installed = false;
                                                                                    updated_game.addon_installed = false;
                                                                                    updated_game.installed_route = None;
                                                                                    updated_game.has_backup = false;
                                                                                }

                                                                                let mut current_games = games.read().clone();
                                                                                if let Some(pos) = current_games.iter().position(|g| g.dir == updated_game.dir) {
                                                                                    current_games[pos] = updated_game.clone();
                                                                                }
                                                                                games.set(current_games.clone());

                                                                                let mut s = load_state();
                                                                                s.cached_games = current_games;
                                                                                let _ = save_state(&s);
                                                                            }
                                                                            Ok(Err(e)) => {
                                                                                lines.push(format!("[ERROR] Failed to clean mods: {}", e));
                                                                            }
                                                                            Err(je) => {
                                                                                lines.push(format!("[ERROR] Task execution failed: {}", je));
                                                                            }
                                                                        }
                                                                        job_lines.set(lines);
                                                                        is_busy.set(false);
                                                                    });
                                                                }
                                                            },
                                                            "{crate::core::i18n::t(&current_lang.read(), \"btn_clean_mods\")}"
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                // Terminal Log
                                div { class: "job-terminal", style: "margin-top:16px;",
                                    div { class: "job-head", style: "display:flex; justify-content:space-between; margin-bottom:6px;",
                                        span { class: "dim", "{crate::core::i18n::t(&current_lang.read(), \"btn_execution_log\")}" }
                                        button {
                                            class: "ghost sm",
                                            onclick: move |_| {
                                                let lang = current_lang.read().clone();
                                                let text = job_lines.read()
                                                    .iter()
                                                    .map(|l| crate::core::i18n::format_log_entry(&lang, l))
                                                    .collect::<Vec<String>>()
                                                    .join("\r\n");
                                                copy_to_clipboard(&text);
                                                copy_toast_text.set(crate::core::i18n::t(&current_lang.read(), "toast_activity_copied").to_string());
                                                *toast_generation.write() += 1;
                                                copy_toast.set(true);
                                            },
                                            "{crate::core::i18n::t(&current_lang.read(), \"btn_copy_log\")}"
                                        }
                                    }
                                    div { class: "job-body", style: "font-family:monospace; font-size:0.85em; background:rgba(0,0,0,0.4); padding:10px; border-radius:6px; max-height:120px; overflow-y:auto;",
                                        for line in job_lines.read().iter() {
                                            {
                                                let rendered_line = crate::core::i18n::format_log_entry(&current_lang.read(), line);
                                                rsx! {
                                                    div { "{rendered_line}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

                        // ---------------- OVERLAY HOTKEY DIALOG ----------------
            if *overlay_hotkey_open.read() {
                div {
                    class: "overlay",
                    style: "display:flex; justify-content:center; align-items:center; z-index:9999;",
                    onmousedown: move |e| e.stop_propagation(),
                    onclick: move |_| overlay_hotkey_open.set(false),
                    div {
                        class: "glass pad",
                        style: "width: 360px; max-width:90%; background: #181b20; border: 1px solid var(--line); border-radius: 8px; padding: 20px;",
                        onmousedown: move |e| e.stop_propagation(),
                        onclick: move |e| e.stop_propagation(),
                        h4 { style: "margin-top:0; margin-bottom: 8px;", "{crate::core::i18n::t(&current_lang.read(), \"hotkey_dialog_title\")}" }
                        p { class: "hint", style: "margin-bottom:16px; font-size:0.85em;", "Select the shortcut to toggle HUD in-game:" }
                        div { style: "display:grid; grid-template-columns: 1fr 1fr; gap:8px; margin-bottom:16px;",
                            for hk in &["F8", "F9", "F10", "F11", "Ctrl+Shift+O", "Alt+`"] {
                                {
                                    let key_str = hk.to_string();
                                    let is_active = *overlay_hotkey.read() == key_str;
                                    rsx! {
                                        button {
                                            class: if is_active { "glass-btn" } else { "ghost sm" },
                                            style: "padding: 8px;",
                                            onclick: {
                                                let k = key_str.clone();
                                                move |_| {
                                                    overlay_hotkey.set(k.clone());
                                                    let mut s = load_state();
                                                    s.overlay_hotkey = k.clone();
                                                    let _ = save_state(&s);
                                                    crate::core::state::sync_overlay_preferences(&s);
                                                    overlay_hotkey_open.set(false);
                                                }
                                            },
                                            "{key_str}"
                                        }
                                    }
                                }
                            }
                        }
                        div { style: "text-align:right;",
                            button { class: "ghost sm", onclick: move |_| overlay_hotkey_open.set(false), "{crate::core::i18n::t(&current_lang.read(), \"btn_close\")}" }
                        }
                    }
                }
            }

            // ---------------- OVERLAY CREATE THEME DIALOG ----------------
            if *overlay_create_open.read() {
                div {
                    class: "overlay",
                    style: "display:flex; justify-content:center; align-items:center; z-index:9999;",
                    onmousedown: move |e| e.stop_propagation(),
                    onclick: move |_| overlay_create_open.set(false),
                    div {
                        class: "glass pad",
                        style: "width: 400px; max-width:90%; background: #181b20; border: 1px solid var(--line); border-radius: 8px; padding: 24px;",
                        onmousedown: move |e| e.stop_propagation(),
                        onclick: move |e| e.stop_propagation(),
                        h4 { style: "margin-top:0; margin-bottom: 6px;", "{crate::core::i18n::t(&current_lang.read(), \"theme_create_title\")}" }
                        p { class: "hint", style: "margin-bottom:16px; font-size:0.85em;", "{crate::core::i18n::t(&current_lang.read(), \"theme_create_hint\")}" }
                        div { style: "margin-bottom:12px;",
                            label { style: "display:block; font-size:0.85em; font-weight:600; margin-bottom:4px;", "{crate::core::i18n::t(&current_lang.read(), \"theme_create_name\")}" }
                            input {
                                style: "width:100%; padding:8px 12px; background:rgba(0,0,0,0.3); border:1px solid var(--line); border-radius:6px; color:var(--text); box-sizing:border-box;",
                                value: "{custom_theme_name.read()}",
                                oninput: move |e| custom_theme_name.set(e.value())
                            }
                        }
                        div { style: "margin-bottom:16px;",
                            label { style: "display:block; font-size:0.85em; font-weight:600; margin-bottom:4px;", "{crate::core::i18n::t(&current_lang.read(), \"theme_create_color\")}" }
                            div { style: "display:flex; gap:10px; align-items:center;",
                                input {
                                    type: "color",
                                    style: "width:44px; height:36px; padding:0; border:none; border-radius:4px; background:transparent; cursor:pointer;",
                                    value: "{custom_theme_color.read()}",
                                    oninput: move |e| custom_theme_color.set(e.value())
                                }
                                span { style: "font-family:monospace; font-weight:600;", "{custom_theme_color.read()}" }
                            }
                        }
                        // Real-time Swatch Preview
                        div {
                            style: "background:rgba(0,0,0,0.4); border-left: 4px solid {custom_theme_color.read()}; padding: 12px; border-radius: 4px; margin-bottom: 20px;",
                            div { style: "display:flex; justify-content:space-between; font-size:0.9em;",
                                b { "{custom_theme_name.read()}" }
                                span { style: "color:{custom_theme_color.read()}; font-family:monospace; font-weight:bold;", "120 FPS" }
                            }
                            span { class: "dim", style: "font-size:0.8em;", "{crate::core::i18n::t(&current_lang.read(), \"theme_preview_simulated\")}" }
                        }
                        div { style: "display:flex; justify-content:flex-end; gap:8px;",
                            button {
                                class: "ghost sm",
                                onclick: move |_| overlay_create_open.set(false),
                                "{crate::core::i18n::t(&current_lang.read(), \"btn_cancel\")}"
                            }
                            button {
                                class: "glass-btn sm",
                                onclick: move |_| {
                                    let name = if custom_theme_name.read().trim().is_empty() {
                                        "My Theme".to_string()
                                    } else {
                                        custom_theme_name.read().trim().to_string()
                                    };
                                    let color = custom_theme_color.read().clone();
                                    let new_id = format!("custom_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0));
                                    let new_theme = crate::core::state::CustomOverlayTheme {
                                        id: new_id.clone(),
                                        name,
                                        color,
                                    };
                                    let mut list = custom_overlay_themes.read().clone();
                                    list.push(new_theme);
                                    custom_overlay_themes.set(list.clone());
                                    overlay_theme.set(new_id.clone());
                                    let mut s = load_state();
                                    s.custom_overlay_themes = list;
                                    s.overlay_theme = new_id;
                                    let _ = save_state(&s);
                                    crate::core::state::sync_overlay_preferences(&s);
                                    overlay_create_open.set(false);
                                },
                                "{crate::core::i18n::t(&current_lang.read(), \"btn_save_apply\")}"
                            }
                        }
                    }
                }
            }

            // ---------------- OVERLAY INTERACTIVE PREVIEW MODAL ----------------
            if *overlay_preview_open.read() {
                {
                    let p_id = preview_theme_id.read().clone();
                    let custom_themes = custom_overlay_themes.read().clone();
                    let (p_name, p_accent, p_soft, p_back, p_bright, p_attr) = match p_id.as_str() {
                        "blue" | "azure" => ("Azure".to_string(), "#4aa8ee".to_string(), "#4aa8ee25".to_string(), "#0d1724".to_string(), "#91d1ff".to_string(), "blue".to_string()),
                        "purple" | "amethyst" => ("Amethyst".to_string(), "#b45dea".to_string(), "#b45dea25".to_string(), "#1b1127".to_string(), "#ddb0ff".to_string(), "purple".to_string()),
                        "green" | "emerald" => ("Emerald".to_string(), "#8bc400".to_string(), "#8bc40025".to_string(), "#111a10".to_string(), "#c2ec66".to_string(), "green".to_string()),
                        custom_id => {
                            if let Some(ct) = custom_themes.iter().find(|c| c.id == custom_id) {
                                (ct.name.clone(), ct.color.clone(), format!("{}25", ct.color), "#15181e".to_string(), ct.color.clone(), "custom".to_string())
                            } else {
                                ("Emerald".to_string(), "#8bc400".to_string(), "#8bc40025".to_string(), "#111a10".to_string(), "#c2ec66".to_string(), "green".to_string())
                            }
                        }
                    };

                    let p_id_apply = p_id.clone();

                    rsx! {
                        div {
                            class: "overlay",
                            style: "display:flex; justify-content:center; align-items:center; z-index:9999; overflow-y:auto; padding:20px 0;",
                            onmousedown: move |e| e.stop_propagation(),
                            onclick: move |_| overlay_preview_open.set(false),
                            div {
                                class: "ol-dialog",
                                style: format!("--ol-accent:{}; --ol-soft:{}; --ol-back:{}; --ol-bright:{}; margin:auto;", p_accent, p_soft, p_back, p_bright),
                                "data-overlay-theme": "{p_attr}",
                                onmousedown: move |e| e.stop_propagation(),
                                onclick: move |e| e.stop_propagation(),
                                div { class: "ol-dialog-head",
                                    h3 { "{crate::core::i18n::t(&current_lang.read(), \"preview_dialog_title\")} • {p_name}" }
                                    button { class: "ghost sm", onclick: move |_| overlay_preview_open.set(false), "✕" }
                                }
                                p { "Interactive RenoDX compact overlay in native resolution (534px). Toggle in-game with {overlay_hotkey.read()}." }
                                div { style: "display:flex; justify-content:center; margin: 16px 0; direction:ltr;",
                                    div { class: "ol-panel", style: "width: 534px; max-width: 100%; box-sizing: border-box;",
                                        header {
                                            div { class: "ol-eyebrow", "DLSS 5 STUDIO CONTROLS" }
                                            span { class: "ol-prototype", "{crate::core::i18n::t(&current_lang.read(), \"preview_interactive_tag\")}" }
                                        }
                                        section { class: "ol-master",
                                            label { class: "ol-check",
                                                input {
                                                    type: "checkbox",
                                                    checked: *prev_dlss_on.read(),
                                                    onchange: move |_| {
                                                        let cur = *prev_dlss_on.read();
                                                        prev_dlss_on.set(!cur);
                                                    }
                                                }
                                                span { "{crate::core::i18n::t(&current_lang.read(), \"preview_dlss_on\")}" }
                                            }
                                        }
                                        section {
                                            h4 { "{crate::core::i18n::t(&current_lang.read(), \"preview_global_controls\")}" }
                                            div { class: "ol-slider",
                                                span { "{crate::core::i18n::t(&current_lang.read(), \"preview_structure_intensity\")}" }
                                                input {
                                                    type: "range", min: "0", max: "2", step: "0.05",
                                                    value: "{prev_structure.read()}",
                                                    oninput: move |e| if let Ok(v) = e.value().parse::<f32>() { prev_structure.set(v); }
                                                }
                                                output { "{prev_structure.read():.2}" }
                                            }
                                            div { class: "ol-slider",
                                                span { "{crate::core::i18n::t(&current_lang.read(), \"preview_tone_intensity\")}" }
                                                input {
                                                    type: "range", min: "0", max: "2", step: "0.05",
                                                    value: "{prev_tone.read()}",
                                                    oninput: move |e| if let Ok(v) = e.value().parse::<f32>() { prev_tone.set(v); }
                                                }
                                                output { "{prev_tone.read():.2}" }
                                            }
                                        }
                                        section { class: "ol-group",
                                            label { class: "ol-check",
                                                input {
                                                    type: "checkbox",
                                                    checked: *prev_char_mask.read(),
                                                    onchange: move |_| {
                                                        let cur = *prev_char_mask.read();
                                                        prev_char_mask.set(!cur);
                                                    }
                                                }
                                                span { "{crate::core::i18n::t(&current_lang.read(), \"preview_char_mask\")}" }
                                            }
                                            div { class: "ol-slider",
                                                span { "{crate::core::i18n::t(&current_lang.read(), \"preview_char_structure\")}" }
                                                input {
                                                    type: "range", min: "0", max: "2", step: "0.05",
                                                    value: "{prev_char_structure.read()}",
                                                    oninput: move |e| if let Ok(v) = e.value().parse::<f32>() { prev_char_structure.set(v); }
                                                }
                                                output { "{prev_char_structure.read():.2}" }
                                            }
                                        }
                                        section {
                                            h4 { "{crate::core::i18n::t(&current_lang.read(), \"preview_nr_style\")}" }
                                            div { class: "ol-models",
                                                button {
                                                    class: if *prev_nr_style.read() == 0 { "ol-model selected" } else { "ol-model" },
                                                    onclick: move |_| prev_nr_style.set(0),
                                                    "{crate::core::i18n::t(&current_lang.read(), \"preview_model_a\")}"
                                                }
                                                button {
                                                    class: if *prev_nr_style.read() == 1 { "ol-model selected" } else { "ol-model" },
                                                    onclick: move |_| prev_nr_style.set(1),
                                                    "{crate::core::i18n::t(&current_lang.read(), \"preview_model_b\")}"
                                                }
                                                button {
                                                    class: if *prev_nr_style.read() == 2 { "ol-model selected" } else { "ol-model" },
                                                    onclick: move |_| prev_nr_style.set(2),
                                                    "{crate::core::i18n::t(&current_lang.read(), \"preview_model_c\")}"
                                                }
                                            }
                                        }
                                        section {
                                            h4 { "{crate::core::i18n::t(&current_lang.read(), \"preview_more_controls\")}" }
                                            div { class: "ol-slider",
                                                span { "{crate::core::i18n::t(&current_lang.read(), \"preview_overall_intensity\")}" }
                                                input {
                                                    type: "range", min: "0", max: "2", step: "0.05",
                                                    value: "{prev_overall_intensity.read()}",
                                                    oninput: move |e| if let Ok(v) = e.value().parse::<f32>() { prev_overall_intensity.set(v); }
                                                }
                                                output { "{prev_overall_intensity.read():.2}" }
                                            }
                                            div { class: "ol-slider",
                                                span { "{crate::core::i18n::t(&current_lang.read(), \"preview_local_tone\")}" }
                                                input {
                                                    type: "range", min: "0", max: "2", step: "0.05",
                                                    value: "{prev_local_tone.read()}",
                                                    oninput: move |e| if let Ok(v) = e.value().parse::<f32>() { prev_local_tone.set(v); }
                                                }
                                                output { "{prev_local_tone.read():.2}" }
                                            }
                                            div { class: "ol-slider",
                                                span { "{crate::core::i18n::t(&current_lang.read(), \"preview_diffuse_white\")}" }
                                                input {
                                                    type: "range", min: "80", max: "1000", step: "1",
                                                    value: "{prev_diffuse_white.read()}",
                                                    oninput: move |e| if let Ok(v) = e.value().parse::<f32>() { prev_diffuse_white.set(v); }
                                                }
                                                output { "{prev_diffuse_white.read():.0} nits" }
                                            }
                                            div { class: "ol-slider",
                                                span { "{crate::core::i18n::t(&current_lang.read(), \"preview_motion_x\")}" }
                                                input {
                                                    type: "range", min: "0", max: "2", step: "0.05",
                                                    value: "{prev_motion_x.read()}",
                                                    oninput: move |e| if let Ok(v) = e.value().parse::<f32>() { prev_motion_x.set(v); }
                                                }
                                                output { "{prev_motion_x.read():.2}" }
                                            }
                                            div { class: "ol-slider",
                                                span { "{crate::core::i18n::t(&current_lang.read(), \"preview_motion_y\")}" }
                                                input {
                                                    type: "range", min: "0", max: "2", step: "0.05",
                                                    value: "{prev_motion_y.read()}",
                                                    oninput: move |e| if let Ok(v) = e.value().parse::<f32>() { prev_motion_y.set(v); }
                                                }
                                                output { "{prev_motion_y.read():.2}" }
                                            }
                                        }
                                        footer { "{crate::core::i18n::t(&current_lang.read(), \"preview_footer\")}" }
                                    }
                                }
                                div { style: "display:flex; justify-content:flex-end; gap:8px; margin-top:12px;",
                                    button {
                                        class: "ghost sm",
                                        onclick: move |_| overlay_preview_open.set(false),
                                        "{crate::core::i18n::t(&current_lang.read(), \"btn_close\")}"
                                    }
                                    button {
                                        class: "glass-btn sm",
                                        onclick: move |_| {
                                            overlay_theme.set(p_id_apply.clone());
                                            let mut s = load_state();
                                            s.overlay_theme = p_id_apply.clone();
                                            let _ = save_state(&s);
                                            crate::core::state::sync_overlay_preferences(&s);
                                            overlay_preview_open.set(false);
                                        },
                                        "{crate::core::i18n::t(&current_lang.read(), \"btn_apply_theme\")}"
                                    }
                                }
                            }
                        }
                    }
                }
            }


            // Toast feedback
            if *copy_toast.read() {
                div {
                    class: "copy-feedback toast-anim",
                    title: "{crate::core::i18n::t(&current_lang.read(), \"tooltip_dismiss\")}",
                    onclick: move |_| copy_toast.set(false),
                    "{copy_toast_text}"
                }
            }

            // Big Picture 10-Foot TV & Gamepad Overlay
            if *big_picture_open.read() {
                crate::big_picture::BigPictureOverlay {
                    games: games,
                    is_open: big_picture_open,
                    rust_theme: rust_theme,
                    theme: theme,
                    lang: current_lang,
                }
            }
        }
    }
}

pub fn launch_game(game: &GameEntry) {
    touch(&game.dir.to_string_lossy());
    if game.is_dlss5_patched() {
        log_message(&format!("@{{log_game_launched_modded|{}|{}}}", game.name, game.route_display_name()));
    } else {
        log_message(&format!("@{{log_game_launched_vanilla|{}}}", game.name));
    }
    let target = if game.exe_path.exists() && game.exe_path.is_file() {
        game.exe_path.clone()
    } else {
        game.dir.clone()
    };

    if target.is_file() {
        let parent = target.parent().unwrap_or(&game.dir);
        let res = std::process::Command::new(&target)
            .current_dir(parent)
            .spawn();
        if res.is_err() {
            let _ = std::process::Command::new("explorer")
                .arg(&target)
                .spawn();
        }
    } else {
        let _ = std::process::Command::new("explorer")
            .arg(&game.dir)
            .spawn();
    }
}

pub fn pick_and_set_cover(dir: &std::path::Path, mut games: Signal<Vec<GameEntry>>) {
    let dir = dir.to_path_buf();
    spawn(async move {
        if let Some(handle) = rfd::AsyncFileDialog::new()
            .set_title("Select Cover Image")
            .add_filter("Images (*.jpg, *.png, *.webp)", &["jpg", "jpeg", "png", "webp", "bmp"])
            .pick_file()
            .await
        {
            let file_path = handle.path().to_path_buf();
            if let Ok(bytes) = std::fs::read(&file_path) {
                let art_dir = crate::core::state::get_appdata_dir().join("art");
                let _ = std::fs::create_dir_all(&art_dir);
                let key = crate::core::steamart::key_for_dir(&dir);
                let cover_file = art_dir.join(format!("{}-cover.jpg", key));
                let _ = crate::core::steamart::optimize_and_save_cover_bytes(&bytes, &cover_file);
                let uri = format!("http://dlss-art.localhost/art/{}-cover.jpg", key);

                // Update games signal so UI refreshes immediately
                let mut cur = games.read().clone();
                for g in cur.iter_mut() {
                    if g.dir == dir {
                        g.poster = Some(uri.clone());
                    }
                }
                games.set(cur);

                // Update persistent cached_games in state
                let mut s = load_state();
                for g in s.cached_games.iter_mut() {
                    if g.dir == dir {
                        g.poster = Some(uri.clone());
                    }
                }
                let _ = save_state(&s);
                log_message(&format!("@{{log_custom_cover_updated|{}}}", dir.file_name().unwrap_or_default().to_string_lossy()));
            }
        }
    });
}
