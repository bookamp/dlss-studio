//! 10-Foot Television & Handheld Showcase View for DLSS Studio Big Picture Mode.
//!
//! Crafted in pure Rust (Dioxus RSX) with deep Rust texture styling, dynamic hero
//! backdrop vignetting, full-screen responsive game grid layout mirroring the desktop UI,
//! inline 2-column inspector with fluid adjacent card shifting, authentic advisor notes,
//! additional preset tuning (Pre-SR, Neural Rendering Style, 4X MFG), separate Play/Patch controls,
//! live patch status reflection, and dynamic hardware controller glyph HUD feedback.
//!
//! Uses the EXACT routing arbitration, recommendation engine, and advisory logic from the desktop app (`src/ui/app.rs`).

use dioxus::desktop::use_window;
use dioxus::prelude::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

use super::gamepad::{detect_controller_kind, start_gamepad_listener, GamepadNavAction};
use super::supervisor::launch_and_supervise;
use super::tv::TvSleepInhibitor;
use crate::core::install_routes::{
    get_mfg_advisory_with_route, get_native_dlss_advisory, get_optiscaler_advisory, recommended_route, InstallRoute,
    RouteAdvisory,
};
use crate::core::journal::restore_game;
use crate::core::optiscaler::{
    deploy_feeder_with_bundle, deploy_native_dlss5, deploy_optiscaler, DeployOptions, PayloadBundle,
};
use crate::core::scan::{GameEntry, GameExeOption};

pub const BP_STYLE_CSS: &str = include_str!("../../assets/big_picture/style.css");
pub const BRAND_BADGE_WEBP: &[u8] = include_bytes!("../../assets/brand-badge.webp");


