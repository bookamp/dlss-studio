#![allow(dead_code)]

pub mod anti_cheat;
pub mod mfg;

use crate::core::scan::GameEntry;

pub use anti_cheat::get_anti_cheat_advisory;
pub use mfg::{get_mfg_advisory, get_mfg_advisory_with_route};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvisorySeverity {
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteAdvisory {
    pub title: String,
    pub reasons: Vec<String>,
    pub recommendation: String,
    pub severity: AdvisorySeverity,
}

/// Evaluates compatibility for OptiScaler DLSS-NR and returns an advisory if not recommended.
pub fn get_optiscaler_advisory(game: &GameEntry) -> Option<RouteAdvisory> {
    let mut reasons = Vec::new();
    let mut severity = AdvisorySeverity::Warning;

    if game.bitness != 64 {
        reasons.push("32-bit Architecture: OptiScaler is compiled strictly as a 64-bit DLL (dxgi.dll). 32-bit executables cannot load 64-bit binaries and will fail to start.".to_string());
    }

    let api_lower = game.api.to_lowercase();
    let is_dx12 = api_lower.contains("12") || api_lower.contains("d3d12");
    let is_vulkan = api_lower.contains("vulkan");
    let is_dx11 = (api_lower.contains("11") || api_lower == "d3d11") && !is_dx12;
    let is_legacy = api_lower.contains("10")
        || api_lower.contains("directx 8")
        || api_lower.contains("d3d8")
        || api_lower.contains("directx 9")
        || api_lower.contains("d3d9")
        || api_lower.contains("opengl");

    let has_native = game.has_native_upscaler();
    let has_mod_dlss = game.dlss_version.is_some() && !game.has_native_dlss();

    if is_dx11 {
        if has_native {
            severity = AdvisorySeverity::Info;
            reasons.push(format!("DirectX 11 Interop Notice: OptiScaler operates via D3D11on12 interop. Ensure the in-game upscaler setting is enabled in {}.", game.name));
        } else {
            reasons.push(format!("Non-DirectX 12 / Missing Native Upscaler: Game has no native DLSS, FSR, or XeSS pipeline for OptiScaler to intercept. Injected proxy hooks (dxgi.dll) can cause D3D11 device creation failures or crashes under {}.", game.api));
            if has_mod_dlss {
                reasons.push("Mod-Deployed DLSS Detected: The present nvngx_dlss.dll was deployed by a mod (e.g. DLSS 5 Feeder) and cannot be intercepted natively by OptiScaler.".to_string());
            }
        }
    } else if is_legacy || (!is_dx12 && !is_vulkan && !api_lower.contains("dxgi")) {
        reasons.push(format!("Legacy / Non-DirectX API: OptiScaler DLSS-NR is engineered for DirectX 12 or Vulkan. Running under {} is not supported.", game.api));
    }

    if !is_dx11 && !has_native {
        reasons.push("Missing Native DLSS: No original nvngx_dlss.dll pipeline was found for OptiScaler to intercept.".to_string());
        if has_mod_dlss {
            reasons.push("Mod-Deployed DLSS Detected: The present nvngx_dlss.dll was deployed by a mod (e.g. DLSS 5 Feeder) and cannot be intercepted natively by OptiScaler.".to_string());
        }
    }

    if reasons.is_empty() {
        None
    } else {
        let recommendation = if is_dx11 && has_native {
            "OptiScaler will bridge DirectX 11 DLSS calls to D3D12 for DLSS 5 Neural Rendering.".to_string()
        } else {
            "DLSS 5 Feeder route provides generic frame interception and image reconstruction for this game.".to_string()
        };

        Some(RouteAdvisory {
            title: format!("OptiScaler on {}", game.name),
            reasons,
            recommendation,
            severity,
        })
    }
}

/// Evaluates compatibility for Native DLSS (RenoDX NGX hook) and returns an advisory if not recommended.
pub fn get_native_dlss_advisory(game: &GameEntry) -> Option<RouteAdvisory> {
    let mut reasons = Vec::new();
    let api_lower = game.api.to_lowercase();
    let is_dx12 = api_lower.contains("12") || api_lower.contains("d3d12");

    if !is_dx12 {
        reasons.push(format!("DirectX 12 Required: DLSS 5 Direct Neural Rendering hooks require a D3D12 graphics pipeline. This title runs under {}.", game.api));
    }
    if !game.has_native_dlss() {
        if game.dlss_version.is_some() {
            reasons.push("Mod-Deployed DLSS Detected: The present nvngx_dlss.dll was deployed by a mod (e.g. DLSS 5 Feeder) and cannot provide a native D3D12 NGX pipeline.".to_string());
        } else {
            reasons.push("Missing Native DLSS: Game does not include nvngx_dlss.dll for RenoDX to hook.".to_string());
        }
    }
    if game.bitness != 64 {
        reasons.push("32-bit Architecture: Native DLSS 5 requires a 64-bit game process.".to_string());
    }

    if reasons.is_empty() {
        None
    } else {
        Some(RouteAdvisory {
            title: format!("DLSS 5 Direct on {}", game.name),
            reasons,
            recommendation: format!("Use DLSS 5 Feeder to enable DLSS 5 Neural Rendering on {}.", game.api),
            severity: AdvisorySeverity::Warning,
        })
    }
}

/// Aggregates all active advisories for a given game, selected backend/route, and feature toggles.
pub fn collect_advisories(
    game: &GameEntry,
    backend: &str,
    route: &str,
    mfg_enabled: bool,
    is_rtx_40: bool,
) -> Vec<RouteAdvisory> {
    let mut advisories = Vec::new();

    if backend == "optiscaler" {
        if let Some(adv) = get_optiscaler_advisory(game) {
            advisories.push(adv);
        }
    } else if route == "native" {
        if let Some(adv) = get_native_dlss_advisory(game) {
            advisories.push(adv);
        }
    }

    if mfg_enabled {
        if let Some(adv) = get_mfg_advisory_with_route(game, is_rtx_40, Some(backend), Some(route)) {
            advisories.push(adv);
        }
    }

    advisories
}
