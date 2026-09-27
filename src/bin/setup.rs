#![windows_subsystem = "windows"]

use base64::Engine;
use dioxus::desktop::tao::window::Icon as TaoIcon;
use dioxus::desktop::{Config, LogicalSize, WindowBuilder};
use dioxus::prelude::*;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use dlss_studio::core::i18n;

fn detect_initial_language() -> String {
    // 1. Check existing library.json in existing installation directory (if update/reinstall)
    if let Some((ref install_dir, _, ref custom_storage)) = detect_existing_installation() {
        if let Some(ref s) = custom_storage {
            let p = Path::new(s).join("library.json");
            if let Ok(content) = std::fs::read_to_string(&p) {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(lang) = val.get("lang").and_then(|l| l.as_str()) {
                        if i18n::SUPPORTED_LANGS.iter().any(|l| l.code == lang) {
                            return lang.to_string();
                        }
                    }
                }
            }
        }
        let p = install_dir.join("library.json");
        if let Ok(content) = std::fs::read_to_string(&p) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(lang) = val.get("lang").and_then(|l| l.as_str()) {
                    if i18n::SUPPORTED_LANGS.iter().any(|l| l.code == lang) {
                        return lang.to_string();
                    }
                }
            }
        }
    }

    // 2. Check existing library.json in %APPDATA%\dlss-5-studio\library.json
    if let Ok(appdata) = std::env::var("APPDATA") {
        let p = PathBuf::from(appdata).join("dlss-5-studio").join("library.json");
        if let Ok(content) = std::fs::read_to_string(&p) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(lang) = val.get("lang").and_then(|l| l.as_str()) {
                    if i18n::SUPPORTED_LANGS.iter().any(|l| l.code == lang) {
                        return lang.to_string();
                    }
                }
            }
        }
    }

    // 3. Check %PROGRAMDATA%\dlss-5-studio\library.json
    if let Ok(progdata) = std::env::var("ProgramData") {
        let p = PathBuf::from(progdata).join("dlss-5-studio").join("library.json");
        if let Ok(content) = std::fs::read_to_string(&p) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(lang) = val.get("lang").and_then(|l| l.as_str()) {
                    if i18n::SUPPORTED_LANGS.iter().any(|l| l.code == lang) {
                        return lang.to_string();
                    }
                }
            }
        }
    }

    // 4. First launch / clean setup: detect native Windows UI display language
    i18n::detect_system_language()
}

const CREATE_NO_WINDOW: u32 = 0x08000000;

static PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/installer_payload.bin"));
static APP_ICON_PNG: &[u8] = include_bytes!("../../assets/icon.png");
static UNINSTALL_TARGET: OnceLock<PathBuf> = OnceLock::new();

#[derive(Clone, Copy, PartialEq)]
enum SetupPhase {
    Config,
    Installing,
    Complete,
    Error,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let curr_exe = std::env::current_exe().ok();
    let exe_name = curr_exe
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let is_named_uninstall = exe_name.contains("uninstall");
    let has_uninstall_flag = args.iter().any(|a| a == "--uninstall");
    let is_worker = args.iter().any(|a| a == "--uninstall-worker");
    let is_silent = args.iter().any(|a| a == "--silent" || a == "/qn" || a == "-s");

    let is_uninstall_flow = is_named_uninstall || has_uninstall_flag || is_worker;

    if is_uninstall_flow {
        // Resolve the target directory being uninstalled
        let target_dir: PathBuf = if let Some(pos) = args.iter().position(|a| a == "--uninstall-worker") {
            args.get(pos + 1).map(PathBuf::from).unwrap_or_else(|| {
                detect_existing_installation().map(|(p, _, _)| p).unwrap_or_else(|| PathBuf::from(r"C:\Program Files\DLSS 5 Studio"))
            })
        } else if is_named_uninstall {
            // When uninstall.exe is launched directly, its parent directory is the installation root
            if let Some(p) = curr_exe.as_ref().and_then(|c| c.parent()) {
                p.to_path_buf()
            } else {
                detect_existing_installation().map(|(p, _, _)| p).unwrap_or_else(|| PathBuf::from(r"C:\Program Files\DLSS 5 Studio"))
            }
        } else if let Some((p, _, _)) = detect_existing_installation() {
            p
        } else {
            PathBuf::from(r"C:\Program Files\DLSS 5 Studio")
        };

        // If running directly from within the installation folder, delegate to trampoline worker in %TEMP%
        // so that Windows unlocks uninstall.exe and allows the entire directory to be purged!
        if !is_worker {
            let temp_dir = std::env::temp_dir();
            let pid = std::process::id();
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis())
                .unwrap_or(0);
            let temp_worker = temp_dir.join(format!("dlss_studio_uninstall_{}_{}.exe", pid, timestamp));

            // Also clean up any legacy fixed uninstaller worker in %TEMP%
            let legacy_worker = temp_dir.join("dlss_studio_uninstall.exe");
            if legacy_worker.exists() {
                let _ = std::fs::remove_file(&legacy_worker);
            }

            if let Some(ref curr) = curr_exe {
                if curr != &temp_worker {
                    if let Err(e) = std::fs::copy(curr, &temp_worker) {
                        eprintln!("Failed to copy temporary uninstaller worker: {}", e);
                    }
                }
            }

            let mut cmd = std::process::Command::new(&temp_worker);
            cmd.arg("--uninstall-worker");
            cmd.arg(&target_dir);
            if is_silent {
                cmd.arg("--silent");
            }
            cmd.stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            let _ = cmd.spawn();
            std::process::exit(0);
        }

        // We are the worker running from %TEMP%
        if is_silent {
            let _ = perform_native_uninstall_worker(&target_dir, true);
            std::process::exit(0);
        }