pub const STORES: &[&str] = &[
    "All",
    "Steam",
    "Xbox",
    "Epic Games",
    "GOG",
    "Added by hand",
    "My folders",
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeployPresetOptions {
    pub pre_sr: bool,
    pub passes: u32,
    pub mfg_unlock: bool,
    pub mfg_multiplier: u32,
    pub nr_style: usize,
    pub nr_style_enabled: bool,
}

impl Default for DeployPresetOptions {
    fn default() -> Self {
        Self {
            pre_sr: true,
            passes: 1,
            mfg_unlock: false,
            mfg_multiplier: 4,
            nr_style: 0,
            nr_style_enabled: true,
        }
    }
}

#[doc(hidden)]
pub fn filter_games(all: &[GameEntry], store: &str) -> Vec<GameEntry> {
    all.iter()
        .filter(|g| crate::core::scan::matches_store_filter(&g.launcher, store))
        .cloned()
        .collect()
}

pub fn build_visual_list(
    filtered_games: &[GameEntry],
    current_store: &str,
) -> (Vec<GameEntry>, Vec<(&'static str, Vec<(usize, GameEntry)>)>) {
    if current_store == "All" {
        let mut visual_list = Vec::new();
        let mut parts = Vec::new();

        for &st in crate::core::scan::DEFAULT_STORE_ORDER {
            let mut matches = Vec::new();
            for g in filtered_games.iter().filter(|g| crate::core::scan::matches_store_filter(&g.launcher, st)) {
                let visual_idx = visual_list.len();
                visual_list.push(g.clone());
                matches.push((visual_idx, g.clone()));
            }
            if !matches.is_empty() {
                parts.push((st, matches));
            }
        }

        // Remaining other games
        let mut other_matches = Vec::new();
        for g in filtered_games.iter().filter(|g| !crate::core::scan::DEFAULT_STORE_ORDER.iter().any(|&st| crate::core::scan::matches_store_filter(&g.launcher, st))) {
            let visual_idx = visual_list.len();
            visual_list.push(g.clone());
            other_matches.push((visual_idx, g.clone()));
        }
        if !other_matches.is_empty() {
            parts.push(("Other", other_matches));
        }

        (visual_list, parts)
    } else {
        let visual_list = filtered_games.to_vec();
        let matched_store = crate::core::scan::DEFAULT_STORE_ORDER
            .iter()
            .find(|&&s| s == current_store)
            .copied()
            .unwrap_or("Custom");
        let parts = vec![(matched_store, visual_list.iter().enumerate().map(|(i, g)| (i, g.clone())).collect())];
        (visual_list, parts)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridNavDirection {
    Up,
    Down,
    Left,
    Right,
}

pub fn navigate_grid_2d(
    cur: usize,
    direction: GridNavDirection,
    parts: &[(&'static str, Vec<(usize, GameEntry)>)],
    cols_per_row: usize,
) -> usize {
    if parts.is_empty() {
        return 0;
    }

    let mut part_idx = None;
    for (s_idx, (_name, items)) in parts.iter().enumerate() {
        if let (Some(first), Some(last)) = (items.first(), items.last()) {
            if cur >= first.0 && cur <= last.0 {
                part_idx = Some(s_idx);
                break;
            }
        }
    }

    let s_idx = match part_idx {
        Some(i) => i,
        None => return cur,
    };

    let (_name, items) = &parts[s_idx];
    if items.is_empty() {
        return cur;
    }

    let start_idx = items[0].0;
    let len = items.len();
    let local_idx = cur.saturating_sub(start_idx);
    let local_row = local_idx / cols_per_row;
    let local_col = local_idx % cols_per_row;
    let total_rows = (len + cols_per_row - 1) / cols_per_row;

    match direction {
        GridNavDirection::Up => {
            if local_row > 0 {
                let target_local = (local_row - 1) * cols_per_row + local_col;
                start_idx + target_local.min(len - 1)
            } else if s_idx > 0 {
                let prev_items = &parts[s_idx - 1].1;
                if prev_items.is_empty() {
                    return cur;
                }
                let prev_start = prev_items[0].0;
                let prev_len = prev_items.len();
                let prev_rows = (prev_len + cols_per_row - 1) / cols_per_row;
                let prev_last_row = prev_rows.saturating_sub(1);
                let target_local = (prev_last_row * cols_per_row + local_col).min(prev_len - 1);
                prev_start + target_local
            } else {
                cur
            }
        }
        GridNavDirection::Down => {
            if local_row + 1 < total_rows {
                let target_local = (local_row + 1) * cols_per_row + local_col;
                start_idx + target_local.min(len - 1)
            } else if s_idx + 1 < parts.len() {
                let next_items = &parts[s_idx + 1].1;
                if next_items.is_empty() {
                    return cur;
                }
                let next_start = next_items[0].0;
                let next_len = next_items.len();
                let target_local = local_col.min(next_len - 1);
                next_start + target_local
            } else {
                cur
            }
        }
        GridNavDirection::Left => {
            cur.saturating_sub(1)
        }
        GridNavDirection::Right => {
            let total_games: usize = parts.iter().map(|(_, it)| it.len()).sum();
            (cur + 1).min(total_games.saturating_sub(1))
        }
    }
}

pub fn apply_profile_selection(
    idx: usize,
    mut backend_choice: Signal<String>,
    mut route_choice: Signal<String>,
    mut is_vanilla_selected: Signal<bool>,
) {
    match idx {
        0 => {
            is_vanilla_selected.set(false);
            backend_choice.set("reshade".to_string());
            route_choice.set("native".to_string());
        }
        1 => {
            is_vanilla_selected.set(false);
            backend_choice.set("reshade".to_string());
            route_choice.set("feeder".to_string());
        }
        2 => {
            is_vanilla_selected.set(false);
            backend_choice.set("optiscaler".to_string());
        }
        _ => {
            is_vanilla_selected.set(true);
        }
    }
}

/// Cycles the selected executable forward or backward among available executables.
pub fn cycle_game_exe(game: &GameEntry, forward: bool) -> Option<GameExeOption> {
    if game.available_exes.len() <= 1 {
        return None;
    }
    let cur_idx = game
        .available_exes
        .iter()
        .position(|opt| opt.path == game.exe_path)
        .unwrap_or(0);
    let count = game.available_exes.len();
    let next_idx = if forward {
        (cur_idx + 1) % count
    } else {
        (cur_idx + count - 1) % count
    };
    game.available_exes.get(next_idx).cloned()
}

/// Applies the chosen executable option to the target game entry within the global games list
/// and persists the update to state.
pub fn apply_cycled_exe(
    opt: &GameExeOption,
    target_dir: &std::path::Path,
    mut games: Signal<Vec<GameEntry>>,
    mut backend_choice: Signal<String>,
    mut route_choice: Signal<String>,
) {
    let mut current_games = games.read().clone();
    if let Some(pos) = current_games.iter().position(|g| g.dir == target_dir) {
        current_games[pos].exe_path = opt.path.clone();
        current_games[pos].exe_rel = opt.rel.clone();
        current_games[pos].api = opt.api.clone();
        current_games[pos].bitness = opt.bitness;
        current_games[pos].is_laa = opt.is_laa;

        let has_native_dlss = current_games[pos].has_native_dlss();
        let api_lower_opt = opt.api.to_lowercase();
        let is_dx12_opt = api_lower_opt.contains("12") || api_lower_opt.contains("d3d12");
        let is_vulkan_opt = api_lower_opt.contains("vulkan");
        let is_dx11_opt = (api_lower_opt.contains("11") || api_lower_opt == "d3d11") && !is_dx12_opt;
        if is_vulkan_opt || is_dx11_opt {
            current_games[pos].has_frame_generation = false;
        }
        current_games[pos].can_inject_fg = opt.bitness == 64 && has_native_dlss && !current_games[pos].has_frame_generation && (is_dx12_opt || is_dx11_opt);

        let is_deployed = current_games[pos].installed_route.is_some()
            || current_games[pos].optiscaler_installed
            || current_games[pos].addon_installed
            || current_games[pos].reshade_installed
            || current_games[pos].has_backup;
        if !is_deployed {
            let rec = crate::core::install_routes::recommended_route(&current_games[pos]);
            match rec {
                crate::core::install_routes::InstallRoute::Native => {
                    backend_choice.set("reshade".to_string());
                    route_choice.set("native".to_string());
                }
                crate::core::install_routes::InstallRoute::Feeder => {
                    backend_choice.set("reshade".to_string());
                    route_choice.set("feeder".to_string());
                }
                crate::core::install_routes::InstallRoute::OptiScaler => {
                    backend_choice.set("optiscaler".to_string());
                    route_choice.set("optiscaler".to_string());
                }
            }
        }

        let mut s = crate::core::state::load_state();
        s.cached_games = current_games.clone();
        let _ = crate::core::state::save_state(&s);
        games.set(current_games);
    }
}


/// Restores normal desktop window geometry upon exiting Big Picture mode.
pub fn exit_big_picture(
    desktop: dioxus::desktop::DesktopContext,
    mut is_open: Signal<bool>,
    mut show_exit_confirm: Signal<bool>,
) {
    crate::big_picture::logger::info("ui", "exit_big_picture: un-fullscreening, stripping topmost, and restoring standard desktop geometry");
    desktop.set_always_on_top(false);
    desktop.set_fullscreen(false);
    desktop.set_maximized(false);
    desktop.set_inner_size(dioxus::desktop::LogicalSize::new(
        crate::core::display::DEFAULT_DESKTOP_WIDTH as f64,
        crate::core::display::DEFAULT_DESKTOP_HEIGHT as f64,
    ));
    crate::core::display::restore_desktop_window_geometry();
    is_open.set(false);
    show_exit_confirm.set(false);

    // Follow-up restore after OS window manager completes un-fullscreen animation
    dioxus::prelude::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        desktop.set_always_on_top(false);
        crate::core::display::restore_desktop_window_geometry();
        crate::core::display::ensure_window_not_stranded();
    });
}

pub fn installed_profile_idx(game: &GameEntry) -> Option<usize> {
    let is_currently_modded = game.installed_route.is_some()
        || game.optiscaler_installed
        || game.addon_installed
        || game.reshade_installed;

    if !is_currently_modded {
        return None;
    }

    if game.installed_route.as_deref() == Some("native") {
        Some(0)
    } else if game.installed_route.as_deref() == Some("feeder") {
        Some(1)
    } else if game.optiscaler_installed || game.installed_route.as_deref() == Some("optiscaler") {
        Some(2)
    } else if game.reshade_installed {
        match recommended_route(game) {
            InstallRoute::Native => Some(0),
            InstallRoute::Feeder => Some(1),
            InstallRoute::OptiScaler => Some(2),
        }
    } else {
        None
    }
}

pub fn active_profile_idx(game: &GameEntry) -> usize {
    if let Some(idx) = installed_profile_idx(game) {
        idx
    } else {
        match recommended_route(game) {
            InstallRoute::Native => 0,
            InstallRoute::Feeder => 1,
            InstallRoute::OptiScaler => 2,
        }
    }
}

pub fn is_selection_matching_installed(
    game: &GameEntry,
    backend: &str,
    route: &str,
    is_vanilla: bool,
    pre_sr: bool,
    passes: u32,
    mfg: bool,
    mfg_mult: u32,
    nr_style: usize,
    nr_enabled: bool,
) -> bool {
    let is_currently_modded = game.installed_route.is_some()
        || game.optiscaler_installed
        || game.addon_installed
        || game.reshade_installed
        || game.has_backup;

    if is_vanilla {
        return !is_currently_modded;
    }

    if !is_currently_modded {
        return false;
    }

    // Check backend & route match
    let route_matches = if backend == "optiscaler" {
        game.installed_route.as_deref() == Some("optiscaler") || (game.optiscaler_installed && game.installed_route.is_none())
    } else if route == "native" {
        game.installed_route.as_deref() == Some("native")
    } else {
        game.installed_route.as_deref() == Some("feeder") || (game.reshade_installed && game.installed_route.is_none())
    };

    if !route_matches {
        return false;
    }

    // Tuning options match
    if backend == "optiscaler" {
        if game.optiscaler_presr != pre_sr || (game.optiscaler_passes > 0 && game.optiscaler_passes != passes) {
            return false;
        }
    }

    if game.mfg_unlock_installed != mfg {
        return false;
    }
    if mfg && game.mfg_multiplier > 0 && game.mfg_multiplier != mfg_mult {
        return false;
    }
    if game.nr_style_enabled != nr_enabled {
        return false;
    }
    if nr_enabled && game.nr_style != nr_style {
        return false;
    }

    true
}

#[derive(Clone, Debug, PartialEq)]
pub struct LaunchOverlayInfo {
    pub title: String,
    pub poster_url: Option<String>,
    pub is_exiting: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchLifecycleStage {
    /// Initial launch sequence (shows "Launching <Game>...", spinner, poster)
    Launching,
    /// Game window has taken foreground focus; background overlay pre-staged for exit
    RunningInForeground,
    /// Game process has terminated; transition overlay displayed before return
    ReturningToBigPicture,
    /// Return transition completed; overlay dismissed and library grid revealed
    Completed,
}

/// Pure helper resolving the dynamic launch/return overlay state across lifecycle stages.
pub fn resolve_launch_overlay_state(
    stage: LaunchLifecycleStage,
    game_name: &str,
    poster_url: Option<String>,
    lang: &str,
) -> Option<LaunchOverlayInfo> {
    match stage {
        LaunchLifecycleStage::Launching => Some(LaunchOverlayInfo {
            title: format!("{} {}...", crate::core::i18n::t(lang, "bp_launching_prefix"), game_name),
            poster_url,
            is_exiting: false,
        }),
        LaunchLifecycleStage::RunningInForeground | LaunchLifecycleStage::ReturningToBigPicture => Some(LaunchOverlayInfo {
            title: crate::core::i18n::t(lang, "bp_returning_to_bp").to_string(),
            poster_url,
            is_exiting: true,
        }),
        LaunchLifecycleStage::Completed => None,
    }
}

pub const SCROLL_PADDING_TOP_PX: f32 = 84.0;
pub const SCROLL_PADDING_BOTTOM_PX: f32 = 72.0;

/// Pure helper resolving the dynamic action text for the bottom controller HUD bar in Big Picture mode with localized tuning action labels.
pub fn resolve_options_hud_label_with_tuning(
    options_col: usize,
    options_sub_idx: usize,
    options_profile_idx: usize,
    is_exe_item: bool,
    is_selection_synced: bool,
    play_text: &str,
    apply_and_play_text: &str,
    cancel_text: &str,
    cycle_exe_text: &str,
    cycle_val_text: &str,
    toggle_opt_text: &str,
) -> String {
    if options_col == 1 {
        if is_exe_item {
            cycle_exe_text.to_string()
        } else if options_sub_idx == 1 {
            cycle_val_text.to_string()
        } else {
            toggle_opt_text.to_string()
        }
    } else if options_profile_idx == 5 {
        cancel_text.to_string()
    } else if is_selection_synced {
        play_text.to_string()
    } else {
        apply_and_play_text.to_string()
    }
}

/// Pure helper resolving the dynamic action text for the bottom controller HUD bar in Big Picture mode.
pub fn resolve_options_hud_label(
    options_col: usize,
    options_sub_idx: usize,
    options_profile_idx: usize,
    is_exe_item: bool,
    is_selection_synced: bool,
    play_text: &str,
    apply_and_play_text: &str,
    cancel_text: &str,
) -> String {
    resolve_options_hud_label_with_tuning(
        options_col,
        options_sub_idx,
        options_profile_idx,
        is_exe_item,
        is_selection_synced,
        play_text,
        apply_and_play_text,
        cancel_text,
        "Cycle Executable",
        "Cycle Value",
        "Toggle Option",
    )
}

/// Computes the count of completely visible grid rows in the Big Picture library grid.
pub fn calculate_visible_grid_rows(
    viewport_height: f32,
    top_chrome: f32,
    bottom_chrome: f32,
    card_height: f32,
    row_gap: f32,
) -> usize {
    let available = (viewport_height - top_chrome - bottom_chrome).max(0.0);
    if available < card_height || card_height <= 0.0 {
        return 0;
    }
    let unit = card_height + row_gap;
    ((available + row_gap) / unit).floor() as usize
}

fn scroll_focused_into_view(target_idx: usize) {
    let script = format!(
        r#"
    requestAnimationFrame(function() {{
        const container = document.querySelector('.bp-grid-container');
        if (!container) return;
        const el = document.getElementById('bp-card-{target_idx}') || document.querySelector('.bp-card-focused');
        if (!el) return;
        const elRect = el.getBoundingClientRect();
        const cRect = container.getBoundingClientRect();
        const delta = (elRect.top + elRect.height / 2) - (cRect.top + cRect.height / 2);
        const targetScroll = Math.max(0, container.scrollTop + delta);
        container.scrollTo({{ top: targetScroll, behavior: 'smooth' }});
    }});
    "#
    );
    let _ = dioxus::desktop::window().webview.evaluate_script(&script);
}

fn scroll_options_into_view() {
    let script = r#"
    requestAnimationFrame(function() {
        document.querySelector('.bp-inline-inspector')?.scrollIntoView({ behavior: 'smooth', block: 'center' });
    });
    "#;
    let _ = dioxus::desktop::window().webview.evaluate_script(script);
}

#[derive(Props, Clone, PartialEq)]
pub struct BigPictureProps {
    pub games: Signal<Vec<GameEntry>>,
    pub is_open: Signal<bool>,
    pub rust_theme: Signal<bool>,
    pub theme: Signal<String>,
    pub lang: Signal<String>,
}

#[component]
pub fn BigPictureOverlay(props: BigPictureProps) -> Element {
    let desktop = use_window();
    let _sleep_guard = use_hook(|| Arc::new(TvSleepInhibitor::acquire()));

    let brand_badge_data_uri = use_hook(|| {
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(BRAND_BADGE_WEBP);
        format!("data:image/webp;base64,{}", b64)
    });

    let mut selected_store = use_signal(|| "All".to_string());
    let mut focused_idx = use_signal(|| 0usize);
    let mut show_options = use_signal(|| false);
    let mut show_exit_confirm = use_signal(|| false);
    let mut exit_confirm_idx = use_signal(|| 1usize); // 0 = Exit, 1 = Cancel
    let mut options_col = use_signal(|| 0usize); // 0 = Left Column (Profiles), 1 = Right Column (Feature Tuning)
    let mut options_sub_idx = use_signal(|| 0usize); // In Column 1: 0 = Card/Checkbox, 1 = Stepper
    let mut options_profile_idx = use_signal(|| 0usize); // 0..4
    let mut options_tuning_idx = use_signal(|| 0usize); // 0..2
    let mut active_controller = use_signal(detect_controller_kind);
    let is_launching = use_signal(|| false);
    let is_applying = use_signal(|| false);
    let just_applied = use_signal(|| false);
    let launch_overlay = use_signal(|| None::<LaunchOverlayInfo>);
    let status_banner = use_signal(|| None::<String>);
    let mut cursor_hidden = use_signal(|| true);
    let mut cursor_timer_gen = use_signal(|| 0usize);
    let is_open = props.is_open;

    // Direct synchronization with Desktop App routing & preset state
    let mut last_focused_dir = use_signal(|| None::<std::path::PathBuf>);
    let mut backend_choice = use_signal(|| "reshade".to_string());
    let mut route_choice = use_signal(|| "native".to_string());
    let mut is_vanilla_selected = use_signal(|| false);

    let mut opti_pre_sr = use_signal(|| true);
    let mut opti_passes = use_signal(|| 1u32);
    let mut nr_style_choice = use_signal(|| true);
    let mut nr_style_preset = use_signal(|| 0usize);
    let mut mfg_choice = use_signal(|| false);
    let mut mfg_multiplier = use_signal(|| 4u32);

    let is_rtx_40 = use_hook(|| crate::core::gpu::detect_gpus().iter().any(|g| g.is_rtx_40));

    // Filter games by active store tab and build contiguous visual display list
    let all_games = props.games.read();
    let current_store = selected_store.read().clone();
    let filtered_games = filter_games(&all_games, &current_store);
    let (visual_display_list, store_partitions) = build_visual_list(&filtered_games, &current_store);
    drop(all_games);

    // Keep focus within bounds
    let count = visual_display_list.len();
    if count > 0 && *focused_idx.read() >= count {
        focused_idx.set(count - 1);
    }

    let active_game: Option<GameEntry> = if count > 0 {
        visual_display_list.get(*focused_idx.read()).cloned()
    } else {
        None
    };

    // EXACT Desktop App routing state initialization (mirrors src/ui/app.rs lines 724-765)
    let cur_dir = active_game.as_ref().map(|g| g.dir.clone());
    if *last_focused_dir.read() != cur_dir {
        last_focused_dir.set(cur_dir);
        is_vanilla_selected.set(false);

        if let Some(ref g) = active_game {
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
                    let rec = recommended_route(g);
                    route_choice.set(rec.as_str().to_string());
                }
            } else {
                // Unmodded / Vanilla: Automatically select the best compatible path!
                let rec = recommended_route(g);
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

    // EXACT Desktop App effective routing & advisories (mirrors src/ui/app.rs lines 2929-2962)
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

    let opti_advisory = active_game.as_ref().and_then(get_optiscaler_advisory);
    let native_advisory = active_game.as_ref().and_then(get_native_dlss_advisory);
    let mfg_advisory = active_game.as_ref().and_then(|g| {
        get_mfg_advisory_with_route(
            g,
            is_rtx_40,
            Some(&effective_backend),
            Some(&effective_route),
        )
    });

    let mut active_advisories: Vec<RouteAdvisory> = Vec::new();
    if !*is_vanilla_selected.read() {
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
    }

    let show_mfg = true;
    let show_pre_sr = effective_backend == "optiscaler";
    let show_nr_style = effective_backend == "reshade" || (effective_backend == "optiscaler" && *opti_pre_sr.read());

    let api_lower = active_game.as_ref().map(|g| g.api.to_lowercase()).unwrap_or_default();
    let is_dx11 = (api_lower.contains("11") || api_lower == "d3d11") && !api_lower.contains("12") && !api_lower.contains("d3d12");
    let is_vulkan = api_lower.contains("vulkan");
    let is_dx12 = api_lower.contains("12") || api_lower == "d3d12";

    let lang = props.lang.read();
    let feeder_label = if is_dx11 {
        crate::core::i18n::t(&lang, "feeder_label_dx11").to_string()
    } else if is_vulkan {
        crate::core::i18n::t(&lang, "feeder_label_vulkan").to_string()
    } else if is_dx12 {
        crate::core::i18n::t(&lang, "feeder_label_dx12").to_string()
    } else if api_lower.contains("opengl") {
        crate::core::i18n::t(&lang, "feeder_label_opengl").to_string()
    } else {
        crate::core::i18n::t(&lang, "feeder_label_general").to_string()
    };

    let opti_base = crate::core::i18n::t(&lang, "opt_optiscaler_dlssnr");
    let opti_display = if opti_advisory.is_some() {
        format!("{} ⚠️", opti_base)
    } else {
        opti_base.to_string()
    };

    let native_base = crate::core::i18n::t(&lang, "opt_native_dlss_bp");
    let native_display = if native_advisory.is_some() {
        format!("{} ⚠️", native_base)
    } else {
        native_base.to_string()
    };

    let rec_route = active_game.as_ref().map(recommended_route).unwrap_or(InstallRoute::Feeder);


    // Gamepad integration channel
    let cancel_flag = use_hook(|| Arc::new(AtomicBool::new(false)));
    let cancel_flag_clone = cancel_flag.clone();

    let desktop_future = desktop.clone();
    let desktop_keydown = desktop.clone();

    // Async Gamepad Listener
    use_future(move || {
        let cancel_flag = cancel_flag_clone.clone();
        let desktop = desktop_future.clone();

        async move {
            struct GamepadCancelGuard(Arc<AtomicBool>);
            impl Drop for GamepadCancelGuard {
                fn drop(&mut self) {
                    self.0.store(true, Ordering::Relaxed);
                }
            }
            let _guard = GamepadCancelGuard(cancel_flag.clone());
            let (tx, mut rx) = mpsc::unbounded_channel::<GamepadNavAction>();
            let _worker = start_gamepad_listener(tx, cancel_flag);

            while let Some(action) = rx.recv().await {
                if *is_launching.read() {
                    continue;
                }
                if !*cursor_hidden.read() {
                    cursor_hidden.set(true);
                }
                let st = selected_store.read().clone();
                let (v_list, parts) = build_visual_list(&filter_games(&props.games.read(), &st), &st);
                let target_game = v_list.get(*focused_idx.read()).cloned();

                let cur_backend = backend_choice.read().clone();
                let show_pre_sr = cur_backend == "optiscaler";
                let show_nr_style = cur_backend != "optiscaler" || *opti_pre_sr.read();
                let show_mfg = true;

                let show_exe = target_game.as_ref().map(|g| g.available_exes.len() > 1).unwrap_or(false);
                let is_vanilla = *is_vanilla_selected.read();
                let mut tun_items: Vec<&'static str> = Vec::new();
                if !is_vanilla {
                    if show_exe { tun_items.push("exe"); }
                    if show_pre_sr { tun_items.push("pre_sr"); }
                    if show_nr_style { tun_items.push("nr_style"); }
                    if show_mfg { tun_items.push("mfg"); }
                }

                match action {
                    GamepadNavAction::ControllerChanged(kind) => {
                        active_controller.set(kind);
                    }
                    GamepadNavAction::NavigateLeft => {
                        if *show_exit_confirm.read() {
                            exit_confirm_idx.set(0);
                        } else if *show_options.read() {
                            if *options_col.read() == 1 {
                                if *options_sub_idx.read() == 1 {
                                    options_sub_idx.set(0);
                                } else {
                                    options_col.set(0);
                                }
                            }
                        } else {
                            let cur = *focused_idx.read();
                            let next = navigate_grid_2d(cur, GridNavDirection::Left, &parts, 6);
                            if next != cur {
                                focused_idx.set(next);
                                scroll_focused_into_view(next);
                            }
                        }
                    }
                    GamepadNavAction::NavigateRight => {
                        if *show_exit_confirm.read() {
                            exit_confirm_idx.set(1);
                        } else if *show_options.read() {
                            if *options_col.read() == 0 {
                                if !tun_items.is_empty() {
                                    options_col.set(1);
                                    options_sub_idx.set(0);
                                    let max_t = tun_items.len().saturating_sub(1);
                                    if *options_tuning_idx.read() > max_t {
                                        options_tuning_idx.set(0);
                                    }
                                }
                            } else if *options_col.read() == 1 {
                                if *options_sub_idx.read() == 0 {
                                    let cur_t = *options_tuning_idx.read();
                                    let is_enabled = match tun_items.get(cur_t).copied() {
                                        Some("exe") => true,
                                        Some("pre_sr") => *opti_pre_sr.read(),
                                        Some("nr_style") => *nr_style_choice.read(),
                                        Some("mfg") => *mfg_choice.read(),
                                        _ => false,
                                    };
                                    if is_enabled {
                                        options_sub_idx.set(1);
                                    }
                                } else if *options_sub_idx.read() == 1 {
                                    let cur_t = *options_tuning_idx.read();
                                    if let Some(&item) = tun_items.get(cur_t) {
                                        match item {
                                            "exe" => {
                                                if let Some(target) = target_game.as_ref() {
                                                    if let Some(next_opt) = cycle_game_exe(target, true) {
                                                        apply_cycled_exe(&next_opt, &target.dir, props.games, backend_choice, route_choice);
                                                    }
                                                }
                                            }
                                            "pre_sr" => {
                                                let cur = *opti_passes.read();
                                                let next = if cur >= 3 { 1 } else { cur + 1 };
                                                opti_passes.set(next);
                                            }
                                            "nr_style" => {
                                                let cur = *nr_style_preset.read();
                                                nr_style_preset.set((cur + 1) % 3);
                                            }
                                            "mfg" => {
                                                let cur = *mfg_multiplier.read();
                                                let next = if cur >= 4 { 1 } else { cur + 1 };
                                                mfg_multiplier.set(next);
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        } else {
                            let cur = *focused_idx.read();
                            let next = navigate_grid_2d(cur, GridNavDirection::Right, &parts, 6);
                            if next != cur {
                                focused_idx.set(next);
                                scroll_focused_into_view(next);
                            }
                        }
                    }
                    GamepadNavAction::NavigateUp => {
                        if !*show_exit_confirm.read() {
                            if *show_options.read() {
                                if *options_col.read() == 0 {
                                    let cur = *options_profile_idx.read();
                                    let next = cur.saturating_sub(1);
                                    options_profile_idx.set(next);
                                    apply_profile_selection(next, backend_choice, route_choice, is_vanilla_selected);
                                } else {
                                    options_sub_idx.set(0);
                                    let cur = *options_tuning_idx.read();
                                    options_tuning_idx.set(cur.saturating_sub(1));
                                }
                            } else {
                                let cur = *focused_idx.read();
                                let next = navigate_grid_2d(cur, GridNavDirection::Up, &parts, 6);
                                if next != cur {
                                    focused_idx.set(next);
                                    scroll_focused_into_view(next);
                                }
                            }
                        }
                    }
                    GamepadNavAction::NavigateDown => {
                        if !*show_exit_confirm.read() {
                            if *show_options.read() {
                                if *options_col.read() == 0 {
                                    let cur = *options_profile_idx.read();
                                    let next = (cur + 1).min(3);
                                    options_profile_idx.set(next);
                                    apply_profile_selection(next, backend_choice, route_choice, is_vanilla_selected);
                                } else {
                                    options_sub_idx.set(0);
                                    let max_t = tun_items.len().saturating_sub(1);
                                    let cur = *options_tuning_idx.read();
                                    options_tuning_idx.set((cur + 1).min(max_t));
                                }
                            } else {
                                let cur = *focused_idx.read();
                                let next = navigate_grid_2d(cur, GridNavDirection::Down, &parts, 6);
                                if next != cur {
                                    focused_idx.set(next);
                                    scroll_focused_into_view(next);
                                }
                            }
                        }
                    }
                    GamepadNavAction::PageLeft => {
                        if *show_options.read() {
                            if *options_col.read() == 1 {
                                let cur_t = *options_tuning_idx.read();
                                if let Some(&item) = tun_items.get(cur_t) {
                                    match item {
                                        "exe" => {
                                            if let Some(target) = target_game.as_ref() {
                                                if let Some(prev_opt) = cycle_game_exe(target, false) {
                                                    apply_cycled_exe(&prev_opt, &target.dir, props.games, backend_choice, route_choice);
                                                }
                                            }
                                        }
                                        "pre_sr" => {
                                            let cur = *opti_passes.read();
                                            let prev = if cur <= 1 { 3 } else { cur - 1 };
                                            opti_passes.set(prev);
                                        }
                                        "nr_style" => {
                                            let cur = *nr_style_preset.read();
                                            nr_style_preset.set(if cur == 0 { 2 } else { cur - 1 });
                                        }
                                        "mfg" => {
                                            let cur = *mfg_multiplier.read();
                                            let prev = if cur <= 1 { 4 } else { cur - 1 };
                                            mfg_multiplier.set(prev);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        } else if !*show_exit_confirm.read() {
                            let cur = *focused_idx.read();
                            let next = navigate_grid_2d(cur, GridNavDirection::Up, &parts, 6);
                            if next != cur {
                                focused_idx.set(next);
                                scroll_focused_into_view(next);
                            }
                        }
                    }
                    GamepadNavAction::PageRight => {
                        if *show_options.read() {
                            if *options_col.read() == 1 {
                                let cur_t = *options_tuning_idx.read();
                                if let Some(&item) = tun_items.get(cur_t) {
                                    match item {
                                        "exe" => {
                                            if let Some(target) = target_game.as_ref() {
                                                if let Some(next_opt) = cycle_game_exe(target, true) {
                                                    apply_cycled_exe(&next_opt, &target.dir, props.games, backend_choice, route_choice);
                                                }
                                            }
                                        }
                                        "pre_sr" => {
                                            let cur = *opti_passes.read();
                                            let next = if cur >= 3 { 1 } else { cur + 1 };
                                            opti_passes.set(next);
                                        }
                                        "nr_style" => {
                                            let cur = *nr_style_preset.read();
                                            nr_style_preset.set((cur + 1) % 3);
                                        }
                                        "mfg" => {
                                            let cur = *mfg_multiplier.read();
                                            let next = if cur >= 4 { 1 } else { cur + 1 };
                                            mfg_multiplier.set(next);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        } else if !*show_exit_confirm.read() {
                            let cur = *focused_idx.read();
                            let next = navigate_grid_2d(cur, GridNavDirection::Down, &parts, 6);
                            if next != cur {
                                focused_idx.set(next);
                                scroll_focused_into_view(next);
                            }
                        }
                    }
                    GamepadNavAction::TertiaryAction => {
                        // Cycle Store Filter (Y / Triangle button) - disabled inside options & confirm modal
                        if !*show_options.read() && !*show_exit_confirm.read() {
                            let cur_store = selected_store.read().clone();
                            let idx = STORES.iter().position(|&s| s == cur_store).unwrap_or(0);
                            let next_idx = (idx + 1) % STORES.len();
                            selected_store.set(STORES[next_idx].to_string());
                            focused_idx.set(0);
                            scroll_focused_into_view(0);
                        }
                    }
                    GamepadNavAction::SecondaryAction => {
                        // Options Screen: [X] Apply configuration without launching
                        // Library Grid: Toggle Details / Options Screen (X / Square button)
                        if !*show_exit_confirm.read() {
                            let cur = *show_options.read();
                            if cur {
                                if let Some(target) = target_game {
                                    trigger_apply(
                                        target,
                                        backend_choice.read().clone(),
                                        route_choice.read().clone(),
                                        *is_vanilla_selected.read(),
                                        is_applying,
                                        just_applied,
                                        status_banner,
                                        props.games,
                                        DeployPresetOptions {
                                            pre_sr: *opti_pre_sr.read(),
                                            passes: *opti_passes.read(),
                                            mfg_unlock: *mfg_choice.read(),
                                            mfg_multiplier: *mfg_multiplier.read(),
                                            nr_style: *nr_style_preset.read(),
                                            nr_style_enabled: *nr_style_choice.read(),
                                        },
                                    );
                                }
                            } else {
                                let active_idx = target_game.as_ref().map(active_profile_idx).unwrap_or(0);
                                options_col.set(0);
                                options_sub_idx.set(0);
                                options_profile_idx.set(active_idx);
                                apply_profile_selection(active_idx, backend_choice, route_choice, is_vanilla_selected);
                                show_options.set(true);
                            }
                        }
                    }
                    GamepadNavAction::PrimaryAction => {
                        if *show_exit_confirm.read() {
                            if *exit_confirm_idx.read() == 0 {
                                exit_big_picture(desktop.clone(), is_open, show_exit_confirm);
                            } else {
                                show_exit_confirm.set(false);
                            }
                        } else if *show_options.read() {
                            if *options_col.read() == 1 {
                                let cur_t = *options_tuning_idx.read();
                                if let Some(&item) = tun_items.get(cur_t) {
                                    if item == "exe" {
                                        if let Some(target) = target_game.as_ref() {
                                            if let Some(next_opt) = cycle_game_exe(target, true) {
                                                apply_cycled_exe(&next_opt, &target.dir, props.games, backend_choice, route_choice);
                                            }
                                        }
                                    } else if *options_sub_idx.read() == 1 {
                                        // In Stepper: Cycle value
                                        match item {
                                            "pre_sr" => {
                                                let cur = *opti_passes.read();
                                                let next = if cur >= 3 { 1 } else { cur + 1 };
                                                opti_passes.set(next);
                                            }
                                            "nr_style" => {
                                                let cur = *nr_style_preset.read();
                                                nr_style_preset.set((cur + 1) % 3);
                                            }
                                            "mfg" => {
                                                let cur = *mfg_multiplier.read();
                                                let next = if cur >= 4 { 1 } else { cur + 1 };
                                                mfg_multiplier.set(next);
                                            }
                                            _ => {}
                                        }
                                    } else {
                                        // In Feature Tuning: Toggle checkbox
                                        match item {
                                            "pre_sr" => {
                                                let v = *opti_pre_sr.read();
                                                opti_pre_sr.set(!v);
                                            }
                                            "nr_style" => {
                                                let v = *nr_style_choice.read();
                                                nr_style_choice.set(!v);
                                            }
                                            "mfg" => {
                                                let v = *mfg_choice.read();
                                                mfg_choice.set(!v);
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            } else {
                                let cur_p = *options_profile_idx.read();
                                apply_profile_selection(cur_p, backend_choice, route_choice, is_vanilla_selected);
                                // [A] PLAY / APPLY & PLAY directly from selected preset card 0..3
                                if let Some(target) = target_game {
                                    if !*is_launching.read() && !*is_applying.read() {
                                        let should_dep = !is_selection_matching_installed(
                                            &target,
                                            &backend_choice.read(),
                                            &route_choice.read(),
                                            *is_vanilla_selected.read(),
                                            *opti_pre_sr.read(),
                                            *opti_passes.read(),
                                            *mfg_choice.read(),
                                            *mfg_multiplier.read(),
                                            *nr_style_preset.read(),
                                            *nr_style_choice.read(),
                                        );
                                        trigger_launch(
                                            target,
                                            backend_choice.read().clone(),
                                            route_choice.read().clone(),
                                            *is_vanilla_selected.read(),
                                            desktop.clone(),
                                            is_launching,
                                            launch_overlay,
                                            status_banner,
                                            props.games,
                                            DeployPresetOptions {
                                                pre_sr: *opti_pre_sr.read(),
                                                passes: *opti_passes.read(),
                                                mfg_unlock: *mfg_choice.read(),
                                                mfg_multiplier: *mfg_multiplier.read(),
                                                nr_style: *nr_style_preset.read(),
                                                nr_style_enabled: *nr_style_choice.read(),
                                            },
                                            should_dep,
                                            props.lang.read().clone(),
                                            is_open,
                                            show_exit_confirm,
                                        );
                                    }
                                }
                            }
                        } else {
                            // Direct Launch from Library Grid (no redeploy necessary)
                            if let Some(target) = target_game {
                                if !*is_launching.read() && !*is_applying.read() {
                                    trigger_launch(
                                        target,
                                        backend_choice.read().clone(),
                                        route_choice.read().clone(),
                                        *is_vanilla_selected.read(),
                                        desktop.clone(),
                                        is_launching,
                                        launch_overlay,
                                        status_banner,
                                        props.games,
                                        DeployPresetOptions {
                                            pre_sr: *opti_pre_sr.read(),
                                            passes: *opti_passes.read(),
                                            mfg_unlock: *mfg_choice.read(),
                                            mfg_multiplier: *mfg_multiplier.read(),
                                            nr_style: *nr_style_preset.read(),
                                            nr_style_enabled: *nr_style_choice.read(),
                                        },
                                        false,
                                        props.lang.read().clone(),
                                        is_open,
                                        show_exit_confirm,
                                    );
                                }
                            }
                        }
                    }
                    GamepadNavAction::Start => {
                        if *show_exit_confirm.read() {
                            show_exit_confirm.set(false);
                        } else if *show_options.read() {
                            show_options.set(false);
                        } else {
                            let active_idx = target_game.as_ref().map(active_profile_idx).unwrap_or(0);
                            options_col.set(0);
                            options_sub_idx.set(0);
                            options_profile_idx.set(active_idx);
                            apply_profile_selection(active_idx, backend_choice, route_choice, is_vanilla_selected);
                            show_options.set(true);
                        }
                    }
                    GamepadNavAction::Back => {
                        if *show_exit_confirm.read() {
                            show_exit_confirm.set(false);
                        } else if *show_options.read() {
                            if *options_col.read() == 1 {
                                if *options_sub_idx.read() == 1 {
                                    options_sub_idx.set(0);
                                } else {
                                    options_col.set(0);
                                }
                            } else {
                                show_options.set(false);
                            }
                        } else {
                            // Prompt confirmation modal instead of immediate exit
                            exit_confirm_idx.set(1);
                            show_exit_confirm.set(true);
                        }
                    }
                    GamepadNavAction::Guide => {
                        desktop.set_focus();
                    }
                }
            }
        }
    });

    // Background monitor detachment watcher: if streaming on a virtual display that is suddenly detached, cleanly exit BP
    let desktop_detachment = desktop.clone();
    use_future(move || {
        let desktop = desktop_detachment.clone();
        async move {
            let mut had_virtual = crate::core::display::is_virtual_display_present();
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                if !*is_open.read() {
                    break;
                }
                let cur_virtual = crate::core::display::is_virtual_display_present();
                if had_virtual && !cur_virtual {
                    crate::big_picture::logger::warn(
                        "ui",
                        "Virtual streaming display detached during active session; cleanly exiting Big Picture mode to prevent host desktop lock",
                    );
                    exit_big_picture(desktop.clone(), is_open, show_exit_confirm);
                    break;
                }
                had_virtual = cur_virtual;
            }
        }
    });

    // Keyboard navigation fallback
    let on_keydown = move |evt: KeyboardEvent| {
        if *is_launching.read() {
            return;
        }
        if !*cursor_hidden.read() {
            cursor_hidden.set(true);
        }
        let st = selected_store.read().clone();
        let (v_list, parts) = build_visual_list(&filter_games(&props.games.read(), &st), &st);
        let target_game = v_list.get(*focused_idx.read()).cloned();

        let cur_backend = backend_choice.read().clone();
        let show_pre_sr = cur_backend == "optiscaler";
        let show_nr_style = cur_backend != "optiscaler" || *opti_pre_sr.read();
        let show_mfg = true;

        let show_exe = target_game.as_ref().map(|g| g.available_exes.len() > 1).unwrap_or(false);
        let is_vanilla = *is_vanilla_selected.read();
        let mut tun_items: Vec<&'static str> = Vec::new();
        if !is_vanilla {
            if show_exe { tun_items.push("exe"); }
            if show_pre_sr { tun_items.push("pre_sr"); }
            if show_nr_style { tun_items.push("nr_style"); }
            if show_mfg { tun_items.push("mfg"); }
        }

        let key = evt.key();

        if *show_exit_confirm.read() {
            match key {
                Key::ArrowLeft | Key::ArrowRight | Key::ArrowUp | Key::ArrowDown => {
                    let cur = *exit_confirm_idx.read();
                    exit_confirm_idx.set(1 - cur);
                }
                Key::Enter => {
                    if *exit_confirm_idx.read() == 0 {
                        exit_big_picture(desktop_keydown.clone(), is_open, show_exit_confirm);
                    } else {
                        show_exit_confirm.set(false);
                    }
                }
                Key::Escape => {
                    show_exit_confirm.set(false);
                }
                Key::Character(ref s) if s == " " => {
                    if *exit_confirm_idx.read() == 0 {
                        exit_big_picture(desktop_keydown.clone(), is_open, show_exit_confirm);
                    } else {
                        show_exit_confirm.set(false);
                    }
                }
                Key::Character(ref s) if s == "b" || s == "B" => {
                    show_exit_confirm.set(false);
                }
                _ => {}
            }
            return;
        }

        match key {
            Key::ArrowLeft => {
                if *show_options.read() {
                    if *options_col.read() == 1 {
                        if *options_sub_idx.read() == 1 {
                            options_sub_idx.set(0);
                        } else {
                            options_col.set(0);
                        }
                    }
                } else {
                    let cur = *focused_idx.read();
                    let next = navigate_grid_2d(cur, GridNavDirection::Left, &parts, 6);
                    if next != cur {
                        focused_idx.set(next);
                        scroll_focused_into_view(next);
                    }
                }
            }
            Key::ArrowRight => {
                if *show_options.read() {
                    if *options_col.read() == 0 {
                        if !tun_items.is_empty() {
                            options_col.set(1);
                            options_sub_idx.set(0);
                            let max_t = tun_items.len().saturating_sub(1);
                            if *options_tuning_idx.read() > max_t {
                                options_tuning_idx.set(0);
                            }
                        }
                    } else if *options_col.read() == 1 {
                        if *options_sub_idx.read() == 0 {
                            let cur_t = *options_tuning_idx.read();
                            let is_enabled = match tun_items.get(cur_t).copied() {
                                Some("exe") => true,
                                Some("pre_sr") => *opti_pre_sr.read(),
                                Some("nr_style") => *nr_style_choice.read(),
                                Some("mfg") => *mfg_choice.read(),
                                _ => false,
                            };
                            if is_enabled {
                                options_sub_idx.set(1);
                            }
                        } else if *options_sub_idx.read() == 1 {
                            let cur_t = *options_tuning_idx.read();
                            if let Some(&item) = tun_items.get(cur_t) {
                                match item {
                                    "exe" => {
                                        if let Some(target) = target_game.as_ref() {
                                            if let Some(next_opt) = cycle_game_exe(target, true) {
                                                apply_cycled_exe(&next_opt, &target.dir, props.games, backend_choice, route_choice);
                                            }
                                        }
                                    }
                                    "pre_sr" => {
                                        let cur = *opti_passes.read();
                                        let next = if cur >= 3 { 1 } else { cur + 1 };
                                        opti_passes.set(next);
                                    }
                                    "nr_style" => {
                                        let cur = *nr_style_preset.read();
                                        nr_style_preset.set((cur + 1) % 3);
                                    }
                                    "mfg" => {
                                        let cur = *mfg_multiplier.read();
                                        let next = if cur >= 4 { 1 } else { cur + 1 };
                                        mfg_multiplier.set(next);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                } else {
                    let cur = *focused_idx.read();
                    let next = navigate_grid_2d(cur, GridNavDirection::Right, &parts, 6);
                    if next != cur {
                        focused_idx.set(next);
                        scroll_focused_into_view(next);
                    }
                }
            }
            Key::ArrowUp => {
                if *show_options.read() {
                    if *options_col.read() == 0 {
                        let cur = *options_profile_idx.read();
                        let next = cur.saturating_sub(1);
                        options_profile_idx.set(next);
                        apply_profile_selection(next, backend_choice, route_choice, is_vanilla_selected);
                    } else {
                        options_sub_idx.set(0);
                        let cur = *options_tuning_idx.read();
                        options_tuning_idx.set(cur.saturating_sub(1));
                    }
                } else {
                    let cur = *focused_idx.read();
                    let next = navigate_grid_2d(cur, GridNavDirection::Up, &parts, 6);
                    if next != cur {
                        focused_idx.set(next);
                        scroll_focused_into_view(next);
                    }
                }
            }
            Key::ArrowDown => {
                if *show_options.read() {
                    if *options_col.read() == 0 {
                        let cur = *options_profile_idx.read();
                        let next = (cur + 1).min(3);
                        options_profile_idx.set(next);
                        apply_profile_selection(next, backend_choice, route_choice, is_vanilla_selected);
                    } else {
                        options_sub_idx.set(0);
                        let max_t = tun_items.len().saturating_sub(1);
                        let cur = *options_tuning_idx.read();
                        options_tuning_idx.set((cur + 1).min(max_t));
                    }
                } else {
                    let cur = *focused_idx.read();
                    let next = navigate_grid_2d(cur, GridNavDirection::Down, &parts, 6);
                    if next != cur {
                        focused_idx.set(next);
                        scroll_focused_into_view(next);
                    }
                }
            }
            Key::PageUp => {
                if *show_options.read() {
                    if *options_col.read() == 1 {
                        let cur_t = *options_tuning_idx.read();
                        if let Some(&item) = tun_items.get(cur_t) {
                            match item {
                                "exe" => {
                                    if let Some(target) = target_game.as_ref() {
                                        if let Some(prev_opt) = cycle_game_exe(target, false) {
                                            apply_cycled_exe(&prev_opt, &target.dir, props.games, backend_choice, route_choice);
                                        }
                                    }
                                }
                                "pre_sr" => {
                                    let cur = *opti_passes.read();
                                    let next = if cur <= 1 { 3 } else { cur - 1 };
                                    opti_passes.set(next);
                                }
                                "nr_style" => {
                                    let cur = *nr_style_preset.read();
                                    nr_style_preset.set(if cur == 0 { 2 } else { cur - 1 });
                                }
                                "mfg" => {
                                    let cur = *mfg_multiplier.read();
                                    let next = if cur <= 1 { 4 } else { cur - 1 };
                                    mfg_multiplier.set(next);
                                }
                                _ => {}
                            }
                        }
                    }
                } else {
                    let cur = *focused_idx.read();
                    let next = navigate_grid_2d(cur, GridNavDirection::Up, &parts, 6);
                    if next != cur {
                        focused_idx.set(next);
                        scroll_focused_into_view(next);
                    }
                }
            }
            Key::PageDown => {
                if *show_options.read() {
                    if *options_col.read() == 1 {
                        let cur_t = *options_tuning_idx.read();
                        if let Some(&item) = tun_items.get(cur_t) {
                            match item {
                                "exe" => {
                                    if let Some(target) = target_game.as_ref() {
                                        if let Some(next_opt) = cycle_game_exe(target, true) {
                                            apply_cycled_exe(&next_opt, &target.dir, props.games, backend_choice, route_choice);
                                        }
                                    }
                                }
                                "pre_sr" => {
                                    let cur = *opti_passes.read();
                                    let next = if cur >= 3 { 1 } else { cur + 1 };
                                    opti_passes.set(next);
                                }
                                "nr_style" => {
                                    let cur = *nr_style_preset.read();
                                    nr_style_preset.set((cur + 1) % 3);
                                }
                                "mfg" => {
                                    let cur = *mfg_multiplier.read();
                                    let next = if cur >= 4 { 1 } else { cur + 1 };
                                    mfg_multiplier.set(next);
                                }
                                _ => {}
                            }
                        }
                    }
                } else {
                    let cur = *focused_idx.read();
                    let next = navigate_grid_2d(cur, GridNavDirection::Down, &parts, 6);
                    if next != cur {
                        focused_idx.set(next);
                        scroll_focused_into_view(next);
                    }
                }
            }
            Key::Tab => {
                if !*show_options.read() && !*show_exit_confirm.read() {
                    let cur_store = selected_store.read().clone();
                    let idx = STORES.iter().position(|&s| s == cur_store).unwrap_or(0);
                    let next_idx = (idx + 1) % STORES.len();
                    selected_store.set(STORES[next_idx].to_string());
                    focused_idx.set(0);
                    scroll_focused_into_view(0);
                }
            }
            Key::Escape => {
                if *show_options.read() {
                    if *options_col.read() == 1 {
                        if *options_sub_idx.read() == 1 {
                            options_sub_idx.set(0);
                        } else {
                            options_col.set(0);
                        }
                    } else {
                        show_options.set(false);
                    }
                } else {
                    exit_confirm_idx.set(1);
                    show_exit_confirm.set(true);
                }
            }
            Key::Enter => {
                if *show_options.read() {
                    if *options_col.read() == 1 {
                        let cur_t = *options_tuning_idx.read();
                        if let Some(&item) = tun_items.get(cur_t) {
                            if item == "exe" {
                                if let Some(target) = target_game.as_ref() {
                                    if let Some(next_opt) = cycle_game_exe(target, true) {
                                        apply_cycled_exe(&next_opt, &target.dir, props.games, backend_choice, route_choice);
                                    }
                                }
                            } else if *options_sub_idx.read() == 1 {
                                // Cycle stepper value
                                match item {
                                    "pre_sr" => {
                                        let cur = *opti_passes.read();
                                        let next = if cur >= 3 { 1 } else { cur + 1 };
                                        opti_passes.set(next);
                                    }
                                    "nr_style" => {
                                        let cur = *nr_style_preset.read();
                                        nr_style_preset.set((cur + 1) % 3);
                                    }
                                    "mfg" => {
                                        let cur = *mfg_multiplier.read();
                                        let next = if cur >= 4 { 1 } else { cur + 1 };
                                        mfg_multiplier.set(next);
                                    }
                                    _ => {}
                                }
                            } else {
                                match item {
                                    "pre_sr" => {
                                        let v = *opti_pre_sr.read();
                                        opti_pre_sr.set(!v);
                                    }
                                    "nr_style" => {
                                        let v = *nr_style_choice.read();
                                        nr_style_choice.set(!v);
                                    }
                                    "mfg" => {
                                        let v = *mfg_choice.read();
                                        mfg_choice.set(!v);
                                    }
                                    _ => {}
                                }
                            }
                        }
                    } else {
                        let cur_p = *options_profile_idx.read();
                        apply_profile_selection(cur_p, backend_choice, route_choice, is_vanilla_selected);
                        // [A] PLAY / APPLY & PLAY directly from selected preset card 0..3
                        if let Some(target) = target_game {
                            if !*is_launching.read() && !*is_applying.read() {
                                let should_dep = !is_selection_matching_installed(
                                    &target,
                                    &backend_choice.read(),
                                    &route_choice.read(),
                                    *is_vanilla_selected.read(),
                                    *opti_pre_sr.read(),
                                    *opti_passes.read(),
                                    *mfg_choice.read(),
                                    *mfg_multiplier.read(),
                                    *nr_style_preset.read(),
                                    *nr_style_choice.read(),
                                );
                                trigger_launch(
                                    target,
                                    backend_choice.read().clone(),
                                    route_choice.read().clone(),
                                    *is_vanilla_selected.read(),
                                    desktop_keydown.clone(),
                                    is_launching,
                                    launch_overlay,
                                    status_banner,
                                    props.games,
                                    DeployPresetOptions {
                                        pre_sr: *opti_pre_sr.read(),
                                        passes: *opti_passes.read(),
                                        mfg_unlock: *mfg_choice.read(),
                                        mfg_multiplier: *mfg_multiplier.read(),
                                        nr_style: *nr_style_preset.read(),
                                        nr_style_enabled: *nr_style_choice.read(),
                                    },
                                    should_dep,
                                    props.lang.read().clone(),
                                    is_open,
                                    show_exit_confirm,
                                );
                            }
                        }
                    }
                } else {
                    if let Some(target) = target_game {
                        if !*is_launching.read() && !*is_applying.read() {
                            trigger_launch(
                                target,
                                backend_choice.read().clone(),
                                route_choice.read().clone(),
                                *is_vanilla_selected.read(),
                                desktop_keydown.clone(),
                                is_launching,
                                launch_overlay,
                                status_banner,
                                props.games,
                                DeployPresetOptions {
                                    pre_sr: *opti_pre_sr.read(),
                                    passes: *opti_passes.read(),
                                    mfg_unlock: *mfg_choice.read(),
                                    mfg_multiplier: *mfg_multiplier.read(),
                                    nr_style: *nr_style_preset.read(),
                                    nr_style_enabled: *nr_style_choice.read(),
                                },
                                false,
                                props.lang.read().clone(),
                                is_open,
                                show_exit_confirm,
                            );
                        }
                    }
                }
            }
            Key::Character(ref s) if s == "o" || s == "O" => {
                if *show_options.read() {
                    show_options.set(false);
                } else {
                    let active_idx = target_game.as_ref().map(active_profile_idx).unwrap_or(0);
                    options_col.set(0);
                    options_profile_idx.set(active_idx);
                    apply_profile_selection(active_idx, backend_choice, route_choice, is_vanilla_selected);
                    show_options.set(true);
                    scroll_options_into_view();
                }
            }
            Key::Character(ref s) if s == " " || s == "x" || s == "X" => {
                let cur = *show_options.read();
                if cur {
                    if let Some(target) = target_game {
                        trigger_apply(
                            target,
                            backend_choice.read().clone(),
                            route_choice.read().clone(),
                            *is_vanilla_selected.read(),
                            is_applying,
                            just_applied,
                            status_banner,
                            props.games,
                            DeployPresetOptions {
                                pre_sr: *opti_pre_sr.read(),
                                passes: *opti_passes.read(),
                                mfg_unlock: *mfg_choice.read(),
                                mfg_multiplier: *mfg_multiplier.read(),
                                nr_style: *nr_style_preset.read(),
                                nr_style_enabled: *nr_style_choice.read(),
                            },
                        );
                    }
                } else {
                    let active_idx = target_game.as_ref().map(active_profile_idx).unwrap_or(0);
                    options_col.set(0);
                    options_profile_idx.set(active_idx);
                    apply_profile_selection(active_idx, backend_choice, route_choice, is_vanilla_selected);
                    show_options.set(true);
                    scroll_options_into_view();
                }
            }
            _ => {}
        }
    };

    // Controller button glyph helpers
    let (play_glyph_cls, play_glyph_sym) = active_controller.read().play_glyph();
    let (patch_glyph_cls, patch_glyph_sym) = active_controller.read().patch_glyph();
    let (filter_glyph_cls, filter_glyph_sym) = active_controller.read().filter_glyph();
    let (back_glyph_cls, back_glyph_sym) = active_controller.read().back_glyph();
    let (menu_glyph_cls, menu_glyph_sym) = active_controller.read().menu_glyph();
    let (bumper_l, bumper_r) = active_controller.read().bumper_labels();
    let controller_badge = active_controller.read().badge_text();

    // Check if current options match deployed state on disk
    let is_selection_synced = active_game.as_ref().map(|g| {
        is_selection_matching_installed(
            g,
            &effective_backend,
            &effective_route,
            *is_vanilla_selected.read(),
            *opti_pre_sr.read(),
            *opti_passes.read(),
            *mfg_choice.read(),
            *mfg_multiplier.read(),
            *nr_style_preset.read(),
            *nr_style_choice.read(),
        )
    }).unwrap_or(true);

    // Resolve dynamic hero backdrop URL
    let hero_url = if let Some(ref g) = active_game {
        let hero_p = crate::core::state::get_appdata_dir()
            .join("art")
            .join(format!("{}-hero.jpg", crate::core::steamart::key_for_dir(&g.dir)));
        let hero_key = crate::core::steamart::key_for_dir(&g.dir);
        if hero_p.exists() {
            Some(format!("http://dlss-art.localhost/art/{}-hero.jpg", hero_key))
        } else {
            g.poster.as_ref().map(|p| crate::core::steamart::normalize_art_uri(p))
        }
    } else {
        None
    };

    let time_str = chrono_time_str();
    let theme_val = props.theme.read().clone();
    let rust_val = if *props.rust_theme.read() { "true" } else { "false" };

    rsx! {
        div {
            class: if *cursor_hidden.read() { "bp-root bp-hide-cursor" } else { "bp-root" },
            tabindex: "0",
            autofocus: true,
            onkeydown: on_keydown,
            onmousemove: move |_| {
                if *cursor_hidden.read() {
                    cursor_hidden.set(false);
                }
                let cur_gen = *cursor_timer_gen.read() + 1;
                cursor_timer_gen.set(cur_gen);
                spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(2500)).await;
                    if *cursor_timer_gen.read() == cur_gen {
                        cursor_hidden.set(true);
                    }
                });
            },
            "data-theme": "{theme_val}",
            "data-rust-theme": "{rust_val}",

            style { "{BP_STYLE_CSS}" }

            // Dynamic Hero Vignette Backdrop
            if let Some(h) = hero_url {
                div {
                    class: "bp-backdrop",
                    style: "background-image: linear-gradient(to bottom, rgba(10,12,16,0.60), rgba(8,10,14,0.94)), url('{h}');"
                }
            } else {
                div { class: "bp-backdrop bp-backdrop-fallback" }
            }

            // Top Header Bar
            header {
                class: "bp-header",
                div {
                    class: "bp-brand",
                    img {
                        class: "bp-brand-badge",
                        src: "{brand_badge_data_uri}",
                        alt: "DLSS STUDIO",
                    }
                    span { class: "bp-brand-pill", "{crate::core::i18n::t(&props.lang.read(), \"bp_brand_pill\")}" }
                }

                if !*show_options.read() && !*show_exit_confirm.read() {
                    div {
                        class: "bp-store-selector",
                        style: "cursor: pointer;",
                        onclick: {
                            let cur_s = selected_store.read().clone();
                            move |_| {
                                let idx = STORES.iter().position(|&s| s == cur_s).unwrap_or(0);
                                let next_idx = (idx + 1) % STORES.len();
                                selected_store.set(STORES[next_idx].to_string());
                                focused_idx.set(0);
                            }
                        },
                        span { class: "{filter_glyph_cls}", "{filter_glyph_sym}" }
                        span { class: "bp-store-label", "{crate::core::i18n::t(&props.lang.read(), \"bp_store_label\")}: " }
                        span { class: "bp-store-active", "{current_store}" }
                    }
                }

                div {
                    class: "bp-header-right",
                    span { class: "bp-controller-badge", "{controller_badge}" }
                    span { class: "bp-clock", "{time_str}" }
                    button {
                        class: "bp-btn-exit",
                        onclick: move |_| {
                            exit_confirm_idx.set(1);
                            show_exit_confirm.set(true);
                        },
                        span { class: "{back_glyph_cls}", "{back_glyph_sym}" }
                        span { " {crate::core::i18n::t(&props.lang.read(), \"bp_btn_desktop\")}" }
                    }
                }
            }

            // Status notification bar (floating toast, zero layout shift)
            if let Some(msg) = status_banner.read().as_ref() {
                div {
                    class: "bp-status-toast-container",
                    div {
                        class: if msg.to_lowercase().contains("error") || msg.to_lowercase().contains("failed") {
                            "bp-status-banner error"
                        } else {
                            "bp-status-banner"
                        },
                        "{msg}"
                    }
                }
            }

            // Main Library Grid OR Dedicated Options Screen
            if *show_options.read() {
                {
                    if let Some(ref game) = active_game {
                        let g_name = game.name.clone();
                        let g_api = game.api.clone();
                        let is_dx12 = game.api == "DirectX 12" || game.api.contains("12");
                        let has_dlss = game.dlss_version.is_some();
                        let dlss_label = if let Some(ref v) = game.dlss_version {
                            crate::core::scan::short_version(v)
                        } else {
                            crate::core::i18n::t(&props.lang.read(), "badge_no_dlss").to_string()
                        };
                        let g_opti = game.optiscaler_installed;
                        let has_addon = game.mfg_unlock_installed || game.has_backup;
                        let poster_opt = game.poster.as_ref().map(|p| crate::core::steamart::normalize_art_uri(p));
                        let is_dlss5 = game.is_dlss5_patched();

                        let (mod_badge_label, mod_badge_class) = if game.mfg_unlock_installed || game.has_frame_generation {
                            (crate::core::i18n::t(&props.lang.read(), "bp_badge_mfg_active"), "bp-badge-emerald")
                        } else if game.optiscaler_installed {
                            (crate::core::i18n::t(&props.lang.read(), "bp_badge_optiscaler_active"), "bp-badge-cyan")
                        } else if game.dlss_version.is_some() || is_dlss5 {
                            (crate::core::i18n::t(&props.lang.read(), "bp_badge_dlss_active"), "bp-badge-emerald")
                        } else {
                            (crate::core::i18n::t(&props.lang.read(), "bp_badge_unpatched_vanilla"), "bp-badge-muted")
                        };

                        let installed_profile = installed_profile_idx(game);

                        rsx! {
                        main {
                            class: "bp-options-screen",
                            div {
                                class: "bp-options-layout",

                                // LEFT COLUMN: Poster Card & Metadata
                                div {
                                    class: "bp-options-left",
                                    div {
                                        class: "bp-options-poster",
                                        if let Some(ref p_url) = poster_opt {
                                            img { src: "{p_url}", alt: "{g_name}" }
                                        } else {
                                            div { class: "placeholder", "{&g_name[0..2.min(g_name.len())]}" }
                                        }
                                        div {
                                            class: "status",
                                            span { class: if has_dlss { "dot-s on" } else { "dot-s" } }
                                            "{dlss_label}"
                                            span { class: if g_opti || has_addon { "dot-s on" } else { "dot-s" }, style: "margin-inline-start:8px" }
                                            if g_opti { "OptiScaler" } else { "{crate::core::i18n::t(&props.lang.read(), \"bp_addon_label\")}" }
                                        }
                                    }
                                    div {
                                        class: "bp-options-left-meta",
                                        span { class: "badge", "{game.launcher.to_uppercase()}" }
                                        span { class: if is_dx12 { "badge dx12" } else { "badge" }, "{g_api}" }
                                        span { class: "tag {mod_badge_class}", "{mod_badge_label}" }
                                    }
                                    div {
                                        class: "bp-options-path",
                                        title: "{game.exe_path.display()}",
                                        "{game.exe_path.file_name().and_then(|n| n.to_str()).unwrap_or(\"\")}"
                                    }
                                }

                                // CENTER COLUMN: Title, Profiles Selector, Action Buttons
                                div {
                                    class: "bp-options-center",
                                    div {
                                        class: "bp-options-header",
                                        div {
                                            class: "bp-options-header-left",
                                            h1 { class: "bp-options-title", "{game.name}" }
                                            div { class: "bp-options-subtitle", "{game.dir.display()}" }
                                        }
                                    }
                                    div {
                                        class: "bp-options-indicators",
                                        {
                                            let is_game_patched = game.is_dlss5_patched();
                                            let pill_state = if !is_selection_synced {
                                                "apply-and-play"
                                            } else if is_game_patched {
                                                "modded"
                                            } else {
                                                "vanilla"
                                            };
                                            let play_pill_text = if is_selection_synced {
                                                format!(" {}", crate::core::i18n::t(&props.lang.read(), "bp_hud_play"))
                                            } else {
                                                format!(" {}", crate::core::i18n::t(&props.lang.read(), "bp_apply_and_play"))
                                            };
                                            let apply_pill_cls = if *is_applying.read() {
                                                "applying"
                                            } else if *just_applied.read() {
                                                "applied-success"
                                            } else if is_selection_synced {
                                                "synced"
                                            } else {
                                                "needs-apply"
                                            };
                                            let (apply_glyph_sym, apply_pill_text) = if *is_applying.read() {
                                                (patch_glyph_sym, format!(" {}", crate::core::i18n::t(&props.lang.read(), "bp_hud_applying")))
                                            } else if *just_applied.read() {
                                                ("✓", format!(" {}", crate::core::i18n::t(&props.lang.read(), "bp_hud_applied")))
                                            } else {
                                                (patch_glyph_sym, format!(" {}", crate::core::i18n::t(&props.lang.read(), "bp_hud_apply")))
                                            };

                                            let is_in_tuning = *options_col.read() == 1;
                                            let show_exe = game.available_exes.len() > 1;
                                            let show_pre_sr = effective_backend == "optiscaler";
                                            let show_nr_style = effective_backend == "reshade";
                                            let show_mfg = game.bitness == 64 && game.can_inject_fg;
                                            let mut col1_tun_items: Vec<&'static str> = Vec::new();
                                            if show_exe { col1_tun_items.push("exe"); }
                                            if show_pre_sr { col1_tun_items.push("pre_sr"); }
                                            if show_nr_style { col1_tun_items.push("nr_style"); }
                                            if show_mfg { col1_tun_items.push("mfg"); }

                                            let cur_t = *options_tuning_idx.read();
                                            let focused_item = col1_tun_items.get(cur_t).copied();
                                            let is_cur_exe = focused_item == Some("exe");
                                            let is_sub_stepper = *options_sub_idx.read() == 1;

                                            let (action_pill_cls, action_pill_text) = if is_in_tuning {
                                                let txt = if is_cur_exe {
                                                    format!(" {}", crate::core::i18n::t(&props.lang.read(), "bp_hud_cycle_executable"))
                                                } else if is_sub_stepper {
                                                    format!(" {}", crate::core::i18n::t(&props.lang.read(), "bp_hud_cycle_value"))
                                                } else {
                                                    format!(" {}", crate::core::i18n::t(&props.lang.read(), "bp_hud_toggle_option"))
                                                };
                                                ("bp-indicator-pill launch active tune-action".to_string(), txt)
                                            } else {
                                                (format!("bp-indicator-pill launch active {}", pill_state), play_pill_text)
                                            };

                                            let back_pill_text = if is_in_tuning {
                                                format!(" {}", crate::core::i18n::t(&props.lang.read(), "bp_hud_back"))
                                            } else {
                                                format!(" {}", crate::core::i18n::t(&props.lang.read(), "bp_exit_btn_cancel"))
                                            };

                                            let g_launch = game.clone();
                                            let g_apply = game.clone();
                                            let g_tune = game.clone();
                                            let games_tune = props.games;
                                            let opts_deploy = DeployPresetOptions {
                                                pre_sr: *opti_pre_sr.read(),
                                                passes: *opti_passes.read(),
                                                mfg_unlock: *mfg_choice.read(),
                                                mfg_multiplier: *mfg_multiplier.read(),
                                                nr_style: *nr_style_preset.read(),
                                                nr_style_enabled: *nr_style_choice.read(),
                                            };
                                            let opts_launch = opts_deploy.clone();
                                            let desktop_launch = desktop.clone();
                                            let lang_for_launch = props.lang.read().clone();

                                            rsx! {
                                                span {
                                                    class: "{action_pill_cls}",
                                                    onclick: move |_| {
                                                        if is_in_tuning {
                                                            if let Some(item) = focused_item {
                                                                if item == "exe" {
                                                                    if let Some(next_opt) = cycle_game_exe(&g_tune, true) {
                                                                        apply_cycled_exe(&next_opt, &g_tune.dir, games_tune, backend_choice, route_choice);
                                                                    }
                                                                } else if *options_sub_idx.read() == 1 {
                                                                    match item {
                                                                        "pre_sr" => {
                                                                            let cur = *opti_passes.read();
                                                                            let next = if cur >= 3 { 1 } else { cur + 1 };
                                                                            opti_passes.set(next);
                                                                        }
                                                                        "nr_style" => {
                                                                            let cur = *nr_style_preset.read();
                                                                            nr_style_preset.set((cur + 1) % 3);
                                                                        }
                                                                        "mfg" => {
                                                                            let cur = *mfg_multiplier.read();
                                                                            let next = if cur >= 4 { 1 } else { cur + 1 };
                                                                            mfg_multiplier.set(next);
                                                                        }
                                                                        _ => {}
                                                                    }
                                                                } else {
                                                                    match item {
                                                                        "pre_sr" => {
                                                                            let v = *opti_pre_sr.read();
                                                                            opti_pre_sr.set(!v);
                                                                        }
                                                                        "nr_style" => {
                                                                            let v = *nr_style_choice.read();
                                                                            nr_style_choice.set(!v);
                                                                        }
                                                                        "mfg" => {
                                                                            let v = *mfg_choice.read();
                                                                            mfg_choice.set(!v);
                                                                        }
                                                                        _ => {}
                                                                    }
                                                                }
                                                            }
                                                        } else if !*is_launching.read() && !*is_applying.read() {
                                                            let should_dep = !is_selection_matching_installed(
                                                                &g_launch,
                                                                &backend_choice.read(),
                                                                &route_choice.read(),
                                                                *is_vanilla_selected.read(),
                                                                *opti_pre_sr.read(),
                                                                *opti_passes.read(),
                                                                *mfg_choice.read(),
                                                                *mfg_multiplier.read(),
                                                                *nr_style_preset.read(),
                                                                *nr_style_choice.read(),
                                                            );
                                                            trigger_launch(
                                                                g_launch.clone(),
                                                                backend_choice.read().clone(),
                                                                route_choice.read().clone(),
                                                                *is_vanilla_selected.read(),
                                                                desktop_launch.clone(),
                                                                is_launching,
                                                                launch_overlay,
                                                                status_banner,
                                                                props.games,
                                                                opts_launch.clone(),
                                                                should_dep,
                                                                lang_for_launch.clone(),
                                                                is_open,
                                                                show_exit_confirm,
                                                            );
                                                        }
                                                    },
                                                    span { class: "{play_glyph_cls}", "{play_glyph_sym}" }
                                                    span { "{action_pill_text}" }
                                                }
                                                span {
                                                    class: format!("bp-indicator-pill apply {}", apply_pill_cls),
                                                    onclick: move |_| {
                                                        if !*is_applying.read() {
                                                            trigger_apply(
                                                                g_apply.clone(),
                                                                backend_choice.read().clone(),
                                                                route_choice.read().clone(),
                                                                *is_vanilla_selected.read(),
                                                                is_applying,
                                                                just_applied,
                                                                status_banner,
                                                                props.games,
                                                                opts_deploy.clone(),
                                                            );
                                                        }
                                                    },
                                                    span { class: "{patch_glyph_cls}", "{apply_glyph_sym}" }
                                                    span { "{apply_pill_text}" }
                                                }
                                                span {
                                                    class: "bp-indicator-pill back",
                                                    onclick: move |_| {
                                                        if *options_col.read() == 1 {
                                                            if *options_sub_idx.read() == 1 {
                                                                options_sub_idx.set(0);
                                                            } else {
                                                                options_col.set(0);
                                                            }
                                                        } else {
                                                            show_options.set(false);
                                                        }
                                                    },
                                                    span { class: "{back_glyph_cls}", "{back_glyph_sym}" }
                                                    span { "{back_pill_text}" }
                                                }
                                            }
                                        }
                                    }

                                    div { class: "bp-drawer-title", "{crate::core::i18n::t(&props.lang.read(), \"bp_select_deployment_profile\")}" }
                                    div {
                                        class: "bp-profile-list",

                                        // 1. Direct Native DLSS 5
                                        {
                                            let is_deployed = installed_profile == Some(0);
                                            let is_selected = !*is_vanilla_selected.read() && effective_backend == "reshade" && effective_route == "native";
                                            let is_focused_0 = *options_col.read() == 0 && *options_profile_idx.read() == 0;
                                            let is_rec_0 = rec_route == InstallRoute::Native;
                                            rsx! {
                                                div {
                                                    class: format!("bp-profile-item {} {} {}",
                                                        if is_deployed { "deployed" } else { "" },
                                                        if is_selected { "selected" } else { "" },
                                                        if is_focused_0 { "bp-cursor-focused" } else { "" }
                                                    ),
                                                    onclick: {
                                                        move |_| {
                                                            options_col.set(0);
                                                            options_profile_idx.set(0);
                                                            apply_profile_selection(0, backend_choice, route_choice, is_vanilla_selected);
                                                        }
                                                    },
                                                    div {
                                                        class: "bp-profile-item-left",
                                                        span { class: "bp-profile-cursor", if is_focused_0 { "▶" } else { "" } }
                                                        span { class: "bp-profile-name", "{native_display}" }
                                                    }
                                                    div {
                                                        class: "bp-profile-item-right",
                                                        if is_rec_0 {
                                                            span { class: "bp-tag-rec", "{crate::core::i18n::t(&props.lang.read(), \"bp_tag_recommended\")}" }
                                                        }
                                                        if is_deployed {
                                                            span { class: "bp-profile-badge-active", "{crate::core::i18n::t(&props.lang.read(), \"bp_badge_active\")}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // 2. DLSS 5 Feeder
                                        {
                                            let is_deployed = installed_profile == Some(1);
                                            let is_selected = !*is_vanilla_selected.read() && effective_backend == "reshade" && effective_route == "feeder";
                                            let is_focused_1 = *options_col.read() == 0 && *options_profile_idx.read() == 1;
                                            let is_rec_1 = rec_route == InstallRoute::Feeder;
                                            rsx! {
                                                div {
                                                    class: format!("bp-profile-item {} {} {}",
                                                        if is_deployed { "deployed" } else { "" },
                                                        if is_selected { "selected" } else { "" },
                                                        if is_focused_1 { "bp-cursor-focused" } else { "" }
                                                    ),
                                                    onclick: {
                                                        move |_| {
                                                            options_col.set(0);
                                                            options_profile_idx.set(1);
                                                            apply_profile_selection(1, backend_choice, route_choice, is_vanilla_selected);
                                                        }
                                                    },
                                                    div {
                                                        class: "bp-profile-item-left",
                                                        span { class: "bp-profile-cursor", if is_focused_1 { "▶" } else { "" } }
                                                        span { class: "bp-profile-name", "{feeder_label}" }
                                                    }
                                                    div {
                                                        class: "bp-profile-item-right",
                                                        if is_rec_1 {
                                                            span { class: "bp-tag-rec", "{crate::core::i18n::t(&props.lang.read(), \"bp_tag_recommended\")}" }
                                                        }
                                                        if is_deployed {
                                                            span { class: "bp-profile-badge-active", "{crate::core::i18n::t(&props.lang.read(), \"bp_badge_active\")}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // 3. OptiScaler Pre-SR + Frame Gen
                                        {
                                            let is_deployed = installed_profile == Some(2);
                                            let is_selected = !*is_vanilla_selected.read() && effective_backend == "optiscaler";
                                            let is_focused_2 = *options_col.read() == 0 && *options_profile_idx.read() == 2;
                                            let is_rec_2 = rec_route == InstallRoute::OptiScaler;
                                            rsx! {
                                                div {
                                                    class: format!("bp-profile-item {} {} {}",
                                                        if is_deployed { "deployed" } else { "" },
                                                        if is_selected { "selected" } else { "" },
                                                        if is_focused_2 { "bp-cursor-focused" } else { "" }
                                                    ),
                                                    onclick: {
                                                        move |_| {
                                                            options_col.set(0);
                                                            options_profile_idx.set(2);
                                                            apply_profile_selection(2, backend_choice, route_choice, is_vanilla_selected);
                                                        }
                                                    },
                                                    div {
                                                        class: "bp-profile-item-left",
                                                        span { class: "bp-profile-cursor", if is_focused_2 { "▶" } else { "" } }
                                                        span { class: "bp-profile-name", "{opti_display}" }
                                                    }
                                                    div {
                                                        class: "bp-profile-item-right",
                                                        if is_rec_2 {
                                                            span { class: "bp-tag-rec", "{crate::core::i18n::t(&props.lang.read(), \"bp_tag_recommended\")}" }
                                                        }
                                                        if is_deployed {
                                                            span { class: "bp-profile-badge-active", "{crate::core::i18n::t(&props.lang.read(), \"bp_badge_active\")}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // 4. Vanilla Restore
                                        {
                                            let is_deployed = installed_profile == Some(3);
                                            let is_selected = *is_vanilla_selected.read();
                                            let is_focused_3 = *options_col.read() == 0 && *options_profile_idx.read() == 3;
                                            rsx! {
                                                div {
                                                    class: format!("bp-profile-item {} {} {}",
                                                        if is_deployed { "deployed" } else { "" },
                                                        if is_selected { "selected" } else { "" },
                                                        if is_focused_3 { "bp-cursor-focused" } else { "" }
                                                    ),
                                                    onclick: {
                                                        move |_| {
                                                            options_col.set(0);
                                                            options_profile_idx.set(3);
                                                            apply_profile_selection(3, backend_choice, route_choice, is_vanilla_selected);
                                                        }
                                                    },
                                                    div {
                                                        class: "bp-profile-item-left",
                                                        span { class: "bp-profile-cursor", if is_focused_3 { "▶" } else { "" } }
                                                        svg {
                                                            style: "width: 14px; height: 14px; opacity: 0.85; margin-right: 4px;",
                                                            view_box: "0 0 24 24",
                                                            fill: "none",
                                                            stroke: "currentColor",
                                                            stroke_width: "2.2",
                                                            stroke_linecap: "round",
                                                            stroke_linejoin: "round",
                                                            path { d: "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" }
                                                            path { d: "M3 3v5h5" }
                                                        }
                                                        span { class: "bp-profile-name", "{crate::core::i18n::t(&props.lang.read(), \"bp_profile_vanilla\")}" }
                                                    }
                                                    div {
                                                        class: "bp-profile-item-right",
                                                        if is_deployed {
                                                            span { class: "bp-profile-badge-active", "{crate::core::i18n::t(&props.lang.read(), \"bp_badge_active\")}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // RIGHT COLUMN: Feature Tuning & Compatibility Advisory
                                div {
                                    class: "bp-options-right",
                                    div { class: "bp-drawer-title", "{crate::core::i18n::t(&props.lang.read(), \"bp_feature_tuning_title\")}" }

                                    if *is_vanilla_selected.read() {
                                        div {
                                            class: "emu-note",
                                            style: "border-left: 3px solid #0284c7; background: rgba(2, 132, 199, 0.08);",
                                            b { "{crate::core::i18n::t(&props.lang.read(), \"advisory_vanilla_title\")}" }
                                            p { class: "advisory-intro", "{crate::core::i18n::t(&props.lang.read(), \"advisory_vanilla_desc\")}" }
                                        }
                                    } else if !active_advisories.is_empty() {
                                        for adv in &active_advisories {
                                            {
                                                let is_info = adv.severity == crate::core::install_routes::AdvisorySeverity::Info;
                                                let card_class = if is_info { "emu-note info-notice" } else { "emu-note incompatibility-warning" };
                                                let title_text = if is_info {
                                                    crate::core::i18n::t_param(&props.lang.read(), "advisory_notice_info", &adv.title)
                                                } else {
                                                    crate::core::i18n::t_param(&props.lang.read(), "advisory_warning_high", &adv.title)
                                                };
                                                let intro_text = if is_info {
                                                    crate::core::i18n::t(&props.lang.read(), "advisory_intro_info")
                                                } else {
                                                    crate::core::i18n::t(&props.lang.read(), "advisory_intro_restrictions")
                                                };
                                                rsx! {
                                                    div {
                                                        key: "{adv.title}",
                                                        class: "{card_class}",
                                                        b { "{title_text}" }
                                                        p { class: "advisory-intro", "{intro_text}" }
                                                        ul { class: "advisory-reasons",
                                                            for reason in &adv.reasons {
                                                                li { key: "{reason}", "{crate::core::i18n::translate_advisory_reason(&props.lang.read(), reason)}" }
                                                            }
                                                        }
                                                        div { class: "advisory-footer",
                                                            span { "{crate::core::i18n::translate_advisory_recommendation(&props.lang.read(), &adv.recommendation)}" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    } else {
                                        div {
                                            class: "emu-note",
                                            style: "border-left: 3px solid #10b981; background: rgba(16, 185, 129, 0.08);",
                                            b { "{crate::core::i18n::t(&props.lang.read(), \"advisory_compatible_title\")}" }
                                            p { class: "advisory-intro", "{crate::core::i18n::t(&props.lang.read(), \"advisory_compatible_desc\")}" }
                                        }
                                    }

                                    div {
                                        class: "bp-presets-box",
                                        if *is_vanilla_selected.read() {
                                            div {
                                                class: "sheet-feature-card bp-vanilla-card",
                                                div {
                                                    class: "body",
                                                    div {
                                                        class: "t",
                                                        span { "{crate::core::i18n::t(&props.lang.read(), \"bp_vanilla_no_tuning\")}" }
                                                    }
                                                }
                                            }
                                        } else {
                                            {
                                                let show_exe = game.available_exes.len() > 1;
                                            let mut tun_items: Vec<&'static str> = Vec::new();
                                            if show_exe { tun_items.push("exe"); }
                                            if show_pre_sr { tun_items.push("pre_sr"); }
                                            if show_nr_style { tun_items.push("nr_style"); }
                                            if show_mfg { tun_items.push("mfg"); }

                                            let exe_idx = tun_items.iter().position(|&x| x == "exe");
                                            let presr_idx = tun_items.iter().position(|&x| x == "pre_sr");
                                            let nr_idx = tun_items.iter().position(|&x| x == "nr_style");
                                            let mfg_idx = tun_items.iter().position(|&x| x == "mfg");

                                            rsx! {
                                                // Target Executable (for multi-executable titles like Baldur's Gate 3)
                                                if let Some(e_idx) = exe_idx {
                                                    {
                                                        let is_exe_card_focused = *options_col.read() == 1 && *options_tuning_idx.read() == e_idx;
                                                        let is_exe_stepper_focused = is_exe_card_focused && *options_sub_idx.read() == 1;
                                                        let cur_exe_display = if !game.exe_rel.is_empty() {
                                                            game.exe_rel.as_str()
                                                        } else {
                                                            game.exe_path.file_name().and_then(|n| n.to_str()).unwrap_or("")
                                                        };
                                                        let g_card = game.clone();
                                                        let g_prev = game.clone();
                                                        let g_next = game.clone();
                                                        let games_card = props.games;
                                                        let games_prev = props.games;
                                                        let games_next = props.games;
                                                        rsx! {
                                                            div {
                                                                class: format!("sheet-feature-card on {} {}",
                                                                    if is_exe_card_focused { "bp-cursor-focused" } else { "" },
                                                                    if is_exe_stepper_focused { "has-stepper-focused" } else { "" }
                                                                ),
                                                                onclick: move |_| {
                                                                    if let Some(next_opt) = cycle_game_exe(&g_card, true) {
                                                                        apply_cycled_exe(&next_opt, &g_card.dir, games_card, backend_choice, route_choice);
                                                                    }
                                                                },
                                                                div {
                                                                    class: "body",
                                                                    div {
                                                                        class: "t",
                                                                        div {
                                                                            class: "t-left",
                                                                            style: "cursor: pointer;",
                                                                            span { "{crate::core::i18n::t(&props.lang.read(), \"bp_target_executable_title\")}" }
                                                                            span { class: "tag accent", "{game.api}" }
                                                                        }
                                                                        div {
                                                                            class: "passes-ctrl",
                                                                            span { "{crate::core::i18n::t(&props.lang.read(), \"bp_target_executable_label\")}" }
                                                                            div {
                                                                                class: format!("bp-stepper {}",
                                                                                    if is_exe_stepper_focused { "bp-cursor-focused" } else { "" }
                                                                                ),
                                                                                button {
                                                                                    class: "bp-stepper-btn prev",
                                                                                    onclick: move |e| {
                                                                                        e.stop_propagation();
                                                                                        if let Some(next_opt) = cycle_game_exe(&g_prev, false) {
                                                                                            apply_cycled_exe(&next_opt, &g_prev.dir, games_prev, backend_choice, route_choice);
                                                                                        }
                                                                                    },
                                                                                    "◀"
                                                                                }
                                                                                span {
                                                                                    class: "bp-stepper-value",
                                                                                    style: "min-width: 140px; font-family: monospace; font-size: 0.85rem;",
                                                                                    "{cur_exe_display}"
                                                                                }
                                                                                button {
                                                                                    class: "bp-stepper-btn next",
                                                                                    onclick: move |e| {
                                                                                        e.stop_propagation();
                                                                                        if let Some(next_opt) = cycle_game_exe(&g_next, true) {
                                                                                            apply_cycled_exe(&next_opt, &g_next.dir, games_next, backend_choice, route_choice);
                                                                                        }
                                                                                    },
                                                                                    "▶"
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                    div { class: "d", "{crate::core::i18n::t(&props.lang.read(), \"bp_target_executable_desc\")}" }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                // Pre-SR Pass
                                                if let Some(p_idx) = presr_idx {
                                                    {
                                                        let is_presr_card_focused = *options_col.read() == 1 && *options_tuning_idx.read() == p_idx;
                                                        let is_presr_stepper_focused = is_presr_card_focused && *options_sub_idx.read() == 1;
                                                        rsx! {
                                                            div {
                                                                class: format!("sheet-feature-card {} {} {}",
                                                                    if *opti_pre_sr.read() { "on" } else { "" },
                                                                    if is_presr_card_focused { "bp-cursor-focused" } else { "" },
                                                                    if is_presr_stepper_focused { "has-stepper-focused" } else { "" }
                                                                ),
                                                                input {
                                                                    type: "checkbox",
                                                                    id: "bpChkPreSr",
                                                                    checked: *opti_pre_sr.read(),
                                                                    onchange: move |e| opti_pre_sr.set(e.checked()),
                                                                }
                                                                div {
                                                                    class: "body",
                                                                    div {
                                                                        class: "t",
                                                                        label {
                                                                            r#for: "bpChkPreSr",
                                                                            class: "t-left",
                                                                            style: "cursor: pointer;",
                                                                            span { "{crate::core::i18n::t(&props.lang.read(), \"feature_presr_title\")}" }
                                                                            span { class: "tag accent", "{crate::core::i18n::t(&props.lang.read(), \"feature_presr_tag\")}" }
                                                                        }
                                                                        div {
                                                                            class: "passes-ctrl",
                                                                            span { "{crate::core::i18n::t(&props.lang.read(), \"feature_presr_passes\")}" }
                                                                            div {
                                                                                class: format!("bp-stepper {} {}",
                                                                                    if !*opti_pre_sr.read() { "disabled" } else { "" },
                                                                                    if is_presr_stepper_focused { "bp-cursor-focused" } else { "" }
                                                                                ),
                                                                                button {
                                                                                    class: "bp-stepper-btn prev",
                                                                                    disabled: !*opti_pre_sr.read(),
                                                                                    onclick: move |e| {
                                                                                        e.stop_propagation();
                                                                                        let cur = *opti_passes.read();
                                                                                        let next = if cur <= 1 { 3 } else { cur - 1 };
                                                                                        opti_passes.set(next);
                                                                                    },
                                                                                    "◀"
                                                                                }
                                                                                span { class: "bp-stepper-value", "{opti_passes}x" }
                                                                                button {
                                                                                    class: "bp-stepper-btn next",
                                                                                    disabled: !*opti_pre_sr.read(),
                                                                                    onclick: move |e| {
                                                                                        e.stop_propagation();
                                                                                        let cur = *opti_passes.read();
                                                                                        let next = if cur >= 3 { 1 } else { cur + 1 };
                                                                                        opti_passes.set(next);
                                                                                    },
                                                                                    "▶"
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                    div { class: "d", "{crate::core::i18n::t(&props.lang.read(), \"feature_presr_desc\")}" }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                // Neural Rendering Style
                                                if let Some(n_idx) = nr_idx {
                                                    {
                                                        let is_nr_card_focused = *options_col.read() == 1 && *options_tuning_idx.read() == n_idx;
                                                        let is_nr_stepper_focused = is_nr_card_focused && *options_sub_idx.read() == 1;
                                                        let model_name = match *nr_style_preset.read() {
                                                            0 => crate::core::i18n::t(&props.lang.read(), "preview_model_a"),
                                                            1 => crate::core::i18n::t(&props.lang.read(), "preview_model_b"),
                                                            _ => crate::core::i18n::t(&props.lang.read(), "preview_model_c"),
                                                        };
                                                        rsx! {
                                                            div {
                                                                class: format!("sheet-feature-card {} {} {}",
                                                                    if *nr_style_choice.read() { "on" } else { "" },
                                                                    if is_nr_card_focused { "bp-cursor-focused" } else { "" },
                                                                    if is_nr_stepper_focused { "has-stepper-focused" } else { "" }
                                                                ),
                                                                input {
                                                                    type: "checkbox",
                                                                    id: "bpChkNrStyle",
                                                                    checked: *nr_style_choice.read(),
                                                                    onchange: move |e| nr_style_choice.set(e.checked()),
                                                                }
                                                                div {
                                                                    class: "body",
                                                                    div {
                                                                        class: "t",
                                                                        label {
                                                                            r#for: "bpChkNrStyle",
                                                                            class: "t-left",
                                                                            style: "cursor: pointer;",
                                                                            span { "{crate::core::i18n::t(&props.lang.read(), \"feature_nr_style_title\")}" }
                                                                            span { class: "tag accent", "{crate::core::i18n::t(&props.lang.read(), \"feature_nr_style_tag\")}" }
                                                                        }
                                                                        div {
                                                                            class: "passes-ctrl",
                                                                            span { "{crate::core::i18n::t(&props.lang.read(), \"feature_nr_style_label\")}" }
                                                                            div {
                                                                                class: format!("bp-stepper {} {}",
                                                                                    if !*nr_style_choice.read() { "disabled" } else { "" },
                                                                                    if is_nr_stepper_focused { "bp-cursor-focused" } else { "" }
                                                                                ),
                                                                                button {
                                                                                    class: "bp-stepper-btn prev",
                                                                                    disabled: !*nr_style_choice.read(),
                                                                                    onclick: move |e| {
                                                                                        e.stop_propagation();
                                                                                        let cur = *nr_style_preset.read();
                                                                                        nr_style_preset.set(if cur == 0 { 2 } else { cur - 1 });
                                                                                    },
                                                                                    "◀"
                                                                                }
                                                                                span { class: "bp-stepper-value", style: "min-width: 130px;", "{model_name}" }
                                                                                button {
                                                                                    class: "bp-stepper-btn next",
                                                                                    disabled: !*nr_style_choice.read(),
                                                                                    onclick: move |e| {
                                                                                        e.stop_propagation();
                                                                                        let cur = *nr_style_preset.read();
                                                                                        nr_style_preset.set((cur + 1) % 3);
                                                                                    },
                                                                                    "▶"
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                    div { class: "d", "{crate::core::i18n::t(&props.lang.read(), \"feature_nr_style_desc\")}" }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                // 4X MFG
                                                if let Some(m_idx) = mfg_idx {
                                                    {
                                                        let is_mfg_card_focused = *options_col.read() == 1 && *options_tuning_idx.read() == m_idx;
                                                        let is_mfg_stepper_focused = is_mfg_card_focused && *options_sub_idx.read() == 1;
                                                        let mfg_name = match *mfg_multiplier.read() {
                                                            1 => crate::core::i18n::t(&props.lang.read(), "bp_mfg_mult_1x"),
                                                            2 => "2x",
                                                            3 => "3x",
                                                            _ => crate::core::i18n::t(&props.lang.read(), "bp_mfg_mult_4x"),
                                                        };
                                                        rsx! {
                                                            div {
                                                                class: format!("sheet-feature-card {} {} {}",
                                                                    if *mfg_choice.read() { "on" } else { "" },
                                                                    if is_mfg_card_focused { "bp-cursor-focused" } else { "" },
                                                                    if is_mfg_stepper_focused { "has-stepper-focused" } else { "" }
                                                                ),
                                                                input {
                                                                    type: "checkbox",
                                                                    id: "bpChkMfg",
                                                                    checked: *mfg_choice.read(),
                                                                    onchange: move |e| mfg_choice.set(e.checked()),
                                                                }
                                                                div {
                                                                    class: "body",
                                                                    div {
                                                                        class: "t",
                                                                        label {
                                                                            r#for: "bpChkMfg",
                                                                            class: "t-left",
                                                                            style: "cursor: pointer;",
                                                                            span { "{crate::core::i18n::t(&props.lang.read(), \"feature_mfg_title\")}" }
                                                                            if mfg_advisory.is_some() {
                                                                                span { class: "tag advisory", "{crate::core::i18n::t(&props.lang.read(), \"advisory_tag_warn\")}" }
                                                                            } else {
                                                                                span { class: "tag warn", "{crate::core::i18n::t(&props.lang.read(), \"feature_mfg_tag\")}" }
                                                                            }
                                                                        }
                                                                        div {
                                                                            class: "passes-ctrl",
                                                                            span { "{crate::core::i18n::t(&props.lang.read(), \"sheet_mfg_multiplier\")}" }
                                                                            div {
                                                                                class: format!("bp-stepper {} {}",
                                                                                    if !*mfg_choice.read() { "disabled" } else { "" },
                                                                                    if is_mfg_stepper_focused { "bp-cursor-focused" } else { "" }
                                                                                ),
                                                                                button {
                                                                                    class: "bp-stepper-btn prev",
                                                                                    disabled: !*mfg_choice.read(),
                                                                                    onclick: move |e| {
                                                                                        e.stop_propagation();
                                                                                        let cur = *mfg_multiplier.read();
                                                                                        let next = if cur <= 1 { 4 } else { cur - 1 };
                                                                                        mfg_multiplier.set(next);
                                                                                    },
                                                                                    "◀"
                                                                                }
                                                                                span { class: "bp-stepper-value", style: "min-width: 100px;", "{mfg_name}" }
                                                                                button {
                                                                                    class: "bp-stepper-btn next",
                                                                                    disabled: !*mfg_choice.read(),
                                                                                    onclick: move |e| {
                                                                                        e.stop_propagation();
                                                                                        let cur = *mfg_multiplier.read();
                                                                                        let next = if cur >= 4 { 1 } else { cur + 1 };
                                                                                        mfg_multiplier.set(next);
                                                                                    },
                                                                                    "▶"
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                    div { class: "d", "{crate::core::i18n::t(&props.lang.read(), \"feature_mfg_desc\")}" }
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
                    }
                } else {
                    rsx! {}
                }
            }
        } else {
            main {
                class: "bp-grid-container",
                        for (store_label, store_games) in store_partitions {
                            {
                                let group_count = store_games.len();
                                rsx! {
                                    section {
                                        class: "group",
                                        key: "{store_label}",
                                        div {
                                            class: "group-head",
                                            h4 { "{store_label}" }
                                            span { class: "count", "{group_count}" }
                                        }

                                        div {
                                            class: "grid",
                                            for (orig_idx, game) in store_games {
                                                {
                                                    let idx = orig_idx;
                                                    let is_focused = idx == *focused_idx.read();
                                                    let g_name = game.name.clone();
                                                    let g_api = game.api.clone();
                                                    let is_dx12 = game.api == "DirectX 12" || game.api.contains("12");
                                                    let has_dlss = game.dlss_version.is_some();
                                                    let dlss_label = if let Some(ref v) = game.dlss_version {
                                                        crate::core::scan::short_version(v)
                                                    } else {
                                                        "No DLSS".to_string()
                                                    };
                                                    let g_opti = game.optiscaler_installed;
                                                    let has_addon = game.mfg_unlock_installed || game.has_backup;
                                                    let poster_opt = game.poster.as_ref().map(|p| crate::core::steamart::normalize_art_uri(p));
                                                    let is_game_patched = game.is_dlss5_patched();

                                                    rsx! {
                                                        article {
                                                            key: "{game.dir.display()}",
                                                            id: "bp-card-{idx}",
                                                            class: if is_focused {
                                                                if is_dx12 { "card dx12 bp-card-focused" } else { "card bp-card-focused" }
                                                            } else {
                                                                if is_dx12 { "card dx12" } else { "card" }
                                                            },
                                                            tabindex: "0",
                                                            onclick: {
                                                                let g = game.clone();
                                                                move |_| {
                                                                    focused_idx.set(idx);
                                                                    let active_idx = active_profile_idx(&g);
                                                                    options_col.set(0);
                                                                    options_sub_idx.set(0);
                                                                    options_profile_idx.set(active_idx);
                                                                    apply_profile_selection(active_idx, backend_choice, route_choice, is_vanilla_selected);
                                                                    show_options.set(true);
                                                                }
                                                            },

                                                            span { class: if is_dx12 { "badge dx12" } else { "badge" }, "{g_api}" }
                                                            div {
                                                                class: "poster",
                                                                button {
                                                                    class: format!("card-center-play bp-card-center-play {}", if is_game_patched { "modded" } else { "vanilla" }),
                                                                    title: "Launch {g_name}",
                                                                    onclick: {
                                                                        let g = game.clone();
                                                                        let d = desktop.clone();
                                                                        let lang_grid = props.lang.read().clone();
                                                                        move |e: MouseEvent| {
                                                                            e.stop_propagation();
                                                                            if !*is_launching.read() && !*is_applying.read() {
                                                                                trigger_launch(
                                                                                    g.clone(),
                                                                                    backend_choice.read().clone(),
                                                                                    route_choice.read().clone(),
                                                                                    *is_vanilla_selected.read(),
                                                                                    d.clone(),
                                                                                    is_launching,
                                                                                    launch_overlay,
                                                                                    status_banner,
                                                                                    props.games,
                                                                                    DeployPresetOptions {
                                                                                        pre_sr: *opti_pre_sr.read(),
                                                                                        passes: *opti_passes.read(),
                                                                                        mfg_unlock: *mfg_choice.read(),
                                                                                        mfg_multiplier: *mfg_multiplier.read(),
                                                                                        nr_style: *nr_style_preset.read(),
                                                                                        nr_style_enabled: *nr_style_choice.read(),
                                                                                    },
                                                                                    false,
                                                                                    lang_grid.clone(),
                                                                                    is_open,
                                                                                    show_exit_confirm,
                                                                                );
                                                                            }
                                                                        }
                                                                    },
                                                                    span { class: "{play_glyph_cls}", "{play_glyph_sym}" }
                                                                    span { " {crate::core::i18n::t(&props.lang.read(), \"bp_hud_play\")}" }
                                                                }
                                                                if let Some(ref p_url) = poster_opt {
                                                                    img { src: "{p_url}", alt: "{g_name}" }
                                                                } else {
                                                                    div { class: "placeholder", "{&g_name[0..2.min(g_name.len())]}" }
                                                                }
                                                                div {
                                                                    class: "status",
                                                                    span { class: if has_dlss { "dot-s on" } else { "dot-s" } }
                                                                    "{dlss_label}"
                                                                    span { class: if g_opti || has_addon { "dot-s on" } else { "dot-s" }, style: "margin-inline-start:8px" }
                                                                    if g_opti { "OptiScaler" } else { "add-on" }
                                                                }
                                                            }
                                                            div { class: "name", "{g_name}" }

                                                            if is_focused {
                                                                div {
                                                                    class: "bp-card-options-hint",
                                                                    span { class: "{menu_glyph_cls}", "{menu_glyph_sym}" }
                                                                    span { " {crate::core::i18n::t(&props.lang.read(), \"bp_card_presets_hint\")}" }
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
            }

            // Exit Big Picture Confirmation Modal
            if *show_exit_confirm.read() {
                div {
                    class: "bp-confirm-modal-backdrop",
                    onclick: move |_| {
                        show_exit_confirm.set(false);
                    },
                    div {
                        class: "bp-confirm-modal",
                        onclick: move |e| e.stop_propagation(),
                        h2 { class: "bp-confirm-title", "{crate::core::i18n::t(&props.lang.read(), \"bp_exit_title\")}" }
                        p { class: "bp-confirm-desc", "{crate::core::i18n::t(&props.lang.read(), \"bp_exit_desc\")}" }
                        div {
                            class: "bp-confirm-actions",
                            button {
                                class: format!("bp-confirm-btn danger {}", if *exit_confirm_idx.read() == 0 { "bp-cursor-focused" } else { "" }),
                                onclick: {
                                    let d = desktop.clone();
                                    move |_| {
                                        exit_big_picture(d.clone(), is_open, show_exit_confirm);
                                    }
                                },
                                span { "{crate::core::i18n::t(&props.lang.read(), \"bp_exit_btn_desktop\")}" }
                            }
                            button {
                                class: format!("bp-confirm-btn cancel {}", if *exit_confirm_idx.read() == 1 { "bp-cursor-focused" } else { "" }),
                                onclick: move |_| {
                                    show_exit_confirm.set(false);
                                },
                                span { "{crate::core::i18n::t(&props.lang.read(), \"bp_exit_btn_cancel\")}" }
                            }
                        }
                    }
                }
            }

            // Fullscreen Launch / Exit Blackout Transition
            if *is_launching.read() {
                {
                    let overlay_info = launch_overlay.read().clone();
                    rsx! {
                        div {
                            class: "bp-launch-blackout",
                            if let Some(ref info) = overlay_info {
                                if let Some(ref p_url) = info.poster_url {
                                    img {
                                        class: "bp-launch-poster",
                                        src: "{p_url}",
                                        alt: "{info.title}",
                                    }
                                }
                                div { class: "bp-launch-spinner" }
                                h2 {
                                    class: "bp-launch-title",
                                    "{info.title}"
                                }
                            } else {
                                div { class: "bp-launch-spinner" }
                                h2 {
                                    class: "bp-launch-title",
                                    "{active_game.as_ref().map(|g| g.name.as_str()).unwrap_or(\"Launching...\")}"
                                }
                            }
                        }
                    }
                }
            }

            // Fixed Bottom Controller HUD Bar
            div {
                class: "bp-hud-bar",
                if *show_exit_confirm.read() {
                    div { class: "bp-hud-item", span { class: "{play_glyph_cls}", "{play_glyph_sym}" }, "{crate::core::i18n::t(&props.lang.read(), \"bp_hud_select\")}" }
                    div { class: "bp-hud-item", span { class: "btn-glyph", "◄►" }, "{crate::core::i18n::t(&props.lang.read(), \"bp_hud_select\")}" }
                    div { class: "bp-hud-item", span { class: "{back_glyph_cls}", "{back_glyph_sym}" }, "{crate::core::i18n::t(&props.lang.read(), \"bp_exit_btn_cancel\")}" }
                } else if *show_options.read() {
                    {
                        let is_cur_exe = *options_col.read() == 1 && {
                            let show_exe = active_game.as_ref().map(|g| g.available_exes.len() > 1).unwrap_or(false);
                            let show_pre_sr = effective_backend == "optiscaler";
                            let show_nr_style = effective_backend == "reshade";
                            let show_mfg = active_game.as_ref().map(|g| g.bitness == 64 && g.can_inject_fg).unwrap_or(false);
                            let mut tun_items: Vec<&'static str> = Vec::new();
                            if show_exe { tun_items.push("exe"); }
                            if show_pre_sr { tun_items.push("pre_sr"); }
                            if show_nr_style { tun_items.push("nr_style"); }
                            if show_mfg { tun_items.push("mfg"); }
                            tun_items.get(*options_tuning_idx.read()) == Some(&"exe")
                        };
                        let hud_action_label = resolve_options_hud_label_with_tuning(
                            *options_col.read(),
                            *options_sub_idx.read(),
                            *options_profile_idx.read(),
                            is_cur_exe,
                            is_selection_synced,
                            &crate::core::i18n::t(&props.lang.read(), "bp_hud_play"),
                            &crate::core::i18n::t(&props.lang.read(), "bp_apply_and_play"),
                            &crate::core::i18n::t(&props.lang.read(), "bp_exit_btn_cancel"),
                            &crate::core::i18n::t(&props.lang.read(), "bp_hud_cycle_executable"),
                            &crate::core::i18n::t(&props.lang.read(), "bp_hud_cycle_value"),
                            &crate::core::i18n::t(&props.lang.read(), "bp_hud_toggle_option"),
                        );

                        let back_label = if *options_col.read() == 1 {
                            crate::core::i18n::t(&props.lang.read(), "bp_hud_back")
                        } else {
                            crate::core::i18n::t(&props.lang.read(), "bp_exit_btn_cancel")
                        };

                        rsx! {
                            div {
                                class: "bp-hud-item",
                                span { class: "{play_glyph_cls}", "{play_glyph_sym}" },
                                "{hud_action_label}"
                            }
                            {
                                let (hud_apply_cls, hud_apply_sym, hud_apply_txt) = if *is_applying.read() {
                                    ("bp-hud-item applying", patch_glyph_sym, crate::core::i18n::t(&props.lang.read(), "bp_hud_applying"))
                                } else if *just_applied.read() {
                                    ("bp-hud-item applied", "✓", crate::core::i18n::t(&props.lang.read(), "bp_hud_applied"))
                                } else {
                                    ("bp-hud-item", patch_glyph_sym, crate::core::i18n::t(&props.lang.read(), "bp_hud_apply"))
                                };
                                rsx! {
                                    div {
                                        class: "{hud_apply_cls}",
                                        span { class: "{patch_glyph_cls}", "{hud_apply_sym}" },
                                        "{hud_apply_txt}"
                                    }
                                }
                            }
                            div { class: "bp-hud-item", span { class: "{menu_glyph_cls}", "{menu_glyph_sym}" }, "{crate::core::i18n::t(&props.lang.read(), \"bp_options_presets\")}" }
                            div { class: "bp-hud-item", span { class: "btn-glyph", "◄►" }, "Switch Column" }
                            div { class: "bp-hud-item", span { class: "btn-glyph", "{bumper_l}/{bumper_r}" }, "Cycle Values" }
                            div { class: "bp-hud-item", span { class: "{back_glyph_cls}", "{back_glyph_sym}" }, "{back_label}" }
                        }
                    }
                } else {
                    div { class: "bp-hud-item", span { class: "{play_glyph_cls}", "{play_glyph_sym}" }, "{crate::core::i18n::t(&props.lang.read(), \"bp_hud_play\")}" }
                    div { class: "bp-hud-item", span { class: "{menu_glyph_cls}", "{menu_glyph_sym}" }, "{crate::core::i18n::t(&props.lang.read(), \"bp_options_presets\")}" }
                    div { class: "bp-hud-item", span { class: "{filter_glyph_cls}", "{filter_glyph_sym}" }, "{crate::core::i18n::t(&props.lang.read(), \"bp_hud_switch_store\")}" }
                    div { class: "bp-hud-item", span { class: "btn-glyph", "{bumper_l}/{bumper_r}" }, "Fast Scroll" }
                    div { class: "bp-hud-item", span { class: "{back_glyph_cls}", "{back_glyph_sym}" }, "{crate::core::i18n::t(&props.lang.read(), \"bp_btn_desktop\")}" }
                }
            }
        }
    }
}

/// Shared async routine executing deployment or restoration and refreshing the game entry cache.
async fn deploy_or_restore_payload(
    target_game: &GameEntry,
    backend_choice: &str,
    route_choice: &str,
    is_vanilla: bool,
    opts: &DeployPresetOptions,
    status_banner: &mut Signal<Option<String>>,
    games: &mut Signal<Vec<GameEntry>>,
) -> bool {
    if is_vanilla {
        let g_dir = target_game.dir.clone();
        let restore_res = tokio::task::spawn_blocking(move || {
            restore_game(&g_dir)
        }).await;

        match restore_res {
            Ok(Err(err)) => {
                eprintln!("[BigPicture] Restore error: {}", err);
                status_banner.set(Some(format!("Restore error: {}", err)));
                return false;
            }
            Err(join_err) => {
                status_banner.set(Some(format!("Internal task error: {}", join_err)));
                return false;
            }
            _ => {}
        }
    } else {
        let deploy_opts = DeployOptions {
            game_name: Some(target_game.name.clone()),
            game_dir: target_game.dir.clone(),
            exe_path: target_game.exe_path.clone(),
            api: target_game.api.clone(),
            pre_sr: opts.pre_sr,
            passes: opts.passes,
            mfg_unlock: opts.mfg_unlock,
            mfg_multiplier: opts.mfg_multiplier,
            nr_style: opts.nr_style,
            nr_style_enabled: opts.nr_style_enabled,
        };

        let b_choice = backend_choice.to_string();
        let r_choice = route_choice.to_string();

        let deploy_res = tokio::task::spawn_blocking(move || {
            let payloads = PayloadBundle::from_system().ok();
            if b_choice == "optiscaler" {
                deploy_optiscaler(&deploy_opts)
            } else if r_choice == "native" {
                deploy_native_dlss5(&deploy_opts)
            } else {
                if let Some(p) = payloads {
                    deploy_feeder_with_bundle(&deploy_opts, &p)
                } else {
                    deploy_native_dlss5(&deploy_opts)
                }
            }
        }).await;

        match deploy_res {
            Ok(Err(err)) => {
                eprintln!("[BigPicture] Deploy error: {}", err);
                status_banner.set(Some(format!("Installation failed: {}", err)));
                return false;
            }
            Err(join_err) => {
                status_banner.set(Some(format!("Internal task error: {}", join_err)));
                return false;
            }
            _ => {}
        }
    }

    // Sync updated game state after deployment/restore
    let g_dir = target_game.dir.clone();
    let refreshed = tokio::task::spawn_blocking(move || {
        crate::core::scan::scan_game_directory(&g_dir)
    }).await;

    if let Ok(Some(updated_entry)) = refreshed {
        let mut current_games = games.read().clone();
        if let Some(pos) = current_games.iter().position(|g| g.dir == updated_entry.dir) {
            current_games[pos] = updated_entry;
            games.set(current_games.clone());
            let mut current_state = crate::core::state::load_state();
            current_state.cached_games = current_games;
            let _ = crate::core::state::save_state(&current_state);
        }
    }

    true
}

/// Triggers mod deployment/restoration without launching the executable.
/// Updates game state and displays a temporary confirmation banner without shifting to black screen.
fn trigger_apply(
    target_game: GameEntry,
    backend_choice: String,
    route_choice: String,
    is_vanilla: bool,
    mut is_applying: Signal<bool>,
    mut just_applied: Signal<bool>,
    mut status_banner: Signal<Option<String>>,
    mut games: Signal<Vec<GameEntry>>,
    opts: DeployPresetOptions,
) {
    if *is_applying.read() {
        return;
    }
    is_applying.set(true);
    just_applied.set(false);

    dioxus::prelude::spawn(async move {
        let success = deploy_or_restore_payload(
            &target_game,
            &backend_choice,
            &route_choice,
            is_vanilla,
            &opts,
            &mut status_banner,
            &mut games,
        ).await;

        if success {
            just_applied.set(true);
            is_applying.set(false);
            tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
            just_applied.set(false);
        } else {
            is_applying.set(false);
        }
    });
}

#[cfg(windows)]
pub fn is_dlss_studio_foreground() -> bool {
    use windows::Win32::System::Threading::GetCurrentProcessId;
    use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId};

    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return false;
        }
        let my_pid = GetCurrentProcessId();
        let mut win_pid = 0u32;
        GetWindowThreadProcessId(fg, Some(&mut win_pid));
        win_pid == my_pid
    }
}

#[cfg(not(windows))]
pub fn is_dlss_studio_foreground() -> bool {
    false
}

/// Triggers mod deployment (if requested) and launches game executable directly,
/// minimizing DLSS Studio during play for seamless Vibepollo streaming capture.
/// Displays poster art, dynamic "Launching <Game>...", and smooth "Returning to Big Picture mode..." exit transition.
fn trigger_launch(
    target_game: GameEntry,
    backend_choice: String,
    route_choice: String,
    is_vanilla: bool,
    desktop: dioxus::desktop::DesktopContext,
    mut is_launching: Signal<bool>,
    mut launch_overlay: Signal<Option<LaunchOverlayInfo>>,
    mut status_banner: Signal<Option<String>>,
    mut _games: Signal<Vec<GameEntry>>,
    opts: DeployPresetOptions,
    should_deploy: bool,
    lang: String,
    _is_open: Signal<bool>,
    _show_exit_confirm: Signal<bool>,
) {
    if *is_launching.read() {
        return;
    }
    is_launching.set(true);

    crate::big_picture::logger::info(
        "ui",
        &format!("trigger_launch: Launching '{}' ({})", target_game.name, target_game.exe_path.display()),
    );

    let poster_url = target_game.poster.as_ref().map(|p| crate::core::steamart::normalize_art_uri(p));

    launch_overlay.set(resolve_launch_overlay_state(
        LaunchLifecycleStage::Launching,
        &target_game.name,
        poster_url.clone(),
        &lang,
    ));

    status_banner.set(Some(format!("Preparing {}...", target_game.name)));

    let exe = target_game.exe_path.clone();
    let name = target_game.name.clone();
    let lang_for_spawn = lang.clone();

    dioxus::prelude::spawn(async move {
        if should_deploy {
            crate::big_picture::logger::info("ui", &format!("Deploying profile before launch for '{}'", target_game.name));
            let success = deploy_or_restore_payload(
                &target_game,
                &backend_choice,
                &route_choice,
                is_vanilla,
                &opts,
                &mut status_banner,
                &mut _games,
            ).await;

            if !success {
                crate::big_picture::logger::error("ui", &format!("Payload deployment failed for '{}'; aborting launch", target_game.name));
                launch_overlay.set(resolve_launch_overlay_state(
                    LaunchLifecycleStage::Completed,
                    &target_game.name,
                    None,
                    &lang_for_spawn,
                ));
                is_launching.set(false);
                return;
            }
        }

        // Direct executable spawn & lifecycle supervision
        let (exit_tx, mut exit_rx) = tokio::sync::oneshot::channel::<()>();

        let session_res = launch_and_supervise(
            &name,
            &exe,
            || {},
            move || {
                let _ = exit_tx.send(());
            },
        );

        match session_res {
            Ok(_) => {
                // Wait until the game window takes foreground focus, or game process ends
                let mut exited_early = false;
                loop {
                    tokio::select! {
                        _ = &mut exit_rx => {
                            exited_early = true;
                            break;
                        }
                        _ = tokio::time::sleep(std::time::Duration::from_millis(150)) => {
                            #[cfg(windows)]
                            {
                                if !is_dlss_studio_foreground() {
                                    crate::big_picture::logger::info("ui", "Game window took foreground focus; DLSS Studio active in background");
                                    break;
                                }
                            }
                            #[cfg(not(windows))]
                            {
                                break;
                            }
                        }
                    }
                }

                // Pre-stage the return overlay immediately while the game is running in foreground!
                launch_overlay.set(resolve_launch_overlay_state(
                    LaunchLifecycleStage::RunningInForeground,
                    &name,
                    poster_url.clone(),
                    &lang_for_spawn,
                ));

                if !exited_early {
                    // Await process exit from supervisor
                    let _ = exit_rx.await;
                }

                crate::big_picture::logger::info("ui", "Game session completed. Restoring Big Picture focus...");
                desktop.set_fullscreen(true);
                crate::core::display::snap_window_to_streaming_display();
                desktop.set_focus();
                status_banner.set(None);

                // Hold smooth console transition for 700ms
                tokio::time::sleep(std::time::Duration::from_millis(700)).await;
            }
            Err(e) => {
                crate::big_picture::logger::error("ui", &format!("Game launch error: {}", e));
                status_banner.set(Some(format!("Launch error: {}", e)));
            }
        }

        launch_overlay.set(resolve_launch_overlay_state(
            LaunchLifecycleStage::Completed,
            &name,
            None,
            &lang_for_spawn,
        ));
        is_launching.set(false);
    });
}

fn chrono_time_str() -> String {
    // Light-weight 24h clock string using standard std / win32 local time
    #[cfg(windows)]
    unsafe {
        let st = windows::Win32::System::SystemInformation::GetLocalTime();
        format!("{:02}:{:02}", st.wHour, st.wMinute)
    }
    #[cfg(not(windows))]
    {
        "12:00".to_string()
    }
}
