use crate::core::scan::GameEntry;
use super::{AdvisorySeverity, RouteAdvisory};

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
