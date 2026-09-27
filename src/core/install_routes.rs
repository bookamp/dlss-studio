#![allow(dead_code)]

use crate::core::scan::GameEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallRoute {
    Native,
    OptiScaler,
    Feeder,
}

impl InstallRoute {
    pub fn as_str(&self) -> &'static str {
        match self {
            InstallRoute::Native => "native",
            InstallRoute::OptiScaler => "optiscaler",
            InstallRoute::Feeder => "feeder",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OptiReason {
    Unsupported,
    NeedsDlss,
}

impl OptiReason {
    pub fn message(&self) -> &'static str {
        match self {
            OptiReason::Unsupported => "OptiScaler DLSS-NR is offered only for 64-bit DX11/DX12/Vulkan games, not DX8/DX9, OpenGL or emulators.",
            OptiReason::NeedsDlss => "OptiScaler needs the game's original DLSS pipeline. No original DLSS DLL was found; copied/injected DLLs alone do not qualify.",
        }
    }
}

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

/// Evaluates compatibility for 4x Multi-Frame Generation and returns an advisory if not recommended.
pub fn get_mfg_advisory(game: &GameEntry, is_rtx_40: bool) -> Option<RouteAdvisory> {
    get_mfg_advisory_with_route(game, is_rtx_40, None, None)
}

/// Evaluates compatibility for 4x Multi-Frame Generation considering the currently selected backend and route.
pub fn get_mfg_advisory_with_route(
    game: &GameEntry,
    is_rtx_40: bool,
    current_backend: Option<&str>,
    current_route: Option<&str>,
) -> Option<RouteAdvisory> {
    let mut reasons = Vec::new();
    let api_lower = game.api.to_lowercase();
    let is_dx12 = api_lower.contains("12") || api_lower.contains("d3d12");
    let is_vulkan = api_lower.contains("vulkan");
    let is_dx11 = (api_lower.contains("11") || api_lower == "d3d11") && !is_dx12;

    let can_bridge_dx11 = is_dx11 && game.bitness == 64 && game.has_native_dlss();

    if is_vulkan {
        reasons.push("Vulkan Limitation: Vulkan does not support DLSS Frame Generation or Multi-Frame Generation swapchain hooks.".to_string());
    } else if current_backend == Some("reshade") && current_route == Some("feeder") {
        reasons.push("DLSS 5 Feeder Limitation: The Feeder route is an image-space Super Resolution pipeline and cannot interpolate frames.".to_string());
    } else if is_dx11 {
        if can_bridge_dx11 {
            reasons.push("DirectX 11 Interop Notice: Multi-Frame Generation operates via D3D11on12 bridge and strictly requires the in-game DLSS/upscaler setting to be enabled. If this specific executable lacks an in-game DLSS setting, Frame Generation cannot capture motion vectors and will stall.".to_string());
        } else {
            reasons.push("DirectX 11 Limitation: Game lacks native DLSS pipeline for D3D11on12 Frame Generation bridge.".to_string());
        }
    } else if !game.has_frame_generation {
        if !is_dx12 {
            reasons.push("Unsupported API: Multi-Frame Generation requires DirectX 11 or DirectX 12.".to_string());
        } else if !game.can_inject_fg {
            reasons.push("Missing Native DLSS-G: Game does not have native Frame Generation / Streamline hooks. Injected MFG will remain dormant.".to_string());
        }
    }

    if !is_rtx_40 {
        reasons.push("Hardware Requirement: 4x MFG unlock requires an RTX 40-Series GPU (Ada Lovelace architecture).".to_string());
    }

    if reasons.is_empty() {
        None
    } else {
        let is_info_only = is_dx11 && can_bridge_dx11 && is_rtx_40 && current_backend != Some("reshade");
        let severity = if is_info_only {
            AdvisorySeverity::Info
        } else {
            AdvisorySeverity::Warning
        };

        let recommendation = if is_vulkan {
            if game.available_exes.iter().any(|e| {
                let a = e.api.to_lowercase();
                a.contains("11") || a.contains("12")
            }) {
                "Switch to a DirectX 11 or DirectX 12 executable in the selector above to use Frame Generation.".to_string()
            } else {
                "Frame Generation is unavailable under Vulkan. Use DLSS 5 Feeder or OptiScaler for Super Resolution instead.".to_string()
            }
        } else if current_backend == Some("reshade") && current_route == Some("feeder") {
            if is_dx11 && can_bridge_dx11 {
                "Switch backend to OptiScaler DLSS-NR to enable D3D11on12 Frame Generation.".to_string()
            } else {
                "Switch to the Native route or OptiScaler DLSS-NR backend for Frame Generation.".to_string()
            }
        } else if is_dx11 && can_bridge_dx11 {
            "Ensure the in-game DLSS setting is enabled (e.g. Baldur's Gate 3). If this executable lacks an in-game DLSS setting (e.g. Control DX11), Frame Generation cannot generate frames.".to_string()
        } else if is_dx11 {
            "For DirectX 11 titles without native DLSS, use the DLSS 5 Feeder route for image reconstruction.".to_string()
        } else {
            "For titles without native DLSS-G, DLSS 5 Feeder provides Super Resolution and DLAA image reconstruction.".to_string()
        };

        Some(RouteAdvisory {
            title: format!("4x Multi-Frame Generation on {}", game.name),
            reasons,
            recommendation,
            severity,
        })
    }
}

/// Returns all universally available installation routes.
pub fn all_routes() -> Vec<InstallRoute> {
    vec![InstallRoute::Feeder, InstallRoute::Native, InstallRoute::OptiScaler]
}

/// Checks why OptiScaler is not eligible for a given game.
/// Mirrors `optiReason(target, api)` from original src/shared/install-routes.js:
/// 1. target.bitness !== 64 || target.emulator -> 'optiUnsupported'
/// 2. !['dxgi', 'vulkan'].includes(api) || target.apiLabel === 'DirectX 10' -> 'optiUnsupported'
/// 3. !target.hasNativeDlss -> 'optiNeedsDlss'
pub fn check_opti_reason(game: &GameEntry) -> Option<OptiReason> {
    if game.bitness != 64 {
        return Some(OptiReason::Unsupported);
    }

    let api_lower = game.api.to_lowercase();
    let is_dxgi_or_vulkan = api_lower.contains("12") 
        || api_lower.contains("11") 
        || api_lower.contains("dxgi") 
        || api_lower.contains("vulkan");

    let is_unsupported_api = api_lower.contains("10")
        || api_lower.contains("directx 8")
        || api_lower.contains("d3d8")
        || api_lower.contains("directx 9")
        || api_lower.contains("d3d9")
        || api_lower.contains("opengl");

    if !is_dxgi_or_vulkan || is_unsupported_api {
        return Some(OptiReason::Unsupported);
    }

    if game.dlss_version.is_none() && !game.has_native_upscaler() {
        return Some(OptiReason::NeedsDlss);
    }

    None
}

/// Returns the supported installation routes for a given game.
/// Mirrors `routesFor(target, api)` from original src/shared/install-routes.js:
pub fn routes_for(game: &GameEntry) -> Vec<InstallRoute> {
    if game.bitness != 32 && game.bitness != 64 {
        return Vec::new();
    }

    let api_lower = game.api.to_lowercase();

    if api_lower.contains("10") && !api_lower.contains("11") && !api_lower.contains("12") {
        return Vec::new(); // DX10 unsupported
    }

    if api_lower.contains("d3d8") || api_lower.contains("directx 8") {
        return if game.bitness == 32 { vec![InstallRoute::Feeder] } else { Vec::new() };
    }

    if api_lower.contains("d3d9") || api_lower.contains("directx 9") || api_lower.contains("opengl") || api_lower.contains("vulkan") {
        let opti_res = check_opti_reason(game);
        if opti_res.is_none() {
            return vec![InstallRoute::Feeder, InstallRoute::OptiScaler];
        } else {
            return vec![InstallRoute::Feeder];
        }
    }

    // DXGI / DirectX 11 / DirectX 12
    let mut routes = if is_native_dlss_supported(game) {
        vec![InstallRoute::Native, InstallRoute::Feeder]
    } else {
        vec![InstallRoute::Feeder]
    };

    if check_opti_reason(game).is_none() {
        routes.push(InstallRoute::OptiScaler);
    }

    routes
}

/// Computes the recommended default route for a game.
/// Mirrors `recommendedRoute(scan, target)` from original src/shared/install-routes.js:
pub fn recommended_route(game: &GameEntry) -> InstallRoute {
    let routes = routes_for(game);
    let has_native = game.has_native_dlss();
    let wanted = if has_native && routes.contains(&InstallRoute::Native) {
        InstallRoute::Native
    } else {
        InstallRoute::Feeder
    };

    if routes.contains(&wanted) {
        wanted
    } else if let Some(&first) = routes.first() {
        first
    } else {
        InstallRoute::Feeder
    }
}

/// Determines if Native DLSS (RenoDX D3D12 NGX hook) is strictly supported by the game engine.
/// Native DLSS relies exclusively on D3D12 NGX EvaluateFeature; non-DX12 engines (Vulkan, DX11) cannot use it.
pub fn is_native_dlss_supported(game: &GameEntry) -> bool {
    let api_lower = game.api.to_lowercase();
    let is_dx12 = api_lower.contains("12");
    game.bitness == 64 && is_dx12 && game.has_native_dlss()
}

/// Determines if Frame Generation (native DLSS-G, Streamline, or injected MFG) is supported for a game.
pub fn is_frame_generation_supported(game: &GameEntry) -> bool {
    let api_lower = game.api.to_lowercase();
    let is_dx12 = api_lower.contains("12") || api_lower.contains("d3d12");
    let is_vulkan = api_lower.contains("vulkan");
    let is_dx11 = (api_lower.contains("11") || api_lower == "d3d11") && !is_dx12;

    if is_vulkan {
        return false;
    }

    // Native DLSS-G games (DX12) always support FG
    if game.has_frame_generation {
        return true;
    }
    // Already installed MFG unlock
    if game.mfg_unlock_installed {
        return true;
    }
    // If DX11, supported if it has native DLSS to bridge via D3D11on12
    if is_dx11 {
        return game.bitness == 64 && game.has_native_dlss();
    }
    // Otherwise, FG injection is supported if the game meets can_inject_fg
    game.can_inject_fg
}

/// Computes the Frame Generation display status tuple: (label, is_on).
pub fn frame_generation_status(game: &GameEntry) -> (String, bool) {
    if game.has_frame_generation {
        ("Active (Native DLSS-G)".to_string(), true)
    } else if game.mfg_unlock_installed {
        ("Active (Injected 4x MFG)".to_string(), true)
    } else {
        let api_lower = game.api.to_lowercase();
        let is_dx12 = api_lower.contains("12") || api_lower.contains("d3d12");
        let is_vulkan = api_lower.contains("vulkan");
        let is_dx11 = (api_lower.contains("11") || api_lower == "d3d11") && !is_dx12;

        if is_vulkan {
            ("Unsupported (Vulkan API)".to_string(), false)
        } else if is_dx11 {
            if game.can_inject_fg || game.has_native_dlss() {
                ("Injectable (D3D11on12 DLSS MFG)".to_string(), false)
            } else {
                ("Unsupported (DirectX 11)".to_string(), false)
            }
        } else if is_dx12 {
            if game.can_inject_fg {
                ("Injectable (Streamline FG)".to_string(), false)
            } else {
                ("Unsupported (Requires Native DLSS-G)".to_string(), false)
            }
        } else {
            ("Unsupported (Requires DirectX 11/12)".to_string(), false)
        }
    }
}



