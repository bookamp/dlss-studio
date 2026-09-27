#![windows_subsystem = "windows"]

use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use dlss_studio::{core, ui, big_picture};

use dioxus::prelude::*;
use dioxus::desktop::{Config, WindowBuilder, LogicalSize};
use dioxus::desktop::tao::window::Fullscreen;
use dioxus::desktop::tao::window::Icon as TaoIcon;

fn main() {
    // Prune logs older than 7 days on startup
    core::logger::prune_old_entries();
    core::logger::info("app", &format!("=== DLSS Studio startup (version: {}, pid: {}) ===", env!("CARGO_PKG_VERSION"), std::process::id()));
    core::logger::info("app", &format!("Command line args: {:?}", std::env::args().collect::<Vec<_>>()));
    core::logger::info("app", &format!("Log file: {}", core::logger::get_log_file_path().display()));
    core::logger::info("app", &format!("Appdata directory: {}", core::state::get_appdata_dir().display()));

    let args: Vec<String> = std::env::args().collect();

    // Early exit headless commands (executed by setup or automation scripts)
    if args.iter().any(|a| a == "--enable-startup-only") {
        core::logger::info("app", "Enabling Windows startup (headless) via --enable-startup-only");
        let _ = core::tray::set_startup_enabled(true);
        return;
    }
    if args.iter().any(|a| a == "--disable-startup-only") {
        core::logger::info("app", "Disabling Windows startup (headless) via --disable-startup-only");
        let _ = core::tray::set_startup_enabled(false);
        return;
    }
    if args.iter().any(|a| a == "--enable-background-only") {
        core::logger::info("app", "Setting run_in_background = true (headless)");
        let mut s = core::state::load_state();
        s.run_in_background = true;
        let _ = core::state::save_state(&s);
        return;
    }
    if args.iter().any(|a| a == "--disable-background-only") {
        core::logger::info("app", "Setting run_in_background = false (headless)");
        let mut s = core::state::load_state();
        s.run_in_background = false;
        let _ = core::state::save_state(&s);
        return;
    }
    if args.len() >= 3 && args[1] == "--restore" {
        let p = std::path::PathBuf::from(&args[2]);
        match core::journal::restore_game(&p) {
            Ok(b) => println!("[RESTORE_SUCCESS] {}", b),
            Err(e) => eprintln!("[RESTORE_ERROR] {}", e),
        }
        return;
    }
    if args.len() >= 4 && args[1] == "--deploy-opti" {
        let game_dir = std::path::PathBuf::from(&args[2]);
        let exe_path = std::path::PathBuf::from(&args[3]);
        let passes = args.get(4).and_then(|p| p.parse::<u32>().ok()).unwrap_or(1);
        let opts = core::optiscaler::DeployOptions {
            game_name: Some("Target Game".to_string()),
            game_dir,
            exe_path,
            api: "DirectX 12".to_string(),
            pre_sr: true,
            passes,
            mfg_unlock: false,
            mfg_multiplier: 1,
            nr_style: 0,
            nr_style_enabled: false,
        };
        match core::optiscaler::deploy_optiscaler(&opts) {
            Ok(res) => {
                for line in res.log_lines {
                    println!("{}", line);
                }
            }
            Err(e) => eprintln!("[DEPLOY_ERROR] {}", e),
        }
        return;
    }

    if args.iter().any(|a| a == "--enable-startup") {
        core::logger::info("app", "Enabling Windows startup via --enable-startup");
        let _ = core::tray::set_startup_enabled(true);
    }
    if args.iter().any(|a| a == "--disable-startup") {
        core::logger::info("app", "Disabling Windows startup via --disable-startup");
        let _ = core::tray::set_startup_enabled(false);
    }
    if args.iter().any(|a| a == "--enable-background") {
        core::logger::info("app", "Setting run_in_background = true");
        let mut s = core::state::load_state();
        s.run_in_background = true;
        let _ = core::state::save_state(&s);
    }
    if args.iter().any(|a| a == "--disable-background") {
        core::logger::info("app", "Setting run_in_background = false");
        let mut s = core::state::load_state();
        s.run_in_background = false;
        let _ = core::state::save_state(&s);
    }

    // Synchronize Vibepollo cover art in local user AppData (no elevation required)
    if core::vibepollo::is_vibepollo_installed() {
        let _ = core::vibepollo::ensure_vibepollo_cover_art();
    }

    let start_in_bp = big_picture::launch_args_request_big_picture(&args);

    // Single instance check: only ever allow one running instance
    let _instance_guard = match core::single_instance::acquire_single_instance(start_in_bp) {
        Some(guard) => guard,
        None => {
            core::logger::info(
                "app",
                &format!(
                    "Another instance of DLSS 5 Studio is already running. Signaled existing instance (big_picture: {}) and exiting.",
                    start_in_bp
                ),
            );
            return;
        }
    };

    let css_bytes = miniz_oxide::inflate::decompress_to_vec(include_bytes!(concat!(
        env!("OUT_DIR"),
        "/style.css.deflate"
    )))
    .expect("Failed to decompress embedded style.css");
    let css_style = String::from_utf8(css_bytes).expect("style.css is not valid UTF-8");
    let css_overlay_lab = include_str!("../assets/overlay-lab.css");
    let css_overlay_ctrl = include_str!("../assets/overlay-controls.css");

    let initial_state = core::state::load_state();
    let _ = core::journal::read_history();
    let initial_theme = initial_state.theme;
    let theme_val = if initial_theme == "light" { "light" } else { "dark" };
    let rust_theme_val = if initial_state.rust_theme { "true" } else { "false" };
    core::logger::info("app", &format!("Initial theme: {}, rust_theme: {}, version: {:?}", theme_val, rust_theme_val, initial_state.version));

    let head = format!(
        r#"<meta charset="utf-8" />
<style>{}</style>
<style>{}</style>
<style>{}</style>
<script>
  document.documentElement.setAttribute('data-theme', '{}');
  document.documentElement.setAttribute('data-rust-theme', '{}');
</script>
"#,
        css_style, css_overlay_lab, css_overlay_ctrl, theme_val, rust_theme_val
    );

    let args: Vec<String> = std::env::args().collect();
    if args.len() > 2 && args[1] == "--deploy-optiscaler" {
        let game_dir = std::path::PathBuf::from(&args[2]);
        if let Some(game) = core::scan::scan_game_directory(&game_dir) {
            let opts = core::optiscaler::DeployOptions {
                game_name: Some(game.name.clone()),
                game_dir: game.dir.clone(),
                exe_path: game.exe_path.clone(),
                api: game.api.clone(),
                pre_sr: true,
                passes: 1,
                mfg_unlock: true,
                mfg_multiplier: 4,
                nr_style: 0,
                nr_style_enabled: false,
            };
            match core::optiscaler::deploy_optiscaler(&opts) {
                Ok(res) => {
                    for line in res.log_lines {
                        println!("{}", line);
                    }
                }
                Err(e) => eprintln!("Deploy error: {}", e),
            }
        } else {
            eprintln!("Could not scan game directory: {}", game_dir.display());
        }
        return;
    }

    let start_in_bg = args.iter().any(|a| a == "--background" || a == "--minimized");
    core::logger::info("app", &format!("Start in background: {}", start_in_bg));


    // Initialize the native Win32 system tray
    core::tray::start_system_tray();
    core::logger::info("app", "Native Win32 system tray initialized");

    let icon = TaoIcon::from_rgba(include_bytes!("../assets/icon_64.rgba").to_vec(), 64, 64).ok();

    use dioxus::desktop::tao::platform::windows::WindowBuilderExtWindows;
    let mut window = WindowBuilder::new()
        .with_title("DLSS 5 Studio")
        .with_decorations(false)
        .with_resizable(true)
        .with_undecorated_shadow(true)
        .with_visible(!start_in_bg)
        .with_inner_size(LogicalSize::new(1280.0, 900.0))
        .with_min_inner_size(LogicalSize::new(960.0, 640.0));

    if start_in_bp {
        if let Some(target) = core::display::find_streaming_display() {
            core::logger::info(
                "app",
                &format!(
                    "Configuring Big Picture window on target monitor: {} ({}x{} at {},{})",
                    target.description, target.width, target.height, target.left, target.top
                ),
            );
            window = window
                .with_position(dioxus::desktop::tao::dpi::PhysicalPosition::new(target.left, target.top));
        }
        window = window.with_fullscreen(Some(Fullscreen::Borderless(None)));
    }

    if let Some(ic) = icon {
        window = window.with_window_icon(Some(ic));
    }

    // Ensure WebView2 user data folder is always placed in a user-writable directory (%APPDATA%\dlss-studio\webview),
    // preventing HRESULT(0x80070005) "Access is denied" when installed in "C:\Program Files\DLSS 5 Studio".
    let webview_data_dir = core::state::get_appdata_dir().join("webview");
    let _ = std::fs::create_dir_all(&webview_data_dir);
    if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_none() {
        std::env::set_var("WEBVIEW2_USER_DATA_FOLDER", &webview_data_dir);
    }

    let cfg = Config::new()
        .with_background_color((8, 10, 14, 255))
        .with_data_directory(webview_data_dir)
        .with_window(window)
        .with_custom_head(head)
        .with_custom_protocol("dlss-art", |req| {
            let uri_str = req.uri().to_string();
            if let Some((mime, bytes)) = core::steamart::handle_art_request(&uri_str) {
                dioxus::desktop::wry::http::Response::builder()
                    .header("Content-Type", mime)
                    .header("Access-Control-Allow-Origin", "*")
                    .header("Cache-Control", "public, max-age=31536000")
                    .body(std::borrow::Cow::Owned(bytes))
                    .unwrap_or_else(|_| {
                        dioxus::desktop::wry::http::Response::builder()
                            .status(500)
                            .body(std::borrow::Cow::Borrowed(&[][..]))
                            .unwrap()
                    })
            } else {
                core::logger::debug("art", &format!("Custom protocol request not found: {}", uri_str));
                dioxus::desktop::wry::http::Response::builder()
                    .status(404)
                    .body(std::borrow::Cow::Borrowed(&[][..]))
                    .unwrap()
            }
        });

    LaunchBuilder::desktop().with_cfg(cfg).launch(ui::app::App);
}
