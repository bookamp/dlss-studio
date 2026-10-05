#![allow(dead_code)]

pub mod common;
pub mod feeder;
pub mod native;
pub mod optiscaler;

use crate::core::advisories::RouteAdvisory;
use crate::core::payloads::PayloadBundle;
use crate::core::scan::GameEntry;

pub use common::{DeployOptions, DeployResult};
pub use feeder::FeederRoute;
pub use native::NativeRoute;
pub use optiscaler::OptiScalerRoute;

/// Fundamental abstraction for all mod and hook deployment routes.
/// Every route is a self-contained module that encapsulates its identification,
/// compatibility and advisory evaluation, and payload deployment.
pub trait Route: Send + Sync {
    /// Unique machine identifier for the route (e.g. "optiscaler", "native", "feeder").
    fn id(&self) -> &'static str;

    /// User-facing display name.
    fn display_name(&self) -> &'static str;

    /// Associated backend family ("optiscaler" or "reshade").
    fn backend(&self) -> &'static str;

    /// Evaluates route-specific compatibility and returns an advisory if non-optimal.
    fn evaluate_advisory(&self, game: &GameEntry) -> Option<RouteAdvisory>;

    /// Executes full mod deployment using the provided options and payload bundle.
    fn deploy(&self, opts: &DeployOptions, payloads: &PayloadBundle) -> Result<DeployResult, String>;
}

/// Extensible compile-time factory and dispatcher for all deployment routes.
pub struct RouteFactory;

impl RouteFactory {
    /// Instantiates a route by its unique string identifier.
    pub fn create(id: &str) -> Option<Box<dyn Route>> {
        match id.to_lowercase().as_str() {
            "optiscaler" => Some(Box::new(OptiScalerRoute)),
            "native" => Some(Box::new(NativeRoute)),
            "feeder" => Some(Box::new(FeederRoute)),
            _ => None,
        }
    }

    /// Resolves and instantiates a route based on UI selection: (backend, subroute).
    pub fn create_from_selection(backend: &str, subroute: &str) -> Option<Box<dyn Route>> {
        if backend.eq_ignore_ascii_case("optiscaler") {
            Self::create("optiscaler")
        } else if subroute.eq_ignore_ascii_case("native") {
            Self::create("native")
        } else {
            Self::create("feeder")
        }
    }

    /// Returns instances of all registered routes.
    pub fn all() -> Vec<Box<dyn Route>> {
        vec![
            Box::new(OptiScalerRoute),
            Box::new(NativeRoute),
            Box::new(FeederRoute),
        ]
    }

    /// Unified dispatcher that executes deployment for the given backend and subroute selection.
    pub fn dispatch_deploy(
        backend: &str,
        subroute: &str,
        opts: &DeployOptions,
        payloads: &PayloadBundle,
    ) -> Result<DeployResult, String> {
        let route = Self::create_from_selection(backend, subroute)
            .ok_or_else(|| format!("Unknown route selection: backend='{}', subroute='{}'", backend, subroute))?;
        route.deploy(opts, payloads)
    }

    /// Unified dispatcher that resolves payloads from system and executes deployment.
    pub fn dispatch_deploy_from_system(
        backend: &str,
        subroute: &str,
        opts: &DeployOptions,
    ) -> Result<DeployResult, String> {
        if backend.eq_ignore_ascii_case("optiscaler") {
            optiscaler::deploy_optiscaler(opts)
        } else if subroute.eq_ignore_ascii_case("native") {
            native::deploy_native_dlss5(opts)
        } else {
            feeder::deploy_feeder(opts)
        }
    }
}

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

/// Returns all universally available installation routes.
pub fn all_routes() -> Vec<InstallRoute> {
    vec![InstallRoute::Feeder, InstallRoute::Native, InstallRoute::OptiScaler]
}

/// Checks why OptiScaler is not eligible for a given game.
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
