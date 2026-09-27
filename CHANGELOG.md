# Changelog

All notable changes to **DLSS 5 Studio** will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [2.0.1] - 2026-09-27

### Added
- **Universal RTX 40 Multi-Frame Generation (Dashdogy Core Module)**: Integrated Dashdogy's standalone `RTXMFG.dll` as a first-class core component alongside RenoDX, with dynamic GitHub asset discovery, status badges, automated configuration (`RTXMFG-Universal.json` / `RTX40MFG-Universal.json`), and dedicated Add-on page controls.
- **DirectX 11 OptiScaler D3D11on12 Interop & Streamline Stack**: Robust first-class DirectX 11 OptiScaler support bridging DX11 DLSS calls through `D3D11on12` to modern DLSS 5 Neural Rendering, deploying Streamline 2.14.1 into `OptiScaler/streamline/` to permanently fix `DXGI_ERROR_DEVICE_REMOVED` crashes.
- **DirectX 11 Interop Notice**: Non-blocking blue configuration notice (`AdvisorySeverity::Info`) alerting users that DirectX 11 Multi-Frame Generation operates via D3D11on12 and requires an in-game DLSS setting.
- **Dynamic Route Advisories Engine**: Implemented `get_mfg_advisory_with_route` evaluating active route and backend limitations across Vulkan, Feeder, and DirectX 11 with zero user lockouts.
- **Cumulative Added History Rollback Hardening**: Introduced `added_history.json` immune to 5-manifest rollback pruning, guaranteeing 100% clean rollback without orphan DLLs and preserving unmodded vanilla backups through successive route hot-swaps.

### Fixed
- **Scanner Native DLSS-G Isolation**: Verified rollback journal history to prevent mod-deployed `nvngx_dlssg.dll` from triggering false-positive native DLSS-G detection.
- **Frame Generation Scanner Heuristic**: Removed `sl.dlss.dll` from the Frame Generation scanner checks and gated Vulkan titles from reporting false positive Frame Generation support (`Unsupported (Vulkan API)`).
- **Vulkan Layer Test Isolation**: Sandboxed test environments for `vulkan_layer.rs` so automated tests never pollute the host system registry.
- **Multilingual Copy Refresh**: Updated `feature_mfg_desc` and notes across all 14 supported languages to reference the Universal RTX 40 MFG module.

---

## [2.0.0] - 2026-09-25

### Added
- **10-Foot Big Picture Mode**: Full-screen console interface designed for TVs and handheld devices with dynamic ambient hero backdrops, smooth card transitions, and quick store filtering.
- **Native Gamepad Navigation**: Pure Rust XInput polling loop supporting Xbox, PlayStation, and Moonlight virtual controllers with dynamic glyph hints, store switching (`LB`/`RB`), and modal navigation.
- **Couch Preset Tuning & Game Inspector**: Dedicated inline inspector for adjusting Neural Rendering styles, cycling executables (`bg3.exe` vs `bg3_dx11.exe`), toggling Pre-SR multi-pass (`1x`–`3x`), and unlocking 4x Multi-Frame Generation directly with a controller.
- **Vibepollo / Apollo / Sunshine Streaming Integration**: Automatic detection of local Apollo and Sunshine streaming hosts. A single Settings toggle registers DLSS Studio Big Picture mode (`--big-picture`) in `apps.json` with embedded cover art for instant 1-click streaming from Moonlight clients.
- **Desktop Window Z-Order & Mouse Demotion**: Zero-flicker return from Big Picture mode with automatic `HWND_NOTOPMOST` window demotion and `ReleaseCapture()` to guarantee host desktop applications remain fully operable.
- **Unified Multilingual Localization**: Complete 14-language support across Big Picture mode and notification toasts.

---

## [1.0.6] - 2026-09-18

### Added
- **Route Advisory & Force Override Engine**: Replaced hard-blocking route restrictions with an interactive advisory model. All backends (`ReShade`, `OptiScaler`) and routes (`DLSS 5 Feeder`, `Native DLSS`) remain selectable for every title. Incompatible configurations surface a prominent crimson advisory alert card (`🚨 High Incompatibility Warning`) explaining technical restrictions, paired with a glowing crimson `Deploy Anyway (Force Override) ⚠️` action for advanced users.
- **Automatic Deployment State Reflection**: Opening the game detail sheet now inspects disk state and synchronizes all UI controls (`backend_choice`, `route_choice`, `opti_pre_sr`, `opti_passes`, `mfg_choice`, and `nr_style`) to match active deployments instead of defaulting to first options.
- **Intelligent Auto-Selection for Vanilla Titles**: When opening an unmodded game, the UI automatically evaluates the game's architecture and graphics API via `recommended_route()`, defaulting to **DLSS 5 Feeder** for DirectX 11, Vulkan, OpenGL, DirectX 9, or non-DLSS games, and **Native DLSS** for 64-bit DirectX 12 DLSS games, preventing false warning alerts on initial view.

### Changed
- **Executable Switching Route Adaptation**: Switching between binaries in the game detail sheet automatically updates the route to the optimal path for the chosen binary if the game is unpatched/unmodded. If an active deployment exists, the existing configuration is strictly preserved.

---

## [1.0.5] - 2026-09-17
 