        let _ = UNINSTALL_TARGET.set(target_dir);
    }

    let icon = TaoIcon::from_rgba(include_bytes!("../../assets/icon_64.rgba").to_vec(), 64, 64).ok();

    let window_title = if is_uninstall_flow {
        "DLSS 5 Studio Uninstaller"
    } else {
        "DLSS 5 Studio Setup"
    };

    let mut window = WindowBuilder::new()
        .with_title(window_title)
        .with_decorations(false)
        .with_transparent(true)
        .with_resizable(false)
        .with_inner_size(LogicalSize::new(780.0, 560.0));

    if let Some(ic) = icon {
        window = window.with_window_icon(Some(ic));
    }

    let temp_webview = std::env::temp_dir().join("dlss_studio_setup_webview");
    let _ = std::fs::create_dir_all(&temp_webview);
    if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_none() {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &temp_webview);
    }

    let cfg = Config::new()
        .with_data_directory(temp_webview)
        .with_window(window)
        .with_custom_head(
            r#"<meta charset="utf-8" />
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; user-select: none; }
  body {
    background: transparent;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, "Helvetica Neue", sans-serif;
    color: #e5e7eb;
    overflow: hidden;
    height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 16px;
  }
  .app-bg-wrapper {
    position: fixed;
    inset: 0;
    z-index: 0;
    cursor: default;
    background:
      radial-gradient(ellipse at 50% 30%, rgba(45, 25, 16, 0.75) 0%, rgba(14, 10, 8, 0.94) 80%),
      linear-gradient(135deg, #18110c 0%, #0d0907 100%);
  }
  .app-bg-wrapper::after {
    content: "";
    position: absolute;
    inset: 0;
    opacity: 0.18;
    background-image: radial-gradient(#d97706 0.75px, transparent 0.75px);
    background-size: 16px 16px;
    pointer-events: none;
  }
  .setup-card {
    position: relative;
    z-index: 1;
    width: 620px;
    max-width: 90vw;
    max-height: 94vh;
    overflow-y: auto;
    overscroll-behavior: contain;
    -webkit-app-region: no-drag !important;
    background: rgba(22, 16, 13, 0.94);
    border: 1px solid rgba(217, 119, 6, 0.65);
    border-radius: 18px;
    padding: 26px 34px;
    box-shadow:
      0 0 28px rgba(217, 119, 6, 0.38),
      0 0 50px rgba(217, 119, 6, 0.16),
      0 24px 60px rgba(0, 0, 0, 0.9),
      inset 0 0 1px rgba(251, 191, 36, 0.45);
    backdrop-filter: blur(28px);
    -webkit-backdrop-filter: blur(28px);
    scrollbar-width: thin;
    scrollbar-color: rgba(217, 119, 6, 0.5) rgba(14, 10, 8, 0.35);
  }
  .setup-card::-webkit-scrollbar {
    width: 7px;
  }
  .setup-card::-webkit-scrollbar-track {
    background: rgba(14, 10, 8, 0.35);
    border-radius: 8px;
    margin: 8px 0;
  }
  .setup-card::-webkit-scrollbar-thumb {
    background: rgba(217, 119, 6, 0.45);
    border-radius: 8px;
    border: 1px solid rgba(251, 191, 36, 0.2);
    transition: background 0.15s ease, border-color 0.15s ease;
  }
  .setup-card::-webkit-scrollbar-thumb:hover {
    background: rgba(245, 158, 11, 0.8);
    border-color: rgba(251, 191, 36, 0.6);
    box-shadow: 0 0 8px rgba(245, 158, 11, 0.4);
  }
  .setup-card::-webkit-scrollbar-corner {
    background: transparent;
  }
  .drag-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 24px;
    cursor: default;
    user-select: none;
  }
  .no-drag {
    -webkit-app-region: no-drag;
  }
  .title-group {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .app-badge {
    width: 44px;
    height: 44px;
    border-radius: 10px;
    object-fit: cover;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
    border: 1px solid rgba(217, 119, 6, 0.4);
    flex-shrink: 0;
  }
  .app-title {
    font-size: 21px;
    font-weight: 700;
    letter-spacing: 0.04em;
    color: #f9fafb;
    text-shadow: 0 2px 8px rgba(0, 0, 0, 0.6);
  }
  .header-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    position: relative;
    z-index: 100;
  }
  .lang-wrap {
    position: relative;
  }
  .lang-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(30, 20, 16, 0.7);
    border: 1px solid rgba(217, 119, 6, 0.35);
    border-radius: 8px;
    padding: 6px 11px;
    color: #f3f4f6;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
  }
  .lang-btn:hover {
    border-color: #d97706;
    background: rgba(217, 119, 6, 0.25);
  }
  .lang-btn svg {
    width: 14px;
    height: 14px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
  }
  .lang-menu {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 200;
    width: 190px;
    max-height: 260px;
    overflow-y: auto;
    padding: 6px;
    border-radius: 12px;
    background: rgba(26, 18, 14, 0.96);
    border: 1px solid rgba(217, 119, 6, 0.5);
    box-shadow: 0 16px 36px rgba(0, 0, 0, 0.75), 0 0 16px rgba(217, 119, 6, 0.2);
    backdrop-filter: blur(20px);
    scrollbar-width: thin;
    scrollbar-color: rgba(217, 119, 6, 0.5) rgba(14, 10, 8, 0.35);
  }
  .lang-menu::-webkit-scrollbar {
    width: 6px;
  }
  .lang-menu::-webkit-scrollbar-thumb {
    background: rgba(217, 119, 6, 0.4);
    border-radius: 6px;
  }
  .lang-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    padding: 7px 10px;
    border: none;
    border-radius: 8px;
    background: transparent;
    color: #e5e7eb;
    font-size: 13px;
    cursor: pointer;
    text-align: left;
    transition: background 0.12s;
  }
  .lang-item:hover {
    background: rgba(217, 119, 6, 0.2);
  }
  .lang-item.active {
    background: rgba(217, 119, 6, 0.3);
    color: #fbbf24;
    font-weight: 600;
  }
  .lang-item .code {
    font-size: 10.5px;
    font-weight: 700;
    color: #9ca3af;
  }
  .lang-item.active .code {
    color: #fbbf24;
  }
  .btn-close-header {
    width: 32px;
    height: 32px;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    color: #9ca3af;
    background: rgba(30, 20, 16, 0.6);
    border: 1px solid rgba(217, 119, 6, 0.25);
    transition: all 0.15s ease;
  }
  .btn-close-header:hover {
    color: #fee2e2;
    background: rgba(239, 68, 68, 0.85);
    border-color: rgba(239, 68, 68, 1);
  }
  .section-label {
    font-size: 13px;
    font-weight: 600;
    color: #d1d5db;
    margin-bottom: 8px;
    letter-spacing: 0.02em;
  }
  .path-row {
    display: flex;
    gap: 10px;
    margin-bottom: 20px;
  }
  .path-input {
    flex: 1;
    background: rgba(14, 10, 8, 0.85);
    border: 1px solid rgba(217, 119, 6, 0.35);
    border-radius: 8px;
    padding: 9px 14px;
    color: #f3f4f6;
    font-size: 13px;
    outline: none;
    transition: border-color 0.2s;
  }
  .path-input:focus {
    border-color: #d97706;
  }
  .path-input[readonly] {
    cursor: default;
  }
  .path-input-warning {
    border-color: rgba(245, 158, 11, 0.75) !important;
    box-shadow: 0 0 10px rgba(245, 158, 11, 0.25);
  }
  .btn-browse {
    background: rgba(45, 25, 16, 0.8);
    border: 1px solid rgba(217, 119, 6, 0.4);
    border-radius: 8px;
    padding: 0 16px;
    color: #f3f4f6;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s;
  }
  .btn-browse:hover {
    background: rgba(217, 119, 6, 0.25);
    border-color: #d97706;
  }
  .prefs-group {
    background: rgba(14, 10, 8, 0.6);
    border: 1px solid rgba(217, 119, 6, 0.2);
    border-radius: 10px;
    padding: 10px 14px;
    margin-bottom: 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .pref-item {
    display: flex;
    align-items: center;
    gap: 10px;
    cursor: pointer;
    font-size: 13px;
    color: #e5e7eb;
  }
  .chk-box {
    width: 18px;
    height: 18px;
    border-radius: 4px;
    border: 1px solid rgba(217, 119, 6, 0.45);
    background: rgba(14, 10, 8, 0.9);
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
  }
  .chk-box.checked {
    background: #d97706;
    border-color: #f59e0b;
  }
  .chk-icon {
    font-size: 11px;
    color: #ffffff;
    display: none;
  }
  .chk-box.checked .chk-icon {
    display: block;
  }
  .advanced-toggle-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 16px;
    cursor: pointer;
    user-select: none;
    width: fit-content;
    padding: 2px 4px;
    border-radius: 4px;
    transition: opacity 0.2s ease;
  }
  .advanced-toggle-row:hover .advanced-toggle-label {
    color: #f59e0b;
  }
  .advanced-chevron {
    font-size: 13px;
    color: #d97706;
    display: inline-block;
    transition: transform 0.2s ease;
  }
  .advanced-chevron.open {
    transform: rotate(90deg);
  }
  .advanced-toggle-label {
    font-size: 13px;
    font-weight: 600;
    color: #d1d5db;
    letter-spacing: 0.02em;
    transition: color 0.2s;
  }
  .advanced-panel {
    background: rgba(14, 10, 8, 0.55);
    border: 1px solid rgba(217, 119, 6, 0.25);
    border-radius: 10px;
    padding: 12px 14px;
    margin-bottom: 20px;
  }
  .helper-text {
    font-size: 11px;
    color: #9ca3af;
    margin-top: 4px;
  }
  .warning-banner {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    background: rgba(245, 158, 11, 0.1);
    border: 1px solid rgba(245, 158, 11, 0.45);
    border-radius: 8px;
    padding: 8px 12px;
    margin-top: 10px;
  }
  .warning-icon {
    font-size: 15px;
    color: #f59e0b;
    line-height: 1.2;
    flex-shrink: 0;
  }
  .warning-msg {
    font-size: 12px;
    line-height: 1.4;
    color: #fbbf24;
  }
  .actions-row {
    display: flex;
    justify-content: flex-end;
    gap: 12px;
  }
  .btn-install {
    background: linear-gradient(135deg, #d97706 0%, #b45309 100%);
    border: 1px solid #f59e0b;
    border-radius: 8px;
    padding: 10px 24px;
    color: #ffffff;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 4px 14px rgba(217, 119, 6, 0.35);
    transition: all 0.2s;
  }
  .btn-install:hover {
    background: linear-gradient(135deg, #f59e0b 0%, #d97706 100%);
    box-shadow: 0 6px 20px rgba(217, 119, 6, 0.5);
    transform: translateY(-1px);
  }
  .btn-danger {
    background: linear-gradient(135deg, #dc2626 0%, #b91c1c 100%);
    border: 1px solid #ef4444;
    border-radius: 8px;
    padding: 10px 24px;
    color: #ffffff;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 4px 14px rgba(220, 38, 38, 0.35);
    transition: all 0.2s;
  }
  .btn-danger:hover {
    background: linear-gradient(135deg, #ef4444 0%, #dc2626 100%);
    box-shadow: 0 6px 20px rgba(239, 68, 68, 0.5);
    transform: translateY(-1px);
  }
  .btn-cancel {
    background: transparent;
    border: 1px solid rgba(156, 163, 175, 0.3);
    border-radius: 8px;
    padding: 10px 20px;
    color: #9ca3af;
    font-size: 14px;
    cursor: pointer;
    transition: all 0.2s;
  }
  .btn-cancel:hover {
    color: #f3f4f6;
    border-color: rgba(209, 213, 219, 0.6);
  }
  .progress-wrap {
    margin: 28px 0;
  }
  .progress-bar-bg {
    width: 100%;
    height: 8px;
    background: rgba(14, 10, 8, 0.9);
    border-radius: 4px;
    overflow: hidden;
    border: 1px solid rgba(217, 119, 6, 0.2);
    margin-bottom: 10px;
  }
  .progress-bar-fill {
    height: 100%;
    background: linear-gradient(90deg, #d97706, #fbbf24);
    box-shadow: 0 0 10px rgba(245, 158, 11, 0.5);
    transition: width 0.3s ease;
  }
  .status-text {
    font-size: 13px;
    color: #d1d5db;
    text-align: center;
  }
</style>
"#.to_string(),
        );

    LaunchBuilder::desktop().with_cfg(cfg).launch(RootApp);
}

fn is_protected_directory<P: AsRef<Path>>(path: P) -> bool {
    let path_str = path.as_ref().to_string_lossy().to_lowercase().replace('/', "\\");
    if let Ok(pf) = std::env::var("ProgramFiles") {
        let pf_lower = pf.to_lowercase().replace('/', "\\");
        if path_str.starts_with(&pf_lower) {
            return true;
        }
    } else if path_str.starts_with(r"c:\program files") {
        return true;
    }

    if let Ok(pf86) = std::env::var("ProgramFiles(x86)") {
        let pf86_lower = pf86.to_lowercase().replace('/', "\\");
        if path_str.starts_with(&pf86_lower) {
            return true;
        }
    } else if path_str.starts_with(r"c:\program files (x86)") {
        return true;
    }

    if let Ok(windir) = std::env::var("SystemRoot") {
        let win_lower = windir.to_lowercase().replace('/', "\\");
        if path_str.starts_with(&win_lower) {
            return true;
        }
    } else if path_str.starts_with(r"c:\windows") {
        return true;
    }

    if path_str.contains(r"\windowsapps") {
        return true;
    }

    false
}

fn compute_default_storage_path(install_dir: &Path) -> String {
    if is_protected_directory(install_dir) {
        if let Ok(pd) = std::env::var("ProgramData") {
            format!("{}\\dlss-5-studio", pd)
        } else {
            r"C:\ProgramData\dlss-5-studio".to_string()
        }
    } else {
        install_dir.join("data").to_string_lossy().to_string()
    }
}

#[component]
fn RootApp() -> Element {
    if let Some(target_dir) = UNINSTALL_TARGET.get() {
        rsx! {
            UninstallApp { target_dir: target_dir.clone() }
        }
    } else {
        rsx! {
            SetupApp {}
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum UninstallPhase {
    Confirm,
    Uninstalling,
    Complete,
    Error,
}

#[component]
fn UninstallApp(target_dir: PathBuf) -> Element {
    let initial_lang = use_hook(detect_initial_language);
    let mut current_lang = use_signal(move || initial_lang);
    let mut lang_menu_open = use_signal(|| false);

    let mut phase = use_signal(|| UninstallPhase::Confirm);
    let mut delete_appdata = use_signal(|| true);
    let mut progress = use_signal(|| 0);
    let mut status_key = use_signal(|| "uninstall_status_preparing".to_string());
    let mut error_msg = use_signal(|| String::new());

    let icon_data_uri = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(APP_ICON_PNG)
    );

    let target_dir_str = target_dir.to_string_lossy().to_string();

    rsx! {
        div {
            class: "app-bg-wrapper",
            onmousedown: move |_| { dioxus::desktop::window().drag(); },
        }
        div {
            class: "setup-card",
            onmousedown: move |e| { e.stop_propagation(); },
            div {
                class: "drag-header",
                onmousedown: move |_| { dioxus::desktop::window().drag(); },
                div { class: "title-group",
                    img { class: "app-badge", src: "{icon_data_uri}" }
                    span { class: "app-title", "DLSS 5 STUDIO" }
                    span {
                        style: "font-size: 11px; font-weight: 700; letter-spacing: 0.08em; color: #ef4444; background: rgba(239, 68, 68, 0.15); border: 1px solid rgba(239, 68, 68, 0.45); border-radius: 6px; padding: 3px 8px; margin-left: 6px;",
                        "{i18n::t(&current_lang.read(), \"setup_tag_uninstall\")}"
                    }
                }
                div { class: "header-actions no-drag",
                    div {
                        class: "lang-wrap",
                        button {
                            class: "lang-btn",
                            title: "Language",
                            onmousedown: move |e| { e.stop_propagation(); },
                            onclick: move |e| {
                                e.stop_propagation();
                                lang_menu_open.toggle();
                            },
                            svg { view_box: "0 0 24 24",
                                circle { cx: "12", cy: "12", r: "10" }
                                path { d: "M2 12h20" }
                                path { d: "M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" }
                            }
                            span { "{current_lang.read().to_uppercase()}" }
                            svg { style: "width: 10px; height: 10px;", view_box: "0 0 24 24",
                                path { d: "M6 9l6 6 6-6" }
                            }
                        }
                        if *lang_menu_open.read() {
                            div {
                                class: "lang-menu",
                                onmousedown: move |e| { e.stop_propagation(); },
                                onclick: move |e| { e.stop_propagation(); },
                                for item in i18n::SUPPORTED_LANGS {
                                    {
                                        let code = item.code;
                                        let is_active = *current_lang.read() == code;
                                        rsx! {
                                            button {
                                                class: if is_active { "lang-item active" } else { "lang-item" },
                                                onmousedown: move |e| { e.stop_propagation(); },
                                                onclick: move |e| {
                                                    e.stop_propagation();
                                                    current_lang.set(code.to_string());
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
                        class: "btn-close-header",
                        title: "{i18n::t(&current_lang.read(), \"setup_btn_close\")}",
                        onmousedown: move |e| { e.stop_propagation(); },
                        onclick: move |e| {
                            e.stop_propagation();
                            schedule_temp_worker_cleanup();
                            dioxus::desktop::window().close();
                        },
                        svg { style: "width: 15px; height: 15px; fill: currentColor;", view_box: "0 0 24 24",
                            path { d: "M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z" }
                        }
                    }
                }
            }

            match phase() {
                UninstallPhase::Confirm => rsx! {
                    div {
                        div {
                            style: "background: rgba(220, 38, 38, 0.1); border: 1px solid rgba(220, 38, 38, 0.45); border-radius: 12px; padding: 14px 18px; margin-bottom: 20px; display: flex; align-items: flex-start; gap: 14px;",
                            span { style: "font-size: 22px; color: #ef4444; line-height: 1;", "⚠" }
                            div {
                                div { style: "font-size: 14px; font-weight: 600; color: #fee2e2; margin-bottom: 4px;",
                                    "{i18n::t(&current_lang.read(), \"uninstall_confirm_title\")}"
                                }
                                div { style: "font-size: 12px; color: #d1d5db; line-height: 1.45;",
                                    "{i18n::t(&current_lang.read(), \"uninstall_confirm_desc\")}"
                                }
                            }
                        }

                        div { class: "section-label", "{i18n::t(&current_lang.read(), \"uninstall_target_label\")}" }
                        div { class: "path-row", style: "margin-bottom: 20px;",
                            input {
                                class: "path-input no-drag",
                                r#type: "text",
                                value: "{target_dir_str}",
                                readonly: true,
                            }
                        }

                        div { class: "section-label", "{i18n::t(&current_lang.read(), \"uninstall_cleanup_opts\")}" }
                        div { class: "prefs-group no-drag", style: "margin-bottom: 28px;",
                            div {
                                class: "pref-item",
                                onclick: move |_| delete_appdata.set(!delete_appdata()),
                                div { class: if delete_appdata() { "chk-box checked" } else { "chk-box" },
                                    span { class: "chk-icon", "✓" }
                                }
                                div {
                                    div { style: "font-weight: 500;", "{i18n::t(&current_lang.read(), \"uninstall_clean_appdata\")}" }
                                    div { class: "helper-text", "{i18n::t(&current_lang.read(), \"uninstall_clean_appdata_desc\")}" }
                                }
                            }
                        }

                        div { class: "actions-row no-drag",
                            button {
                                class: "btn-cancel",
                                onclick: move |_| {
                                    schedule_temp_worker_cleanup();
                                    dioxus::desktop::window().close();
                                },
                                "{i18n::t(&current_lang.read(), \"uninstall_btn_cancel\")}"
                            }
                            button {
                                class: "btn-danger",
                                onclick: move |_| {
                                    phase.set(UninstallPhase::Uninstalling);
                                    let dir_clone = target_dir.clone();
                                    let del_appdata = delete_appdata();

                                    spawn(async move {
                                        progress.set(20);
                                        status_key.set("uninstall_status_closing".to_string());
                                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

                                        progress.set(50);
                                        status_key.set("uninstall_status_shortcuts".to_string());
                                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                                        progress.set(75);
                                        status_key.set("uninstall_status_purging".to_string());

                                        let result = tokio::task::spawn_blocking(move || {
                                            perform_native_uninstall_worker(&dir_clone, del_appdata)
                                        }).await.unwrap_or(Err("Uninstallation task panicked".to_string()));

                                        match result {
                                            Ok(_) => {
                                                progress.set(100);
                                                status_key.set("uninstall_status_complete".to_string());
                                                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                                                phase.set(UninstallPhase::Complete);
                                            }
                                            Err(err) => {
                                                error_msg.set(err);
                                                phase.set(UninstallPhase::Error);
                                            }
                                        }
                                    });
                                },
                                "{i18n::t(&current_lang.read(), \"uninstall_btn_confirm\")}"
                            }
                        }
                    }
                },
                UninstallPhase::Uninstalling => rsx! {
                    div { style: "padding: 30px 0 10px;",
                        div { class: "status-text", style: "margin-bottom: 12px; font-weight: 600; font-size: 15px; color: #f3f4f6;",
                            "{i18n::t(&current_lang.read(), \"uninstall_status_preparing\")}"
                        }
                        div { class: "progress-wrap",
                            div { class: "progress-bar-bg",
                                div {
                                    class: "progress-bar-fill",
                                    style: "width: {progress()}%; background: linear-gradient(90deg, #dc2626, #f59e0b);"
                                }
                            }
                            div { class: "status-text",
                                "{i18n::t(&current_lang.read(), &status_key.read())}"
                            }
                        }
                    }
                },
                UninstallPhase::Complete => rsx! {
                    div { style: "padding: 24px 0 12px; text-align: center;",
                        div {
                            style: "width: 58px; height: 58px; border-radius: 50%; background: rgba(16, 185, 129, 0.15); border: 2px solid #10b981; color: #10b981; font-size: 26px; display: flex; align-items: center; justify-content: center; margin: 0 auto 18px;",
                            "✓"
                        }
                        div { style: "font-size: 18px; font-weight: 700; color: #f9fafb; margin-bottom: 8px;",
                            "{i18n::t(&current_lang.read(), \"uninstall_complete_title\")}"
                        }
                        div { style: "font-size: 13px; color: #9ca3af; margin-bottom: 28px; line-height: 1.5; max-width: 440px; margin-left: auto; margin-right: auto;",
                            "{i18n::t(&current_lang.read(), \"uninstall_complete_desc\")}"
                        }
                        div { class: "actions-row no-drag", style: "justify-content: center;",
                            button {
                                class: "btn-install",
                                onclick: move |_| {
                                    schedule_temp_worker_cleanup();
                                    dioxus::desktop::window().close();
                                },
                                "{i18n::t(&current_lang.read(), \"setup_btn_close\")}"
                            }
                        }
                    }
                },
                UninstallPhase::Error => rsx! {
                    div { style: "padding: 20px 0 10px;",
                        div { class: "warning-banner", style: "border-color: #ef4444; background: rgba(239, 68, 68, 0.1); margin-bottom: 20px;",
                            span { class: "warning-icon", style: "color: #ef4444;", "✕" }
                            div {
                                div { style: "font-size: 13px; font-weight: 600; color: #fca5a5;",
                                    "{i18n::t(&current_lang.read(), \"uninstall_failed_title\")}"
                                }
                                div { style: "font-size: 12px; color: #fecaca; margin-top: 2px;", "{error_msg()}" }
                            }
                        }
                        div { class: "actions-row no-drag",
                            button {
                                class: "btn-cancel",
                                onclick: move |_| {
                                    schedule_temp_worker_cleanup();
                                    dioxus::desktop::window().close();
                                },
                                "{i18n::t(&current_lang.read(), \"setup_btn_close\")}"
                            }
                            button {
                                class: "btn-install",
                                onclick: move |_| phase.set(UninstallPhase::Confirm),
                                "{i18n::t(&current_lang.read(), \"setup_btn_retry\")}"
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn SetupApp() -> Element {
    let initial_lang = use_hook(detect_initial_language);
    let mut current_lang = use_signal(move || initial_lang);
    let mut lang_menu_open = use_signal(|| false);

    let mut phase = use_signal(|| SetupPhase::Config);
    let mut progress = use_signal(|| 0);
    let mut status_key = use_signal(|| "setup_status_preparing".to_string());
    let mut error_msg = use_signal(|| String::new());

    let default_path_fallback = r"C:\DLSS 5 Studio".to_string();
    let existing_info = detect_existing_installation();
    let is_reinstall = existing_info.is_some();
    let existing_version = existing_info.as_ref().and_then(|(_, v, _)| v.clone());

    let default_path = if let Some((ref p, _, _)) = existing_info {
        p.to_string_lossy().to_string()
    } else {
        default_path_fallback
    };

    let initial_storage = if let Some((_, _, Some(ref s))) = existing_info {
        s.clone()
    } else {
        compute_default_storage_path(Path::new(&default_path))
    };

    let customized_storage = existing_info.as_ref().and_then(|(_, _, ref s)| s.as_ref()).is_some();

    let mut install_path = use_signal(move || default_path);
    let mut storage_path = use_signal(move || initial_storage);
    let mut storage_manually_customized = use_signal(move || customized_storage);
    let mut show_advanced = use_signal(|| false);
    let mut startup_on_boot = use_signal(|| true);
    let mut run_in_background = use_signal(|| true);
    let mut create_desktop_shortcut = use_signal(|| true);

    let icon_data_uri = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(APP_ICON_PNG)
    );

    rsx! {
        div {
            class: "app-bg-wrapper",
            onmousedown: move |_| { dioxus::desktop::window().drag(); },
            onclick: move |_| { lang_menu_open.set(false); }
        }
        div {
            class: "setup-card",
            onmousedown: move |e| { e.stop_propagation(); },
            div {
                class: "drag-header",
                onmousedown: move |_| { dioxus::desktop::window().drag(); },
                div { class: "title-group",
                    img { class: "app-badge", src: "{icon_data_uri}" }
                    span { class: "app-title", "DLSS 5 STUDIO" }
                    if is_reinstall {
                        span {
                            style: "font-size: 11px; font-weight: 700; letter-spacing: 0.08em; color: #f59e0b; background: rgba(217, 119, 6, 0.2); border: 1px solid rgba(217, 119, 6, 0.45); border-radius: 6px; padding: 3px 8px; margin-left: 6px;",
                            "{i18n::t(&current_lang.read(), \"setup_tag_update\")}"
                        }
                    }
                }
                div { class: "header-actions no-drag",
                    div {
                        class: "lang-wrap",
                        button {
                            class: "lang-btn",
                            title: "Language",
                            onmousedown: move |e| { e.stop_propagation(); },
                            onclick: move |e| {
                                e.stop_propagation();
                                lang_menu_open.toggle();
                            },
                            svg { view_box: "0 0 24 24",
                                circle { cx: "12", cy: "12", r: "10" }
                                path { d: "M2 12h20" }
                                path { d: "M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" }
                            }
                            span { "{current_lang.read().to_uppercase()}" }
                            svg { style: "width: 10px; height: 10px;", view_box: "0 0 24 24",
                                path { d: "M6 9l6 6 6-6" }
                            }
                        }
                        if *lang_menu_open.read() {
                            div {
                                class: "lang-menu",
                                onmousedown: move |e| { e.stop_propagation(); },
                                onclick: move |e| { e.stop_propagation(); },
                                for item in i18n::SUPPORTED_LANGS {
                                    {
                                        let code = item.code;
                                        let is_active = *current_lang.read() == code;
                                        rsx! {
                                            button {
                                                class: if is_active { "lang-item active" } else { "lang-item" },
                                                onmousedown: move |e| { e.stop_propagation(); },
                                                onclick: move |e| {
                                                    e.stop_propagation();
                                                    current_lang.set(code.to_string());
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
                        class: "btn-close-header",
                        title: "{i18n::t(&current_lang.read(), \"setup_btn_close\")}",
                        onmousedown: move |e| { e.stop_propagation(); },
                        onclick: move |e| {
                            e.stop_propagation();
                            dioxus::desktop::window().close();
                        },
                        svg { style: "width: 15px; height: 15px; fill: currentColor;", view_box: "0 0 24 24",
                            path { d: "M19 6.41L17.59 5 12 10.59 6.41 5 5 6.41 10.59 12 5 17.59 6.41 19 12 13.41 17.59 19 19 17.59 13.41 12z" }
                        }
                    }
                }
            }

            match phase() {
                SetupPhase::Config => rsx! {
                    div {
                        if is_reinstall {
                            div {
                                style: "background: rgba(217, 119, 6, 0.12); border: 1px solid rgba(217, 119, 6, 0.45); border-radius: 10px; padding: 10px 14px; margin-bottom: 18px; display: flex; align-items: center; justify-content: space-between; gap: 12px;",
                                div {
                                    div { style: "font-size: 13px; font-weight: 600; color: #fbbf24;",
                                        if let Some(ref v) = existing_version {
                                            "{i18n::t_params(&current_lang.read(), \"setup_existing_detected\", &[v, env!(\"CARGO_PKG_VERSION\")])}"
                                        } else {
                                            "{i18n::t_param(&current_lang.read(), \"setup_existing_detected_generic\", env!(\"CARGO_PKG_VERSION\"))}"
                                        }
                                    }
                                    div { style: "font-size: 11.5px; color: #9ca3af; margin-top: 3px;",
                                        "{i18n::t(&current_lang.read(), \"setup_existing_help\")}"
                                    }
                                }
                                span { style: "font-size: 10px; font-weight: 700; letter-spacing: 0.05em; color: #f59e0b; background: rgba(217, 119, 6, 0.2); border: 1px solid rgba(217, 119, 6, 0.45); border-radius: 6px; padding: 4px 8px; flex-shrink: 0;",
                                    "{i18n::t(&current_lang.read(), \"setup_tag_update\")}"
                                }
                            }
                        }

                        // Installation Location
                        div { class: "section-label", "{i18n::t(&current_lang.read(), \"setup_install_location\")}" }
                        div { class: "path-row", style: if is_protected_directory(Path::new(&install_path())) { "margin-bottom: 6px;" } else { "margin-bottom: 20px;" },
                            input {
                                class: if is_protected_directory(Path::new(&install_path())) { "path-input path-input-warning no-drag" } else { "path-input no-drag" },
                                r#type: "text",
                                value: "{install_path()}",
                                readonly: true,
                            }
                            button {
                                class: "btn-browse no-drag",
                                onclick: move |_| {
                                    spawn(async move {
                                        if let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await {
                                            let path = folder.path();
                                            let chosen = if path.file_name().map(|n| n.to_string_lossy().to_lowercase()) == Some("dlss 5 studio".to_string()) {
                                                path.to_path_buf()
                                            } else {
                                                path.join("DLSS 5 Studio")
                                            };
                                            let chosen_str = chosen.to_string_lossy().to_string();
                                            install_path.set(chosen_str);

                                            if !storage_manually_customized() {
                                                storage_path.set(compute_default_storage_path(&chosen));
                                            }
                                        }
                                    });
                                },
                                "{i18n::t(&current_lang.read(), \"setup_browse\")}"
                            }
                        }

                        if is_protected_directory(Path::new(&install_path())) {
                            div { class: "warning-banner", style: "margin-top: 0; margin-bottom: 20px;",
                                span { class: "warning-icon", "⚠" }
                                span { class: "warning-msg",
                                    "{i18n::t(&current_lang.read(), \"setup_warn_protected_install\")}"
                                }
                            }
                        }

                        // System Preferences
                        div { class: "section-label", "{i18n::t(&current_lang.read(), \"setup_system_prefs\")}" }
                        div { class: "prefs-group no-drag",
                            div {
                                class: "pref-item",
                                onclick: move |_| startup_on_boot.set(!startup_on_boot()),
                                div { class: if startup_on_boot() { "chk-box checked" } else { "chk-box" },
                                    span { class: "chk-icon", "✓" }
                                }
                                span { "{i18n::t(&current_lang.read(), \"setup_pref_startup\")}" }
                            }
                            div {
                                class: "pref-item",
                                onclick: move |_| run_in_background.set(!run_in_background()),
                                div { class: if run_in_background() { "chk-box checked" } else { "chk-box" },
                                    span { class: "chk-icon", "✓" }
                                }
                                span { "{i18n::t(&current_lang.read(), \"setup_pref_background\")}" }
                            }
                            div {
                                class: "pref-item",
                                onclick: move |_| create_desktop_shortcut.set(!create_desktop_shortcut()),
                                div { class: if create_desktop_shortcut() { "chk-box checked" } else { "chk-box" },
                                    span { class: "chk-icon", "✓" }
                                }
                                span { "{i18n::t(&current_lang.read(), \"setup_pref_desktop\")}" }
                            }
                        }

                        // Advanced Options expandable toggle
                        div {
                            class: "advanced-toggle-row no-drag",
                            onclick: move |_| show_advanced.set(!show_advanced()),
                            span {
                                class: if show_advanced() { "advanced-chevron open" } else { "advanced-chevron" },
                                "▸"
                            }
                            span { class: "advanced-toggle-label", "{i18n::t(&current_lang.read(), \"setup_advanced_options\")}" }
                        }

                        if show_advanced() {
                            div { class: "advanced-panel no-drag",
                                div { class: "section-label", "{i18n::t(&current_lang.read(), \"setup_storage_location\")}" }
                                div { class: "path-row", style: "margin-bottom: 6px;",
                                    input {
                                        class: if is_protected_directory(Path::new(&storage_path())) { "path-input path-input-warning no-drag" } else { "path-input no-drag" },
                                        r#type: "text",
                                        value: "{storage_path()}",
                                        readonly: true,
                                    }
                                    button {
                                        class: "btn-browse no-drag",
                                        onclick: move |_| {
                                            spawn(async move {
                                                if let Some(folder) = rfd::AsyncFileDialog::new().pick_folder().await {
                                                    let path = folder.path().to_string_lossy().to_string();
                                                    storage_path.set(path);
                                                    storage_manually_customized.set(true);
                                                }
                                            });
                                        },
                                        "{i18n::t(&current_lang.read(), \"setup_browse\")}"
                                    }
                                }
                                div { class: "helper-text", "{i18n::t(&current_lang.read(), \"setup_storage_help\")}" }

                                if is_protected_directory(Path::new(&storage_path())) {
                                    div { class: "warning-banner",
                                        span { class: "warning-icon", "⚠" }
                                        span { class: "warning-msg",
                                            "{i18n::t(&current_lang.read(), \"setup_warn_protected_storage\")}"
                                        }
                                    }
                                }
                            }
                        }

                        // Actions
                        div { class: "actions-row no-drag",
                            button {
                                class: "btn-install",
                                onclick: move |_| {
                                    phase.set(SetupPhase::Installing);
                                    let target_folder = install_path();
                                    let target_storage = storage_path();
                                    let s_boot = startup_on_boot();
                                    let b_run = run_in_background();
                                    let d_shortcut = create_desktop_shortcut();
                                    let chosen_lang = current_lang.read().clone();

                                    spawn(async move {
                                        progress.set(15);
                                        status_key.set("setup_status_target".to_string());
                                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

                                        progress.set(45);
                                        status_key.set("setup_status_extracting".to_string());

                                        let target_folder_clone = target_folder.clone();
                                        let target_storage_clone = target_storage.clone();
                                        let lang_clone = chosen_lang.clone();
                                        let result = tokio::task::spawn_blocking(move || {
                                            run_installation_pipeline(target_folder_clone, target_storage_clone, s_boot, b_run, d_shortcut, lang_clone)
                                        }).await.unwrap_or(Err("Installation task panicked".to_string()));

                                        match result {
                                            Ok(_) => {
                                                progress.set(100);
                                                status_key.set("setup_status_complete".to_string());
                                                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                                                phase.set(SetupPhase::Complete);
                                            }
                                            Err(e) => {
                                                error_msg.set(e);
                                                phase.set(SetupPhase::Error);
                                            }
                                        }
                                    });
                                },
                                if is_reinstall {
                                    "{i18n::t(&current_lang.read(), \"setup_btn_update\")}"
                                } else {
                                    "{i18n::t(&current_lang.read(), \"setup_btn_install\")}"
                                }
                            }
                            button {
                                class: "btn-cancel",
                                onclick: move |_| { dioxus::desktop::window().close(); },
                                "{i18n::t(&current_lang.read(), \"setup_btn_cancel\")}"
                            }
                        }
                    }
                },
                SetupPhase::Installing => rsx! {
                    div { class: "progress-wrap",
                        div { class: "progress-bar-bg",
                            div { class: "progress-bar-fill", style: "width: {progress()}%;" }
                        }
                        div { class: "status-text", "{i18n::t(&current_lang.read(), &status_key.read())}" }
                    }
                },
                SetupPhase::Complete => rsx! {
                    div { style: "padding: 10px 0; text-align: center;",
                        div { style: "color: #10b981; font-size: 40px; margin-bottom: 12px;", "✓" }
                        h2 { style: "font-size: 20px; font-weight: 700; color: #f9fafb; margin-bottom: 8px;",
                            "{i18n::t(&current_lang.read(), \"setup_complete_title\")}"
                        }
                        p { style: "font-size: 13px; color: #9ca3af; margin-bottom: 24px;",
                            "{i18n::t(&current_lang.read(), \"setup_complete_desc\")}"
                        }
                        div { class: "actions-row no-drag", style: "justify-content: center;",
                            button {
                                class: "btn-install",
                                onclick: {
                                    let target_folder = install_path();
                                    move |_| {
                                        let exe = PathBuf::from(&target_folder).join("dlss-studio.exe");
                                        if exe.exists() {
                                            let _ = std::process::Command::new(exe).spawn();
                                        }
                                        dioxus::desktop::window().close();
                                    }
                                },
                                "{i18n::t(&current_lang.read(), \"setup_btn_launch\")}"
                            }
                            button {
                                class: "btn-cancel",
                                onclick: move |_| { dioxus::desktop::window().close(); },
                                "{i18n::t(&current_lang.read(), \"setup_btn_finish\")}"
                            }
                        }
                    }
                },
                SetupPhase::Error => rsx! {
                    div { style: "padding: 20px 10px; text-align: center;",
                        div { style: "color: #ef4444; font-size: 32px; margin-bottom: 12px;", "⚠" }
                        h2 { style: "font-size: 19px; font-weight: 700; color: #f3f4f6; margin-bottom: 8px;",
                            "{i18n::t(&current_lang.read(), \"setup_failed_title\")}"
                        }
                        p { style: "font-size: 13px; color: #ef4444; margin-bottom: 24px; word-break: break-word;", "{error_msg()}" }
                        div { class: "actions-row no-drag", style: "justify-content: center;",
                            button {
                                class: "btn-browse",
                                onclick: move |_| { phase.set(SetupPhase::Config); },
                                "{i18n::t(&current_lang.read(), \"setup_btn_back\")}"
                            }
                            button {
                                class: "btn-cancel",
                                onclick: move |_| { dioxus::desktop::window().close(); },
                                "{i18n::t(&current_lang.read(), \"setup_btn_close\")}"
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Creates a Windows shell shortcut (.lnk) using Win32 COM APIs natively
fn create_shortcut(target_exe: &Path, shortcut_path: &Path, description: &str) -> Result<(), String> {
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, IPersistFile};
    use windows::Win32::UI::Shell::{IShellLinkW, ShellLink};
    use windows::core::{Interface, HSTRING};

    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("CoCreateInstance ShellLink failed: {:?}", e))?;

        let target_str = target_exe.to_string_lossy().to_string();
        link.SetPath(&HSTRING::from(target_str.clone()))
            .map_err(|e| format!("SetPath failed: {:?}", e))?;

        let _ = link.SetIconLocation(&HSTRING::from(target_str.clone()), 0);

        if let Some(parent) = target_exe.parent() {
            let working_dir = parent.to_string_lossy().to_string();
            link.SetWorkingDirectory(&HSTRING::from(working_dir))
                .map_err(|e| format!("SetWorkingDirectory failed: {:?}", e))?;
        }

        link.SetDescription(&HSTRING::from(description))
            .map_err(|e| format!("SetDescription failed: {:?}", e))?;

        let persist: IPersistFile = link.cast()
            .map_err(|e| format!("Cast to IPersistFile failed: {:?}", e))?;

        let shortcut_str = shortcut_path.to_string_lossy().to_string();
        persist.Save(&HSTRING::from(shortcut_str), true)
            .map_err(|e| format!("Save shortcut failed: {:?}", e))?;

        CoUninitialize();
    }
    Ok(())
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Detects if DLSS 5 Studio is already installed on the system via Windows registry
fn detect_existing_installation() -> Option<(PathBuf, Option<String>, Option<String>)> {
    use windows::Win32::System::Registry::{
        RegOpenKeyExW, RegQueryValueExW, RegCloseKey, HKEY_CURRENT_USER, KEY_READ, REG_SZ,
    };

    let subkey = to_wide(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\DLSS 5 Studio");
    let mut hkey = windows::Win32::System::Registry::HKEY::default();

    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(subkey.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        ).is_err() {
            return None;
        }

        let read_sz = |name: &str| -> Option<String> {
            let name_w = to_wide(name);
            let mut data_type = windows::Win32::System::Registry::REG_VALUE_TYPE(0);
            let mut byte_len: u32 = 0;
            if RegQueryValueExW(
                hkey,
                windows::core::PCWSTR(name_w.as_ptr()),
                None,
                Some(&mut data_type),
                None,
                Some(&mut byte_len),
            ).is_err() || byte_len == 0 {
                return None;
            }

            let mut buf = vec![0u8; byte_len as usize];
            if RegQueryValueExW(
                hkey,
                windows::core::PCWSTR(name_w.as_ptr()),
                None,
                Some(&mut data_type),
                Some(buf.as_mut_ptr()),
                Some(&mut byte_len),
            ).is_ok() && data_type == REG_SZ {
                let u16_slice: &[u16] = std::slice::from_raw_parts(
                    buf.as_ptr() as *const u16,
                    (byte_len as usize) / 2,
                );
                let trimmed: Vec<u16> = u16_slice.iter().copied().take_while(|&c| c != 0).collect();
                String::from_utf16(&trimmed).ok()
            } else {
                None
            }
        };

        let install_loc = read_sz("InstallLocation");
        let display_ver = read_sz("DisplayVersion");
        let _ = RegCloseKey(hkey);

        if let Some(loc_str) = install_loc {
            let loc_path = PathBuf::from(loc_str);
            if loc_path.join("dlss-studio.exe").exists() {
                let mut data_dir = None;
                let storage_file = loc_path.join("storage.json");
                if let Ok(content) = std::fs::read_to_string(&storage_file) {
                    if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                        if let Some(d) = val.get("data_dir").and_then(|v| v.as_str()) {
                            data_dir = Some(d.to_string());
                        }
                    }
                }
                return Some((loc_path, display_ver, data_dir));
            }
        }
    }
    None
}

/// Registers the application in Windows Installed Apps / Add or Remove Programs registry
fn register_uninstall_entry(install_dir: &Path, installed_exe: &Path) -> Result<(), String> {
    use windows::Win32::System::Registry::{
        RegCreateKeyExW, RegSetValueExW, RegCloseKey, HKEY_CURRENT_USER, KEY_WRITE, REG_SZ, REG_OPTION_NON_VOLATILE,
    };

    let subkey = to_wide(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\DLSS 5 Studio");
    let mut hkey = windows::Win32::System::Registry::HKEY::default();

    unsafe {
        let res = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(subkey.as_ptr()),
            0,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        if res.is_err() {
            return Err(format!("RegCreateKeyExW failed: {:?}", res));
        }

        let write_val = |name: &str, val: &str| {
            let name_w = to_wide(name);
            let val_w = to_wide(val);
            let bytes = std::slice::from_raw_parts(val_w.as_ptr() as *const u8, val_w.len() * 2);
            let _ = RegSetValueExW(
                hkey,
                windows::core::PCWSTR(name_w.as_ptr()),
                0,
                REG_SZ,
                Some(bytes),
            );
        };

        write_val("DisplayName", "DLSS 5 Studio");
        write_val("DisplayVersion", env!("CARGO_PKG_VERSION"));
        write_val("Publisher", "Bookamp");
        write_val("DisplayIcon", &format!("{},0", installed_exe.display()));
        write_val("InstallLocation", &install_dir.display().to_string());
        
        let uninst_exe = install_dir.join("uninstall.exe");
        write_val("UninstallString", &format!("\"{}\" --uninstall", uninst_exe.display()));
        write_val("QuietUninstallString", &format!("\"{}\" --uninstall --silent", uninst_exe.display()));

        let _ = RegCloseKey(hkey);
    }
    Ok(())
}

/// Removes the application registration from Windows Installed Apps registry
fn unregister_uninstall_entry() {
    use windows::Win32::System::Registry::{RegDeleteKeyW, HKEY_CURRENT_USER};
    let subkey = to_wide(r"Software\Microsoft\Windows\CurrentVersion\Uninstall\DLSS 5 Studio");
    unsafe {
        let _ = RegDeleteKeyW(HKEY_CURRENT_USER, windows::core::PCWSTR(subkey.as_ptr()));
    }
}

/// Executes the pure native installation
fn run_installation_pipeline(
    target_folder: String,
    storage_folder: String,
    startup: bool,
    background: bool,
    desktop_shortcut: bool,
    selected_lang: String,
) -> Result<(), String> {
    let dest = PathBuf::from(&target_folder);
    std::fs::create_dir_all(&dest)
        .map_err(|e| format!("Could not create directory {}: {}", dest.display(), e))?;

    // 1. Force close any running background instances of dlss-studio before overwriting
    let _ = std::process::Command::new("taskkill")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["/F", "/IM", "dlss-studio.exe", "/IM", "dlss5-swapper-rust.exe", "/IM", "dlss5-swapper-rust-portable.exe"])
        .status();
    std::thread::sleep(std::time::Duration::from_millis(400));

    // Determine payload bytes:
    let payload_data: Vec<u8> = if !PAYLOAD.is_empty() {
        PAYLOAD.to_vec()
    } else {
        let candidate = Path::new("target/release/dlss-studio.exe");
        let debug_candidate = Path::new("target/debug/dlss-studio.exe");
        if candidate.exists() {
            std::fs::read(candidate).unwrap_or_default()
        } else if debug_candidate.exists() {
            std::fs::read(debug_candidate).unwrap_or_default()
        } else {
            Vec::new()
        }
    };

    if payload_data.is_empty() {
        return Err("Installation payload not found. Please compile dlss-studio first.".to_string());
    }

    let installed_exe = dest.join("dlss-studio.exe");
    let mut write_res = std::fs::write(&installed_exe, &payload_data);
    if write_res.is_err() {
        // Attempt extra process cleanup in case Windows held onto the handle briefly
        for _ in 0..3 {
            let _ = std::process::Command::new("taskkill")
                .creation_flags(CREATE_NO_WINDOW)
                .args(["/F", "/IM", "dlss-studio.exe", "/IM", "dlss5-swapper-rust.exe", "/IM", "dlss5-swapper-rust-portable.exe"])
                .status();
            std::thread::sleep(std::time::Duration::from_millis(400));
            write_res = std::fs::write(&installed_exe, &payload_data);
            if write_res.is_ok() {
                break;
            }
        }
    }
    write_res.map_err(|e| {
        format!(
            "Failed to write application binary: {}. If DLSS 5 Studio is currently open or running in the system tray, please exit it and click Retry.",
            e
        )
    })?;

    // Write storage configuration into installation folder
    let storage_dest = PathBuf::from(&storage_folder);
    let _ = std::fs::create_dir_all(&storage_dest);
    let storage_cfg = serde_json::json!({
        "data_dir": storage_folder
    });
    let _ = std::fs::write(
        dest.join("storage.json"),
        serde_json::to_string_pretty(&storage_cfg).unwrap_or_default(),
    );

    // Persist selected language into library.json so DLSS Studio immediately launches in this language
    let lang_code = selected_lang.clone();
    let update_library_lang = |lib_path: &Path| {
        let mut data: serde_json::Value = if let Ok(c) = std::fs::read_to_string(lib_path) {
            serde_json::from_str(&c).unwrap_or(serde_json::json!({}))
        } else {
            serde_json::json!({})
        };
        if let Some(obj) = data.as_object_mut() {
            obj.insert("lang".to_string(), serde_json::Value::String(lang_code.clone()));
        }
        let _ = std::fs::write(lib_path, serde_json::to_string_pretty(&data).unwrap_or_default());
    };
    update_library_lang(&storage_dest.join("library.json"));
    if let Ok(appdata) = std::env::var("APPDATA") {
        let default_appdata_dir = PathBuf::from(appdata).join("dlss-5-studio");
        let _ = std::fs::create_dir_all(&default_appdata_dir);
        update_library_lang(&default_appdata_dir.join("library.json"));
    }

    // Copy setup.exe as uninstall.exe in target folder (prevent copying onto itself)
    if let Ok(curr) = std::env::current_exe() {
        let uninst_dest = dest.join("uninstall.exe");
        if curr != uninst_dest {
            let _ = std::fs::copy(&curr, &uninst_dest);
        }
    }

    // Create Start Menu shortcut
    if let Ok(appdata) = std::env::var("APPDATA") {
        let start_menu = PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs");
        let _ = std::fs::create_dir_all(&start_menu);
        let _ = create_shortcut(
            &installed_exe,
            &start_menu.join("DLSS 5 Studio.lnk"),
            "DLSS 5 Studio - Native DLSS & Frame Generation Manager",
        );
    }

    // Create Desktop shortcut
    if desktop_shortcut {
        if let Ok(userprofile) = std::env::var("USERPROFILE") {
            let desktop = PathBuf::from(userprofile).join("Desktop");
            let _ = create_shortcut(
                &installed_exe,
                &desktop.join("DLSS 5 Studio.lnk"),
                "DLSS 5 Studio - Native DLSS & Frame Generation Manager",
            );
        }
    }

    // Register in Windows Installed Apps
    let _ = register_uninstall_entry(&dest, &installed_exe);

    // Apply startup & background preferences
    if startup {
        let _ = std::process::Command::new(&installed_exe)
            .creation_flags(CREATE_NO_WINDOW)
            .arg("--enable-startup-only")
            .status();
    }
    if !background {
        let _ = std::process::Command::new(&installed_exe)
            .creation_flags(CREATE_NO_WINDOW)
            .arg("--disable-background-only")
            .status();
    }

    Ok(())
}

/// Silently or interactively uninstalls DLSS 5 Studio
#[allow(dead_code)]
fn perform_native_uninstall(silent: bool) {
    let target_dir = detect_existing_installation()
        .map(|(p, _, _)| p)
        .unwrap_or_else(|| {
            if let Ok(curr) = std::env::current_exe() {
                curr.parent().unwrap_or(Path::new(r"C:\Program Files\DLSS 5 Studio")).to_path_buf()
            } else {
                PathBuf::from(r"C:\Program Files\DLSS 5 Studio")
            }
        });
    let _ = perform_native_uninstall_worker(&target_dir, true);
    if !silent {
        use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
        let text = to_wide("DLSS 5 Studio has been successfully uninstalled from your computer.");
        let caption = to_wide("DLSS 5 Studio Uninstall");
        unsafe {
            let _ = MessageBoxW(
                None,
                windows::core::PCWSTR(text.as_ptr()),
                windows::core::PCWSTR(caption.as_ptr()),
                MB_OK | MB_ICONINFORMATION,
            );
        }
    }
}

/// Executes the full native uninstall worker operations: closing processes, purging files,
/// unregistering shortcuts and registry keys, removing the installation directory, and cleaning app data.
fn perform_native_uninstall_worker(target_install_dir: &Path, delete_appdata: bool) -> Result<(), String> {
    // 1. Terminate any running instances of dlss-studio
    let _ = std::process::Command::new("taskkill")
        .creation_flags(CREATE_NO_WINDOW)
        .args(["/F", "/IM", "dlss-studio.exe", "/IM", "dlss-studio-portable.exe", "/IM", "dlss5-swapper-rust.exe", "/IM", "dlss5-swapper-rust-portable.exe"])
        .status();
    std::thread::sleep(std::time::Duration::from_millis(500));

    // 2. Remove Shortcuts
    if let Ok(appdata) = std::env::var("APPDATA") {
        let sm = PathBuf::from(appdata).join(r"Microsoft\Windows\Start Menu\Programs\DLSS 5 Studio.lnk");
        let _ = std::fs::remove_file(sm);
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        let dt = PathBuf::from(userprofile).join(r"Desktop\DLSS 5 Studio.lnk");
        let _ = std::fs::remove_file(dt);
    }

    // 3. Remove Windows Registry Entries
    unregister_uninstall_entry();
    let _ = crate_startup_uninstall_cleanup();

    // Read custom storage directory from storage.json before removing target_install_dir
    let custom_storage_dir: Option<PathBuf> = if target_install_dir.exists() {
        let storage_file = target_install_dir.join("storage.json");
        std::fs::read_to_string(&storage_file)
            .ok()
            .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok())
            .and_then(|val| val.get("data_dir").and_then(|d| d.as_str()).map(PathBuf::from))
    } else {
        None
    };

    // 4. Purge the entire target installation directory
    if target_install_dir.exists() {
        for _ in 0..5 {
            if std::fs::remove_dir_all(target_install_dir).is_ok() || !target_install_dir.exists() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        if target_install_dir.exists() {
            // Delete individual files if remove_dir_all is partially hindered
            for entry in walkdir::WalkDir::new(target_install_dir).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_file() {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
            let _ = std::fs::remove_dir_all(target_install_dir);
        }
    }

    // 5. Cleanup user cache, downloaded components, and preferences if requested
    if delete_appdata {
        if let Some(ref custom_dir) = custom_storage_dir {
            if custom_dir.exists() && custom_dir != target_install_dir && !target_install_dir.starts_with(custom_dir) {
                let _ = std::fs::remove_dir_all(custom_dir);
            }
        }
        if let Ok(appdata) = std::env::var("APPDATA") {
            let user_dir = PathBuf::from(appdata).join("dlss-5-studio");
            if user_dir.exists() {
                let _ = std::fs::remove_dir_all(user_dir);
            }
        }
        if let Ok(progdata) = std::env::var("ProgramData") {
            let pd_dir = PathBuf::from(progdata).join("dlss-5-studio");
            if pd_dir.exists() {
                let _ = std::fs::remove_dir_all(pd_dir);
            }
        }
    }

    // 6. Schedule self-deletion of the temporary worker binary from %TEMP%
    schedule_temp_worker_cleanup();

    Ok(())
}

fn schedule_temp_worker_cleanup() {
    if let Ok(curr) = std::env::current_exe() {
        if curr.starts_with(std::env::temp_dir()) {
            let _ = std::process::Command::new("cmd.exe")
                .creation_flags(CREATE_NO_WINDOW)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .args(["/C", "choice /C Y /N /D Y /T 2 > NUL & del", &format!("\"{}\"", curr.display())])
                .spawn();
        }
    }
}

fn crate_startup_uninstall_cleanup() -> Result<(), ()> {
    use windows::Win32::System::Registry::{RegOpenKeyExW, RegDeleteValueW, HKEY_CURRENT_USER, KEY_WRITE};
    let run_key = to_wide(r"Software\Microsoft\Windows\CurrentVersion\Run");
    let val_name = to_wide("DLSS5Studio");
    let mut hkey = windows::Win32::System::Registry::HKEY::default();
    unsafe {
        if RegOpenKeyExW(
            HKEY_CURRENT_USER,
            windows::core::PCWSTR(run_key.as_ptr()),
            0,
            KEY_WRITE,
            &mut hkey,
        ).is_ok() {
            let _ = RegDeleteValueW(hkey, windows::core::PCWSTR(val_name.as_ptr()));
            let _ = windows::Win32::System::Registry::RegCloseKey(hkey);
        }
    }
    Ok(())
}