### Fixed
- **Dedicated Interactive Uninstaller & Clean Directory Purge**: Added dedicated `UninstallApp` UI with confirmation screen, live progress bar, and completion screen. Implemented temp trampoline worker pattern in `%TEMP%` to cleanly delete the entire installation folder without Windows file locks. Checkbox to clear `%APPDATA%\dlss-5-studio` defaults to checked (game backups in game folders remain untouched).
- **ReShade Framework Suite & Feeder Verification**: Bundled `DrawText.fxh`, `FontAtlas.png`, and the slim ReShade framework suite into `feeder-shaders\` so `Verify-DLSS5Feeder.ps1` checks out with 0 errors and 0 warnings.
- **WebView2 User Data Directory**: Routed setup and uninstaller WebView2 data directories to `%TEMP%`, completely preventing `.WebView2` folders from cluttering installation and release folders.
- **Configuration & Setup Utility Filtering**: Expanded `is_installer_or_helper` to filter non-game executables (`*config.exe`, `*settings.exe`, `*setup.exe`, `*activation*.exe`, `*autorun*.exe`, `*registration*.exe`, `*support*.exe`) from game directories and executable selection dropdowns (e.g., `MassEffect2Config.exe`).
- **DirectX 9 vs DirectX 10 UE3 Detection**: Fixed false positive DirectX 10 / DirectX 11 detection on legacy Unreal Engine 3 games (such as *Mass Effect 2* `ME2Game.exe`) by prioritizing active Direct3D 9 imports and PE markers over dormant D3D10 engine markers.
- **Relic Modular Rendering Recognition**: Supported modular render libraries (such as *Warhammer 40,000: Dawn of War Definitive Edition*'s `spDx9.dll`) while preventing auxiliary video player DXGI helpers from triggering false positive DirectX 11 detection.
- **"Undetected" Fallback Labeling**: Replaced ambiguous `"DirectX 11"` fallback labeling with `"Undetected"` for executables lacking any 3D graphics imports, PE markers, or graphics sibling modules.

### Changed
- **Candidate Scoring Algorithm**: Prioritized executables with verified graphics APIs (+5,000 pts) and substantial PE code size (> 5 MB, +4,000 pts) while penalizing small launcher stubs (< 1 MB without graphics calls, -5,000 pts).

---

## [1.0.2] - 2026-09-16

### Added
- **Legacy Pre-DirectX 10 (DirectX 8 & 9) dgVoodoo 2 Interop**: Automated translation for older games to modern D3D11 swapchains for the DLSS 5 Feeder pipeline.
- **32-Bit Large Address Aware (LAA) Inspection & Toggle**: Safe LAA inspection and toggling for 32-bit executables, unlocking up to 4 GB address space.
- **Steam CDN Artwork Resolution**: Automatic artwork resolution and banner downloads for manually added games and custom folders, with smart title inference for nested subfolders.

---

## [1.0.1] - 2026-09-14

### Fixed
- **DLSS 5 Feeder Neural Rendering**: Restored `NeuralUplift=1` in `ReShade.ini` for the DLSS 5 Feeder route. Resolves an issue where Neural Reconstruction (Feature 18) was initialized as disabled at startup, enabling seamless DLSS-NR execution across non-DX12 titles (e.g. DirectX 11 executables like `bg3_dx11.exe`).
- **Setup Defaults**: Defaulted installation path to `C:\DLSS 5 Studio` and data directory to `C:\DLSS 5 Studio\data` for friction-free un-elevated installs on standard Windows user accounts, with dynamic warning banners when protected system directories (`Program Files`) are selected.

### Changed
- **OptiScaler Attribution**: Updated README documentation and repository links to accurately credit `wilsjo2/OptiScaler-DLSSNR-PreSR-Multipass` for the Pre-SR Multipass and DLSS-NR runtime implementation.
- **Publish Scripting**: Added `-NoTag` switch to release automation tooling for seamless non-release documentation synchronization.

---

## [1.0.0] - 2026-09-13

### Added
- **Pure Rust Native Architecture**: Ground-up lightweight implementation using Dioxus v0.6 and direct Win32 APIs (~20 MB idle RAM, <200ms cold startup, 5.9 MB standalone binary).
- **4x Multi-Frame Generation Unlock**: Ada Lovelace frame generation multiplier unlocking 3x and 4x multipliers on GeForce RTX 40-Series GPUs via RenoDX Streamline hook companion bridge.
- **OptiScaler DLSS-NR & Pre-SR Multipass**: Integrated multi-pass neural reconstruction (1x/2x/3x passes) and universal RTXMFG proxy injection (`version.dll`).
- **Flexible Rendering Backends**:
  - ReShade Native DLSS (RenoDX) for DirectX 12 games.
  - DLSS 5 Feeder route for non-DLSS or non-DX12 titles (DirectX 11, Vulkan, OpenGL).
  - OptiScaler DLSS-NR for direct neural reconstruction.
- **Universal Multi-Store Game Scanner**: Automatic library scanning across Steam, Xbox Game Pass / Windows Store, Epic Games Store, and GOG Galaxy.
- **Atomic Journaling & Hot-Swapping**: One-click cross-route swapping with vanilla backup continuity (`_DLSS5_Backup/originals/`) and untracked mod cleaning.
- **Running Game Guard & Anti-Cheat Protection**: Win32 process inspection preventing file operations while games are running, with EasyAntiCheat, BattlEye, and Vanguard detection.
- **Complete Localization**: Full dynamic translation support across 14 languages with RTL support for Arabic.
- **Standalone Setup Installer & Portable Binary**: Dedicated WiX v4 installer and portable executable releases.
