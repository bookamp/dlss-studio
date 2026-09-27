#![allow(dead_code)]

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;
use serde::Deserialize;

static RUNNING: AtomicBool = AtomicBool::new(false);

#[doc(hidden)]
pub const FRAME_MAGIC: u32 = 0x3146_4c44;       // 'FLD1'
const INPUT_MAGIC: u32 = 0x3149_4c44;       // 'ILD1'
const HELLO_REPLY_MAGIC: u32 = 0x3148_4c44; // 'DLH1'
const COMMAND_MAGIC: u32 = 0x3143_4c44;     // 'DLC1'
const STATE_JSON_MAGIC: u32 = 0x3153_4c44;  // 'DLS1'

pub const PANEL_WIDTH: u32 = 534;
pub const PANEL_HEIGHT: u32 = 720;
pub const SLIDER_LABEL_WIDTH: f32 = 152.0;
pub const SLIDER_NUM_WIDTH: f32 = 54.0;
pub const SLIDER_GAP: f32 = 10.0;

// Windows Named Pipe API
extern "system" {
    fn CreateNamedPipeW(
        lpName: *const u16,
        dwOpenMode: u32,
        dwPipeMode: u32,
        nMaxInstances: u32,
        nOutBufferSize: u32,
        nInBufferSize: u32,
        nDefaultTimeOut: u32,
        lpSecurityAttributes: *const std::ffi::c_void,
    ) -> isize;
    fn ConnectNamedPipe(hNamedPipe: isize, lpOverlapped: *mut std::ffi::c_void) -> i32;
    fn DisconnectNamedPipe(hNamedPipe: isize) -> i32;
    fn CloseHandle(hObject: isize) -> i32;
    fn ReadFile(
        hFile: isize,
        lpBuffer: *mut u8,
        nNumberOfBytesToRead: u32,
        lpNumberOfBytesRead: *mut u32,
        lpOverlapped: *mut std::ffi::c_void,
    ) -> i32;
    fn WriteFile(
        hFile: isize,
        lpBuffer: *const u8,
        nNumberOfBytesToWrite: u32,
        lpNumberOfBytesWritten: *mut u32,
        lpOverlapped: *mut std::ffi::c_void,
    ) -> i32;
    fn GetNamedPipeClientProcessId(Pipe: isize, ClientProcessId: *mut u32) -> i32;
    fn OpenProcess(dwDesiredAccess: u32, bInheritHandle: i32, dwProcessId: u32) -> isize;
    fn QueryFullProcessImageNameW(hProcess: isize, dwFlags: u32, lpExeName: *mut u16, lpdwSize: *mut u32) -> i32;
}

// Windows GDI+ Flat API for anti-aliased Segoe UI rendering
#[repr(C)]
struct GdiplusStartupInput {
    gdiplus_version: u32,
    debug_event_callback: usize,
    suppress_background_thread: i32,
    suppress_external_codecs: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PointF {
    x: f32,
    y: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RectF {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct RectI {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[repr(C)]
struct BitmapData {
    width: u32,
    height: u32,
    stride: i32,
    pixel_format: i32,
    scan0: *mut u8,
    reserved: usize,
}

#[link(name = "gdiplus")]
extern "system" {
    fn GdiplusStartup(token: *mut usize, input: *const GdiplusStartupInput, output: *mut std::ffi::c_void) -> i32;
    fn GdiplusShutdown(token: usize);
    fn GdipCreateBitmapFromScan0(width: i32, height: i32, stride: i32, format: i32, scan0: *mut u8, bitmap: *mut usize) -> i32;
    fn GdipDisposeImage(image: usize) -> i32;
    fn GdipGetImageGraphicsContext(image: usize, graphics: *mut usize) -> i32;
    fn GdipDeleteGraphics(graphics: usize) -> i32;
    fn GdipSetSmoothingMode(graphics: usize, mode: i32) -> i32;
    fn GdipSetTextRenderingHint(graphics: usize, hint: i32) -> i32;
    fn GdipSetPixelOffsetMode(graphics: usize, mode: i32) -> i32;
    fn GdipSetInterpolationMode(graphics: usize, mode: i32) -> i32;
    fn GdipGraphicsClear(graphics: usize, color: u32) -> i32;
    fn GdipCreateSolidFill(color: u32, brush: *mut usize) -> i32;
    fn GdipCreateLineBrush(point1: *const PointF, point2: *const PointF, color1: u32, color2: u32, wrap_mode: i32, line_gradient: *mut usize) -> i32;
    fn GdipDeleteBrush(brush: usize) -> i32;
    fn GdipCreatePen1(color: u32, width: f32, unit: i32, pen: *mut usize) -> i32;
    fn GdipDeletePen(pen: usize) -> i32;
    fn GdipFillRectangle(graphics: usize, brush: usize, x: f32, y: f32, width: f32, height: f32) -> i32;
    fn GdipDrawRectangle(graphics: usize, pen: usize, x: f32, y: f32, width: f32, height: f32) -> i32;
    fn GdipFillEllipse(graphics: usize, brush: usize, x: f32, y: f32, width: f32, height: f32) -> i32;
    fn GdipDrawLine(graphics: usize, pen: usize, x1: f32, y1: f32, x2: f32, y2: f32) -> i32;
    fn GdipCreateFontFamilyFromName(name: *const u16, collection: usize, family: *mut usize) -> i32;
    fn GdipDeleteFontFamily(family: usize) -> i32;
    fn GdipCreateFont(family: usize, em_size: f32, style: i32, unit: i32, font: *mut usize) -> i32;
    fn GdipDeleteFont(font: usize) -> i32;
    fn GdipCreateStringFormat(format_flags: i32, language: u16, format: *mut usize) -> i32;
    fn GdipSetStringFormatFlags(format: usize, flags: i32) -> i32;
    fn GdipSetStringFormatAlign(format: usize, align: i32) -> i32;
    fn GdipSetStringFormatLineAlign(format: usize, line_align: i32) -> i32;
    fn GdipDeleteStringFormat(format: usize) -> i32;
    fn GdipDrawString(graphics: usize, string: *const u16, length: i32, font: usize, layout_rect: *const RectF, format: usize, brush: usize) -> i32;
    fn GdipCreatePath(brush_mode: i32, path: *mut usize) -> i32;
    fn GdipDeletePath(path: usize) -> i32;
    fn GdipAddPathArc(path: usize, x: f32, y: f32, width: f32, height: f32, start_angle: f32, sweep_angle: f32) -> i32;
    fn GdipAddPathLine(path: usize, x1: f32, y1: f32, x2: f32, y2: f32) -> i32;
    fn GdipClosePathFigure(path: usize) -> i32;
    fn GdipFillPath(graphics: usize, brush: usize, path: usize) -> i32;
    fn GdipDrawPath(graphics: usize, pen: usize, path: usize) -> i32;
    fn GdipSetClipRect(graphics: usize, x: f32, y: f32, width: f32, height: f32, combine_mode: i32) -> i32;
    fn GdipResetClip(graphics: usize) -> i32;
    fn GdipBitmapLockBits(bitmap: usize, rect: *const RectI, flags: u32, format: i32, data: *mut BitmapData) -> i32;
    fn GdipBitmapUnlockBits(bitmap: usize, data: *mut BitmapData) -> i32;
}

const SMOOTHING_MODE_ANTI_ALIAS: i32 = 4;
const TEXT_RENDERING_HINT_ANTI_ALIAS_GRID_FIT: i32 = 3;
const TEXT_RENDERING_HINT_ANTI_ALIAS: i32 = 4;
const PIXEL_OFFSET_MODE_HALF: i32 = 4;
const INTERPOLATION_MODE_HIGH_QUALITY_BICUBIC: i32 = 7;
const STRING_FORMAT_FLAGS_NO_WRAP: i32 = 0x0000_1000;
const STRING_FORMAT_FLAGS_MEASURE_TRAILING_SPACES: i32 = 0x0000_0800;
const PIXEL_FORMAT_32BPP_PARGB: i32 = 0x000E_200B;
const FONT_STYLE_REGULAR: i32 = 0;
const FONT_STYLE_BOLD: i32 = 1;
const UNIT_PIXEL: i32 = 2;
const STRING_ALIGN_NEAR: i32 = 0;
const STRING_ALIGN_CENTER: i32 = 1;
const STRING_ALIGN_FAR: i32 = 2;
const COMBINE_MODE_REPLACE: i32 = 0;

/// Typed representations of in-game telemetry JSON from ReShade add-on
#[derive(Debug, Deserialize, Clone)]
pub struct AddonStatusJson {
    pub epoch: u32,
    #[serde(default)]
    pub effects: bool,
    #[serde(default)]
    pub tools: Vec<AddonToolJson>,
    #[serde(default, rename = "nrAvailable")]
    pub nr_available: bool,
    #[serde(default, rename = "nrEnabled")]
    pub nr_enabled: bool,
    #[serde(default, rename = "nrReason")]
    pub nr_reason: String,
    #[serde(default, rename = "nrTools")]
    pub nr_tools: Vec<AddonToolJson>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AddonToolJson {
    pub id: u32,
    pub kind: u32,
    pub name: String,
    #[serde(default)]
    pub effect: String,
    #[serde(default)]
    pub min: f32,
    #[serde(default)]
    pub max: f32,
    #[serde(default)]
    pub step: f32,
    #[serde(default)]
    pub value: f32,
    #[serde(default)]
    pub available: bool,
    #[serde(default)]
    pub options: Vec<String>,
}

/// 24-byte command packet dispatched to the in-game addon
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayCommandPacket {
    pub magic: u32,        // 0x31434C44 ('DLC1')
    pub version: u32,      // 1
    pub epoch: u32,        // from addon status JSON
    pub control_id: u32,   // 101-115, or 0, or 200
    pub control_kind: u32, // 0 = float, 1 = bool, 4 = combo, 3 = toggle
    pub value: f32,
}

#[derive(Clone, Debug)]
pub struct OverlayUiState {
    pub mode: u32, // 0 = DLSS controls, 1 = Live tools
    pub dlss_on: bool,
    pub structure_intensity: f32,
    pub tone_intensity: f32,
    pub character_mask: bool,
    pub char_structure: f32,
    pub nr_style: usize,
    pub more_scroll: f32,
    pub overall_intensity: f32,
    pub local_tone: f32,
    pub diffuse_white: f32,
    pub motion_x: f32,
    pub motion_y: f32,
    pub nr_ui_correction: bool,
    pub enable_upscaling: bool,
    pub nr_preset: usize,
    pub depth_convention: usize,
    pub reshade_fx_enabled: bool,
    pub dlss_neural_rendering: bool,
    pub live_scroll: f32,
    pub active_slider: Option<String>,
    pub mouse_down: bool,
    pub mouse_x: i32,
    pub mouse_y: i32,
    pub accent_color: u32,
    pub soft_color: u32,
    pub back_color: u32,
    pub bright_color: u32,
    pub current_epoch: u32,
    pub last_enabled_epoch: u32,
    pub nr_available: bool,
    pub nr_enabled: bool,
    pub available_tools: HashSet<u32>,
    pub pending_command: Option<OverlayCommandPacket>,
    pub pending_commands: Vec<OverlayCommandPacket>,
    pub fps: f32,
    pub frametime_ms: f32,
    pub has_mfg: bool,
    pub mfg_enabled: bool,
    pub mfg_multiplier: u32,
    pub has_presr: bool,
    pub presr_enabled: bool,
    pub presr_passes: u32,
    pub active_game_dir: Option<PathBuf>,
}

impl Default for OverlayUiState {
    fn default() -> Self {
        Self {
            mode: 0,
            dlss_on: true,
            structure_intensity: 1.0,
            tone_intensity: 1.0,
            character_mask: true,
            char_structure: 1.0,
            nr_style: 0, // 0 = Default, 1 = Natural, 2 = Cinematic
            more_scroll: 0.0,
            overall_intensity: 1.0,
            local_tone: 1.0,
            diffuse_white: 203.0,
            motion_x: 1.0,
            motion_y: 1.0,
            nr_ui_correction: true,
            enable_upscaling: false,
            nr_preset: 0,
            depth_convention: 0,
            reshade_fx_enabled: true,
            dlss_neural_rendering: true,
            live_scroll: 0.0,
            active_slider: None,
            mouse_down: false,
            mouse_x: -1,
            mouse_y: -1,
            accent_color: 0xFF8B_C400, // Emerald Accent (#8bc400)
            soft_color: 0x258B_C400,   // Soft accent (#8bc40025)
            back_color: 0xFF11_1A10,   // Deep tint for gradient (#111a10)
            bright_color: 0xFFC2_EC66, // Lifted toward white (#c2ec66)
            current_epoch: 1,
            last_enabled_epoch: 0,
            nr_available: true,
            nr_enabled: true,
            available_tools: HashSet::new(),
            pending_command: None,
            pending_commands: Vec::new(),
            fps: 0.0,
            frametime_ms: 0.0,
            has_mfg: false,
            mfg_enabled: true,
            mfg_multiplier: 4,
            has_presr: false,
            presr_enabled: true,
            presr_passes: 2,
            active_game_dir: None,
        }
    }
}

/// Inspects a game directory to check if MFG Unlock or OptiScaler Pre-SR are installed and extracts initial parameters
pub fn detect_game_mods(game_dir: &std::path::Path) -> (bool, bool, u32, bool, bool, u32) {
    let mut has_mfg = false;
    let mut mfg_enabled = true;
    let mut mfg_mult = 4;

    let mut has_presr = false;
    let mut presr_enabled = false;
    let mut presr_passes = 1;

    // 1. Check MFG
    let mfg_addon = game_dir.join("renodx-mfgunlock.addon64");
    let reshade_ini = game_dir.join("ReShade.ini");
    if mfg_addon.exists() || reshade_ini.exists() {
        if let Ok(content) = fs::read_to_string(&reshade_ini) {
            if content.contains("[RenoDX.MFGUnlock]") {
                has_mfg = true;
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("Enabled=") {
                        mfg_enabled = trimmed.ends_with("1");
                    } else if trimmed.starts_with("ForceMultiplier=") {
                        if let Some(val) = trimmed.split('=').nth(1) {
                            match val.trim() {
                                "2" => mfg_mult = 2,
                                "3" => mfg_mult = 3,
                                "4" => mfg_mult = 4,
                                _ => mfg_mult = 1,
                            }
                        }
                    }
                }
            } else if mfg_addon.exists() {
                has_mfg = true;
            }
        } else if mfg_addon.exists() {
            has_mfg = true;
        }
    }

    // 2. Check OptiScaler Pre-SR (DLSS-NR)
    let opti_ini = game_dir.join("OptiScaler.ini");
    if opti_ini.exists() {
        if let Ok(content) = fs::read_to_string(&opti_ini) {
            has_presr = true;
            let mut in_dlssnr = false;
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.eq_ignore_ascii_case("[DlssNr]") {
                    in_dlssnr = true;
                    continue;
                } else if in_dlssnr && trimmed.starts_with('[') && trimmed.ends_with(']') {
                    in_dlssnr = false;
                }
                if in_dlssnr {
                    if trimmed.starts_with("Enabled=") {
                        let val = trimmed.strip_prefix("Enabled=").unwrap().trim().to_lowercase();
                        presr_enabled = val == "true" || val == "1";
                    } else if trimmed.starts_with("RunBeforeSR=") && !presr_enabled {
                        let val = trimmed.strip_prefix("RunBeforeSR=").unwrap().trim().to_lowercase();
                        presr_enabled = val == "true" || val == "1";
                    } else if trimmed.starts_with("Passes=") {
                        if let Some(val) = trimmed.split('=').nth(1) {
                            presr_passes = val.trim().parse::<u32>().unwrap_or(1).clamp(1, 3);
                        }
                    }
                }
            }
        }
    }

    (has_mfg, mfg_enabled, mfg_mult, has_presr, presr_enabled, presr_passes)
}

// Helpers for GDI+ rendering matching authentic CSS metrics
unsafe fn draw_rounded_rect(
    graphics: usize,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    fill_color: Option<u32>,
    stroke_color: Option<u32>,
    stroke_width: f32,
) {
    let mut path: usize = 0;
    if GdipCreatePath(0, &mut path) != 0 {
        return;
    }

    let d = radius * 2.0;
    GdipAddPathArc(path, x, y, d, d, 180.0, 90.0);
    GdipAddPathArc(path, x + w - d, y, d, d, 270.0, 90.0);
    GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0, 90.0);
    GdipAddPathArc(path, x, y + h - d, d, d, 90.0, 90.0);
    GdipClosePathFigure(path);

    if let Some(fc) = fill_color {
        let mut brush: usize = 0;
        if GdipCreateSolidFill(fc, &mut brush) == 0 {
            GdipFillPath(graphics, brush, path);
            GdipDeleteBrush(brush);
        }
    }

    if let Some(sc) = stroke_color {
        let mut pen: usize = 0;
        if GdipCreatePen1(sc, stroke_width, UNIT_PIXEL, &mut pen) == 0 {
            GdipDrawPath(graphics, pen, path);
            GdipDeletePen(pen);
        }
    }

    GdipDeletePath(path);
}

/// Draws rounded rectangle with authentic CSS linear/radial-style gradient
unsafe fn draw_gradient_rounded_rect(
    graphics: usize,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    radius: f32,
    color1: u32,
    color2: u32,
    stroke_color: Option<u32>,
    stroke_width: f32,
) {
    let mut path: usize = 0;
    if GdipCreatePath(0, &mut path) != 0 {
        return;
    }

    let d = radius * 2.0;
    GdipAddPathArc(path, x, y, d, d, 180.0, 90.0);
    GdipAddPathArc(path, x + w - d, y, d, d, 270.0, 90.0);
    GdipAddPathArc(path, x + w - d, y + h - d, d, d, 0.0, 90.0);
    GdipAddPathArc(path, x, y + h - d, d, d, 90.0, 90.0);
    GdipClosePathFigure(path);

    let pt1 = PointF { x, y };
    let pt2 = PointF { x: x + w * 0.4, y: y + h };
    let mut brush: usize = 0;
    if GdipCreateLineBrush(&pt1, &pt2, color1, color2, 0, &mut brush) == 0 {
        GdipFillPath(graphics, brush, path);
        GdipDeleteBrush(brush);
    } else {
        // Fallback to solid color1
        let mut fallback: usize = 0;
        if GdipCreateSolidFill(color1, &mut fallback) == 0 {
            GdipFillPath(graphics, fallback, path);
            GdipDeleteBrush(fallback);
        }
    }

    if let Some(sc) = stroke_color {
        let mut pen: usize = 0;
        if GdipCreatePen1(sc, stroke_width, UNIT_PIXEL, &mut pen) == 0 {
            GdipDrawPath(graphics, pen, path);
            GdipDeletePen(pen);
        }
    }

    GdipDeletePath(path);
}

unsafe fn draw_text(
    graphics: usize,
    text: &str,
    font: usize,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color: u32,
    align_h: i32,
    align_v: i32,
) {
    let wide: Vec<u16> = text.encode_utf16().collect();
    let rect = RectF { x, y, width: w, height: h };
    let mut format: usize = 0;
    if GdipCreateStringFormat(0, 0, &mut format) == 0 {
        GdipSetStringFormatFlags(format, STRING_FORMAT_FLAGS_NO_WRAP | STRING_FORMAT_FLAGS_MEASURE_TRAILING_SPACES);
        GdipSetStringFormatAlign(format, align_h);
        GdipSetStringFormatLineAlign(format, align_v);
        let mut brush: usize = 0;
        if GdipCreateSolidFill(color, &mut brush) == 0 {
            GdipDrawString(graphics, wide.as_ptr(), wide.len() as i32, font, &rect, format, brush);
            GdipDeleteBrush(brush);
        }
        GdipDeleteStringFormat(format);
    }
}

unsafe fn draw_checkbox(
    graphics: usize,
    label: &str,
    checked: bool,
    x: f32,
    y: f32,
    w: f32,
    font_bold: usize,
    font_small: usize,
    accent: u32,
    subtitle: Option<&str>,
) {
    let box_sz = 16.0;
    let box_y = y + 2.0;

    if checked {
        draw_rounded_rect(graphics, x, box_y, box_sz, box_sz, 4.0, Some(accent), None, 0.0);
        // Sharp black checkmark matching web checkbox
        let mut pen: usize = 0;
        if GdipCreatePen1(0xFF000000, 2.2, UNIT_PIXEL, &mut pen) == 0 {
            GdipDrawLine(graphics, pen, x + 3.5, box_y + 8.0, x + 6.5, box_y + 11.5);
            GdipDrawLine(graphics, pen, x + 6.5, box_y + 11.5, x + 12.5, box_y + 4.5);
            GdipDeletePen(pen);
        }
    } else {
        // High-contrast, clearly visible unchecked box with elevated slate fill and luminous silver-slate border
        draw_rounded_rect(graphics, x, box_y, box_sz, box_sz, 4.0, Some(0xFF1E2632), Some(0xFF94A3B8), 1.5);
    }

    let text_x = x + box_sz + 10.0;
    let text_w = w - (box_sz + 10.0);
    draw_text(graphics, label, font_bold, text_x, y, text_w, 20.0, 0xFFFFFFFF, STRING_ALIGN_NEAR, STRING_ALIGN_CENTER);

    if let Some(sub) = subtitle {
        draw_text(graphics, sub, font_small, text_x, y, text_w - 4.0, 20.0, 0xFFCBD5E1, STRING_ALIGN_FAR, STRING_ALIGN_CENTER);
    }
}

unsafe fn draw_slider(
    graphics: usize,
    label: &str,
    value: f32,
    min: f32,
    max: f32,
    x: f32,
    y: f32,
    w: f32,
    font_body: usize,
    font_mono: usize,
    accent: u32,
) {
    // 1. Label on left matching CSS .ol-slider span
    draw_text(graphics, label, font_body, x, y, SLIDER_LABEL_WIDTH, 22.0, 0xFFF8FAFC, STRING_ALIGN_NEAR, STRING_ALIGN_CENTER);

    // 2. Track matching CSS .ol-slider input
    let track_x = x + SLIDER_LABEL_WIDTH;
    let num_w = SLIDER_NUM_WIDTH;
    let track_w = w - SLIDER_LABEL_WIDTH - num_w - SLIDER_GAP;
    let rail_h = 5.0;
    let rail_y = y + 8.5;

    // Dark slim background track with crisp border
    draw_rounded_rect(graphics, track_x, rail_y, track_w, rail_h, 2.5, Some(0xFF222B38), Some(0xFF475569), 1.0);

    // Filled progress rail
    let ratio = ((value - min) / (max - min)).clamp(0.0, 1.0);
    let fill_w = track_w * ratio;
    if fill_w > 0.0 {
        draw_rounded_rect(graphics, track_x, rail_y, fill_w, rail_h, 2.5, Some(accent), None, 0.0);
    }

    // 14px Circular thumb handle centered on the rail
    let thumb_radius = 7.0;
    let thumb_cx = track_x + track_w * ratio;
    let thumb_cy = rail_y + 2.5;

    let mut brush_thumb: usize = 0;
    if GdipCreateSolidFill(0xFFFFFFFF, &mut brush_thumb) == 0 {
        GdipFillEllipse(graphics, brush_thumb, thumb_cx - thumb_radius, thumb_cy - thumb_radius, 14.0, 14.0);
        GdipDeleteBrush(brush_thumb);
    }
    let mut pen_thumb: usize = 0;
    if GdipCreatePen1(accent, 1.5, UNIT_PIXEL, &mut pen_thumb) == 0 {
        let mut path: usize = 0;
        if GdipCreatePath(0, &mut path) == 0 {
            GdipAddPathArc(path, thumb_cx - thumb_radius, thumb_cy - thumb_radius, 14.0, 14.0, 0.0, 360.0);
            GdipClosePathFigure(path);
            GdipDrawPath(graphics, pen_thumb, path);
            GdipDeletePath(path);
        }
        GdipDeletePen(pen_thumb);
    }

    // 3. Inset Numeric box matching CSS .ol-number
    let num_x = track_x + track_w + 10.0;
    let num_h = 24.0;
    let num_y = y - 1.0;
    // background: #1E2632, border: 1px solid #475569
    draw_rounded_rect(graphics, num_x, num_y, num_w, num_h, 4.0, Some(0xFF1E2632), Some(0xFF475569), 1.0);
    let val_str = if max > 50.0 { format!("{:.0}", value) } else { format!("{:.2}", value) };
    draw_text(graphics, &val_str, font_mono, num_x, num_y, num_w - 6.0, num_h, 0xFFFFFFFF, STRING_ALIGN_FAR, STRING_ALIGN_CENTER);
}

#[derive(Clone, Copy, Debug)]
pub struct Tab0Offsets {
    pub y_top: f32,
    pub has_mfg: bool,
    pub y_mfg_divider: f32,
    pub y_mfg_check: f32,
    pub y_mfg_pills: f32,
    pub has_presr: bool,
    pub y_presr_divider: f32,
    pub y_presr_check: f32,
    pub y_presr_pills: f32,
    pub y_global_divider: f32,
    pub y_global_title: f32,
    pub y_struct: f32,
    pub y_tone: f32,
    pub y_mask_divider: f32,
    pub y_mask_check: f32,
    pub y_char_struct: f32,
    pub y_style_divider: f32,
    pub y_style_title: f32,
    pub y_style_pills: f32,
    pub y_more_divider: f32,
    pub y_more_title: f32,
    pub y_scroll_view: f32,
    pub scroll_view_h: f32,
}

pub fn compute_tab0_offsets(has_mfg: bool, has_presr: bool) -> Tab0Offsets {
    let y_top = 58.0;
    let mut cur_y = y_top + 28.0;

    let (y_mfg_div, y_mfg_chk, y_mfg_pills) = if has_mfg {
        let div = cur_y;
        let chk = cur_y + 8.0;
        let pills = chk + 26.0;
        cur_y = pills + 34.0;
        (div, chk, pills)
    } else {
        (0.0, 0.0, 0.0)
    };

    let (y_presr_div, y_presr_chk, y_presr_pills) = if has_presr {
        let div = cur_y;
        let chk = cur_y + 8.0;
        let pills = chk + 26.0;
        cur_y = pills + 34.0;
        (div, chk, pills)
    } else {
        (0.0, 0.0, 0.0)
    };

    let y_global_divider = cur_y;
    let y_global_title = y_global_divider + 8.0;
    let y_struct = y_global_title + 22.0;
    let y_tone = y_struct + 28.0;

    let y_mask_divider = y_tone + 30.0;
    let y_mask_check = y_mask_divider + 8.0;
    let y_char_struct = y_mask_check + 26.0;

    let y_style_divider = y_char_struct + 30.0;
    let y_style_title = y_style_divider + 8.0;
    let y_style_pills = y_style_title + 22.0;

    let y_more_divider = y_style_pills + 42.0;
    let y_more_title = y_more_divider + 8.0;
    let y_scroll_view = y_more_title + 22.0;
    let max_bottom: f32 = 648.0 - 36.0;
    let scroll_view_h = f32::max(max_bottom - y_scroll_view, 90.0);

    Tab0Offsets {
        y_top,
        has_mfg,
        y_mfg_divider: y_mfg_div,
        y_mfg_check: y_mfg_chk,
        y_mfg_pills,
        has_presr,
        y_presr_divider: y_presr_div,
        y_presr_check: y_presr_chk,
        y_presr_pills,
        y_global_divider,
        y_global_title,
        y_struct,
        y_tone,
        y_mask_divider,
        y_mask_check,
        y_char_struct,
        y_style_divider,
        y_style_title,
        y_style_pills,
        y_more_divider,
        y_more_title,
        y_scroll_view,
        scroll_view_h,
    }
}

fn set_ini(text: &str, section: &str, key: &str, value: &str) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut lines: Vec<String> = text.replace('\u{feff}', "").lines().map(|s| s.to_string()).collect();
    let header = format!("[{}]", section).to_lowercase();
    let start_idx = lines.iter().position(|l| l.trim().to_lowercase() == header);

    if let Some(start) = start_idx {
        let mut end = lines.len();
        for i in (start + 1)..lines.len() {
            let t = lines[i].trim();
            if t.starts_with('[') && t.ends_with(']') {
                end = i;
                break;
            }
        }
        let wanted = key.to_lowercase();
        let mut changed = false;
        for i in (start + 1)..end {
            let line = &lines[i];
            let trimmed = line.trim();
            if !trimmed.starts_with(';') && !trimmed.starts_with('#') {
                if let Some((k, _)) = trimmed.split_once('=') {
                    if k.trim().to_lowercase() == wanted {
                        lines[i] = format!("{}={}", key, value);
                        changed = true;
                        break;
                    }
                }
            }
        }
        if !changed {
            lines.insert(end, format!("{}={}", key, value));
        }
    } else {
        if !lines.is_empty() && !lines.last().unwrap().is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("[{}]", section));
        lines.push(format!("{}={}", key, value));
    }
    lines.join(newline)
}

fn configure_mfg_unlock_ini(existing: &str, multiplier: Option<u32>) -> String {
    let mult = multiplier.unwrap_or(4);
    let force_multiplier = match mult {
        2 => "2",
        3 => "3",
        4 => "4",
        _ => "0",
    };
    let mut lines: Vec<String> = existing.lines().map(|s| s.to_string()).collect();
    let mut section_start = None;
    let mut section_end = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("[RenoDX.MFGUnlock]") {
            section_start = Some(i);
        } else if section_start.is_some() && trimmed.starts_with('[') && trimmed.ends_with(']') {
            section_end = Some(i);
            break;
        }
    }

    let entries = vec![
        ("Enabled", "1".to_string()),
        ("LogLevel", "0".to_string()),
        ("Method", "0".to_string()),
        ("ForceMultiplier", force_multiplier.to_string()),
        ("MaxCount", "4".to_string()),
        ("TemporalFix", "1".to_string()),
    ];

    if let Some(start) = section_start {
        let end = section_end.unwrap_or(lines.len());
        for (k, v) in entries {
            let mut found = false;
            for i in (start + 1)..end {
                let trimmed = lines[i].trim();
                if let Some((existing_k, _)) = trimmed.split_once('=') {
                    if existing_k.trim().eq_ignore_ascii_case(k) {
                        lines[i] = format!("{}={}", k, v);
                        found = true;
                        break;
                    }
                }
            }
            if !found {
                lines.insert(end, format!("{}={}", k, v));
            }
        }
    } else {
        if !lines.is_empty() && !lines.last().unwrap().is_empty() {
            lines.push(String::new());
        }
        lines.push("[RenoDX.MFGUnlock]".to_string());
        for (k, v) in entries {
            lines.push(format!("{}={}", k, v));
        }
    }
    let newline = if existing.contains("\r\n") { "\r\n" } else { "\n" };
    lines.join(newline)
}

pub fn update_mfg_ini(game_dir: &std::path::Path, enabled: bool, multiplier: u32) {
    let reshade_ini = game_dir.join("ReShade.ini");
    let content = if reshade_ini.exists() {
        fs::read_to_string(&reshade_ini).unwrap_or_default()
    } else {
        String::new()
    };
    let updated = if enabled {
        configure_mfg_unlock_ini(&content, Some(multiplier))
    } else {
        set_ini(&content, "RenoDX.MFGUnlock", "Enabled", "0")
    };
    let _ = fs::write(&reshade_ini, updated);
}

pub fn update_presr_ini(game_dir: &std::path::Path, enabled: bool, passes: u32) {
    let opti_ini = game_dir.join("OptiScaler.ini");
    if opti_ini.exists() {
        let content = fs::read_to_string(&opti_ini).unwrap_or_default();
        let mut updated = set_ini(&content, "DlssNr", "Enabled", if enabled { "true" } else { "false" });
        updated = set_ini(&updated, "DlssNr", "RunBeforeSR", if enabled { "true" } else { "false" });
        updated = set_ini(&updated, "DlssNr", "Passes", &passes.to_string());
        let _ = fs::write(&opti_ini, updated);
    }
}

pub fn sync_presr_to_optiscaler_ini(state: &OverlayUiState) {
    if let Some(ref dir) = state.active_game_dir {
        let opti_ini = dir.join("OptiScaler.ini");
        if opti_ini.exists() {
            let content = fs::read_to_string(&opti_ini).unwrap_or_default();
            let mut updated = set_ini(&content, "DlssNr", "Enabled", if state.presr_enabled { "true" } else { "false" });
            updated = set_ini(&updated, "DlssNr", "RunBeforeSR", if state.presr_enabled { "true" } else { "false" });
            updated = set_ini(&updated, "DlssNr", "Passes", &state.presr_passes.to_string());
            updated = set_ini(&updated, "DlssNr", "LocalStructure", &format!("{:.2}", state.structure_intensity));
            updated = set_ini(&updated, "DlssNr", "LocalTone", &format!("{:.2}", state.tone_intensity));
            updated = set_ini(&updated, "DlssNr", "SkinStructure", &format!("{:.2}", state.char_structure));
            updated = set_ini(&updated, "DlssNr", "AutoMask", if state.character_mask { "true" } else { "false" });
            updated = set_ini(&updated, "DlssNr", "Style", &state.nr_style.to_string());
            if state.overall_intensity > 0.0 {
                updated = set_ini(&updated, "DlssNr", "Intensity", &format!("{:.2}", state.overall_intensity));
            }
            let _ = fs::write(&opti_ini, updated);
        }
    }
}

/// Renders the complete in-game overlay surface (534x720) in pure Rust
pub fn render_overlay_surface(state: &OverlayUiState, sequence: u32) -> Vec<u8> {
    let width = PANEL_WIDTH as i32;
    let height = PANEL_HEIGHT as i32;
    let byte_count = (width * height * 4) as usize;

    let mut out_buffer = Vec::with_capacity(24 + byte_count);

    let mut startup_token: usize = 0;
    let input = GdiplusStartupInput {
        gdiplus_version: 1,
        debug_event_callback: 0,
        suppress_background_thread: 0,
        suppress_external_codecs: 0,
    };

    out_buffer.extend_from_slice(&FRAME_MAGIC.to_le_bytes());
    out_buffer.extend_from_slice(&1u32.to_le_bytes());
    out_buffer.extend_from_slice(&sequence.to_le_bytes());
    out_buffer.extend_from_slice(&PANEL_WIDTH.to_le_bytes());
    out_buffer.extend_from_slice(&PANEL_HEIGHT.to_le_bytes());
    out_buffer.extend_from_slice(&(byte_count as u32).to_le_bytes());

    unsafe {
        if GdiplusStartup(&mut startup_token, &input, std::ptr::null_mut()) != 0 {
            out_buffer.resize(24 + byte_count, 0);
            return out_buffer;
        }

        let mut bitmap: usize = 0;
        if GdipCreateBitmapFromScan0(width, height, 0, PIXEL_FORMAT_32BPP_PARGB, std::ptr::null_mut(), &mut bitmap) != 0 {
            GdiplusShutdown(startup_token);
            out_buffer.resize(24 + byte_count, 0);
            return out_buffer;
        }

        let mut graphics: usize = 0;
        GdipGetImageGraphicsContext(bitmap, &mut graphics);
        GdipSetSmoothingMode(graphics, SMOOTHING_MODE_ANTI_ALIAS);
        // AntiAliasGridFit (3) aligns glyph stems to whole pixels while keeping smooth antialiased curves, eliminating blurry or broken text
        GdipSetTextRenderingHint(graphics, TEXT_RENDERING_HINT_ANTI_ALIAS_GRID_FIT);
        GdipSetPixelOffsetMode(graphics, PIXEL_OFFSET_MODE_HALF);
        GdipSetInterpolationMode(graphics, INTERPOLATION_MODE_HIGH_QUALITY_BICUBIC);
        GdipGraphicsClear(graphics, 0x0000_0000);

        // Fonts
        let segoe_wide: Vec<u16> = "Segoe UI\0".encode_utf16().collect();
        let consolas_wide: Vec<u16> = "Consolas\0".encode_utf16().collect();

        let mut family_segoe: usize = 0;
        GdipCreateFontFamilyFromName(segoe_wide.as_ptr(), 0, &mut family_segoe);
        let mut family_cons: usize = 0;
        GdipCreateFontFamilyFromName(consolas_wide.as_ptr(), 0, &mut family_cons);

        let mut font_header: usize = 0;
        let mut font_badge: usize = 0;
        let mut font_subhead: usize = 0;
        let mut font_bold: usize = 0;
        let mut font_body: usize = 0;
        let mut font_small: usize = 0;
        let mut font_mono: usize = 0;

        GdipCreateFont(family_segoe, 11.0, FONT_STYLE_BOLD, UNIT_PIXEL, &mut font_header);
        GdipCreateFont(family_segoe, 9.5, FONT_STYLE_BOLD, UNIT_PIXEL, &mut font_badge);
        GdipCreateFont(family_segoe, 11.0, FONT_STYLE_BOLD, UNIT_PIXEL, &mut font_subhead);
        GdipCreateFont(family_segoe, 12.0, FONT_STYLE_BOLD, UNIT_PIXEL, &mut font_bold);
        GdipCreateFont(family_segoe, 12.0, FONT_STYLE_REGULAR, UNIT_PIXEL, &mut font_body);
        GdipCreateFont(family_segoe, 10.5, FONT_STYLE_REGULAR, UNIT_PIXEL, &mut font_small);
        GdipCreateFont(family_cons, 11.0, FONT_STYLE_REGULAR, UNIT_PIXEL, &mut font_mono);

        let accent = state.accent_color;
        let soft = state.soft_color;
        let back = state.back_color;
        let bright = state.bright_color;

        // Card Dimensions matching CSS .ol-panel
        let card_x = 12.0;
        let card_y = 10.0;
        let card_w = (width - 24) as f32;
        let card_h = 638.0;

        // Authentic radial/linear gradient background (back -> #101519) + 1px accent border (#8bc400) + 18px radius
        draw_gradient_rounded_rect(graphics, card_x, card_y, card_w, card_h, 18.0, back, 0xFF101519, Some(accent), 1.0);

        let content_x = card_x + 24.0;
        let content_w = card_w - 48.0;

        // Header Region (matching CSS header)
        let head_y = card_y + 16.0;
        let title_text = if state.mode == 0 { "DLSS 5 STUDIO CONTROLS" } else { "DLSS 5 STUDIO • INJECTED TOOLS" };
        draw_text(graphics, title_text, font_header, content_x, head_y, 250.0, 20.0, 0xFFFFFFFF, STRING_ALIGN_NEAR, STRING_ALIGN_CENTER);

        // Header sync badge: IPC 20Hz · LIVE (named pipe IPC poll frequency, not GPU render FPS)
        let fps_w = 114.0;
        let fps_h = 18.0;
        let badge_w = 78.0;
        let badge_h = 18.0;
        let badge_x = content_x + content_w - badge_w;
        let fps_x = badge_x - fps_w - 8.0;

        let fps_text = if state.fps > 0.0 {
            format!("IPC {:.0}Hz · LIVE", state.fps)
        } else {
            "IPC · LIVE".to_string()
        };
        draw_rounded_rect(graphics, fps_x, head_y, fps_w, fps_h, 4.0, Some(0x33202528), Some(0x55768167), 1.0);
        draw_text(graphics, &fps_text, font_mono, fps_x, head_y, fps_w, fps_h, 0xFFE2E8F0, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);

        // Status badge: CONNECTED matching CSS .ol-prototype
        draw_rounded_rect(graphics, badge_x, head_y, badge_w, badge_h, 4.0, Some(soft), Some(accent), 1.0);
        let badge_text = if state.nr_available { "CONNECTED" } else { "STANDBY" };
        draw_text(graphics, badge_text, font_badge, badge_x, head_y, badge_w, badge_h, bright, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);

        // Hairline Divider Line matching CSS section border-top: 1px solid #c8cab31d
        let draw_divider = |y_pos: f32| {
            let mut pen_div: usize = 0;
            if GdipCreatePen1(0x1DC8CAB3, 1.0, UNIT_PIXEL, &mut pen_div) == 0 {
                GdipDrawLine(graphics, pen_div, content_x, y_pos, content_x + content_w, y_pos);
                GdipDeletePen(pen_div);
            }
        };

        draw_divider(head_y + 26.0);

        if state.mode == 0 {
            let offsets = compute_tab0_offsets(state.has_mfg, state.has_presr);

            // 1. Top row: DLSS ON (left) and ReShade FX (right)
            let half_w = (content_w - 12.0) / 2.0;
            draw_checkbox(graphics, "DLSS ON", state.dlss_on, content_x, offsets.y_top, half_w, font_bold, font_small, accent, Some("RenoDX live"));
            draw_checkbox(graphics, "ReShade FX", state.reshade_fx_enabled, content_x + half_w + 12.0, offsets.y_top, half_w, font_bold, font_small, accent, Some("Shader Effects"));

            // 2. MULTI FRAME GENERATION (MFG) if supported/installed
            if offsets.has_mfg {
                draw_divider(offsets.y_mfg_divider);
                draw_checkbox(graphics, "MULTI FRAME GENERATION (MFG)", state.mfg_enabled, content_x, offsets.y_mfg_check, content_w, font_bold, font_small, accent, Some("Hardware Frame Gen"));
                let pill_w = (content_w - 24.0) / 4.0;
                let mfg_labels = ["1x (Auto)", "2x", "3x", "4x"];
                for (idx, lbl) in mfg_labels.iter().enumerate() {
                    let px = content_x + idx as f32 * (pill_w + 8.0);
                    let selected = state.mfg_multiplier == (idx + 1) as u32;
                    if selected {
                        draw_rounded_rect(graphics, px, offsets.y_mfg_pills, pill_w, 26.0, 6.0, Some(soft), Some(accent), 1.0);
                        draw_text(graphics, lbl, font_bold, px, offsets.y_mfg_pills, pill_w, 26.0, bright, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
                    } else {
                        draw_rounded_rect(graphics, px, offsets.y_mfg_pills, pill_w, 26.0, 6.0, Some(0xFF1E2632), Some(0xFF64748B), 1.0);
                        draw_text(graphics, lbl, font_bold, px, offsets.y_mfg_pills, pill_w, 26.0, 0xFFF1F5F9, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
                    }
                }
            }

            // 3. PRE-SR MULTIPASS CLARITY if supported/installed
            if offsets.has_presr {
                draw_divider(offsets.y_presr_divider);
                draw_checkbox(graphics, "PRE-SR MULTIPASS CLARITY", state.presr_enabled, content_x, offsets.y_presr_check, content_w, font_bold, font_small, accent, Some("OptiScaler Pre-Pass"));
                let pill_w = (content_w - 16.0) / 3.0;
                let presr_labels = ["1x Pass", "2x Passes", "3x Passes"];
                for (idx, lbl) in presr_labels.iter().enumerate() {
                    let px = content_x + idx as f32 * (pill_w + 8.0);
                    let selected = state.presr_passes == (idx + 1) as u32;
                    if selected {
                        draw_rounded_rect(graphics, px, offsets.y_presr_pills, pill_w, 26.0, 6.0, Some(soft), Some(accent), 1.0);
                        draw_text(graphics, lbl, font_bold, px, offsets.y_presr_pills, pill_w, 26.0, bright, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
                    } else {
                        draw_rounded_rect(graphics, px, offsets.y_presr_pills, pill_w, 26.0, 6.0, Some(0xFF1E2632), Some(0xFF64748B), 1.0);
                        draw_text(graphics, lbl, font_bold, px, offsets.y_presr_pills, pill_w, 26.0, 0xFFF1F5F9, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
                    }
                }
            }

            // 4. GLOBAL CONTROLS
            draw_divider(offsets.y_global_divider);
            draw_text(graphics, "GLOBAL CONTROLS", font_subhead, content_x, offsets.y_global_title, content_w, 18.0, 0xFFFFFFFF, STRING_ALIGN_NEAR, STRING_ALIGN_CENTER);
            draw_slider(graphics, "Structure Intensity", state.structure_intensity, 0.0, 2.0, content_x, offsets.y_struct, content_w, font_body, font_mono, accent);
            draw_slider(graphics, "Tone Intensity", state.tone_intensity, 0.0, 2.0, content_x, offsets.y_tone, content_w, font_body, font_mono, accent);

            // 5. CHARACTER MASK
            draw_divider(offsets.y_mask_divider);
            draw_checkbox(graphics, "CHARACTER MASK", state.character_mask, content_x, offsets.y_mask_check, content_w, font_bold, font_small, accent, Some("RenoDX character mask"));
            draw_slider(graphics, "Character/Skin Structure", state.char_structure, -1.0, 2.0, content_x, offsets.y_char_struct, content_w, font_body, font_mono, accent);

            // 6. NR STYLE (Three Radio Pills)
            draw_divider(offsets.y_style_divider);
            draw_text(graphics, "NR STYLE", font_subhead, content_x, offsets.y_style_title, content_w, 18.0, 0xFFFFFFFF, STRING_ALIGN_NEAR, STRING_ALIGN_CENTER);
            let pill_w = (content_w - 16.0) / 3.0;
            let pill_h = 32.0;
            let models = ["Model A · Default", "Model B · Natural", "Model C · Cinematic"];
            for (idx, name) in models.iter().enumerate() {
                let px = content_x + idx as f32 * (pill_w + 8.0);
                let selected = state.nr_style == idx;
                if selected {
                    draw_rounded_rect(graphics, px, offsets.y_style_pills, pill_w, pill_h, 8.0, Some(soft), Some(accent), 1.0);
                    draw_text(graphics, name, font_bold, px, offsets.y_style_pills, pill_w, pill_h, bright, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
                } else {
                    draw_rounded_rect(graphics, px, offsets.y_style_pills, pill_w, pill_h, 8.0, Some(0xFF1E2632), Some(0xFF64748B), 1.0);
                    draw_text(graphics, name, font_bold, px, offsets.y_style_pills, pill_w, pill_h, 0xFFF1F5F9, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
                }
            }

            // 7. MORE RENODX CONTROLS (Scroll Viewport)
            draw_divider(offsets.y_more_divider);
            draw_text(graphics, "MORE RENODX CONTROLS", font_subhead, content_x, offsets.y_more_title, content_w, 18.0, 0xFFFFFFFF, STRING_ALIGN_NEAR, STRING_ALIGN_CENTER);

            let scroll_view_y = offsets.y_scroll_view;
            let scroll_view_h = offsets.scroll_view_h;
            GdipSetClipRect(graphics, content_x, scroll_view_y, content_w - 14.0, scroll_view_h, COMBINE_MODE_REPLACE);

            let mut sc_y = scroll_view_y - state.more_scroll;
            draw_slider(graphics, "Overall Intensity", state.overall_intensity, 0.0, 2.0, content_x, sc_y, content_w - 16.0, font_body, font_mono, accent);
            sc_y += 28.0;
            draw_slider(graphics, "Local Tone Intensity", state.local_tone, 0.0, 2.0, content_x, sc_y, content_w - 16.0, font_body, font_mono, accent);
            sc_y += 28.0;
            draw_slider(graphics, "Diffuse White (nits)", state.diffuse_white, 80.0, 500.0, content_x, sc_y, content_w - 16.0, font_body, font_mono, accent);
            sc_y += 28.0;
            draw_slider(graphics, "Motion Scale X", state.motion_x, -2.0, 2.0, content_x, sc_y, content_w - 16.0, font_body, font_mono, accent);
            sc_y += 28.0;
            draw_slider(graphics, "Motion Scale Y", state.motion_y, -2.0, 2.0, content_x, sc_y, content_w - 16.0, font_body, font_mono, accent);
            sc_y += 30.0;
            draw_checkbox(graphics, "NR UI Correction", state.nr_ui_correction, content_x, sc_y, content_w - 16.0, font_bold, font_small, accent, None);
            sc_y += 26.0;
            draw_checkbox(graphics, "Enable Upscaling (WIP)", state.enable_upscaling, content_x, sc_y, content_w - 16.0, font_bold, font_small, accent, None);

            GdipResetClip(graphics);

            // Neon scrollbar
            let sb_x = content_x + content_w - 6.0;
            draw_rounded_rect(graphics, sb_x, scroll_view_y, 6.0, scroll_view_h, 3.0, Some(0xFF1E2634), None, 0.0);
            let thumb_h = 55.0;
            let thumb_max = (scroll_view_h - thumb_h).max(10.0);
            let thumb_y = scroll_view_y + (state.more_scroll / 140.0 * thumb_max).clamp(0.0, thumb_max);
            draw_rounded_rect(graphics, sb_x, thumb_y, 6.0, thumb_h, 3.0, Some(accent), None, 0.0);
            draw_text(graphics, "▲", font_small, sb_x - 3.0, scroll_view_y - 12.0, 12.0, 10.0, accent, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
            draw_text(graphics, "▼", font_small, sb_x - 3.0, scroll_view_y + scroll_view_h + 2.0, 12.0, 10.0, accent, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);

            // Footer
            draw_divider(card_y + card_h - 36.0);
            let foot_y = card_y + card_h - 28.0;
            draw_text(graphics, "Live RenoDX v4.7 settings. A/B/C: NR Style. In-Game Render FPS: Alt+R / Home", font_small, content_x, foot_y, content_w, 20.0, 0xFFCBD5E1, STRING_ALIGN_NEAR, STRING_ALIGN_NEAR);

        } else {
            // ---------------- TAB 1: LIVE TOOLS ----------------
            let sub_y = 56.0;
            draw_text(graphics, "RenoDX v4.7 controls use its original callback. Experimental adapter; original tools available.", font_small, content_x, sub_y, content_w, 24.0, 0xFFCBD5E1, STRING_ALIGN_NEAR, STRING_ALIGN_NEAR);

            let y_reshade = 84.0;
            draw_checkbox(graphics, "ReShade shader effects", state.reshade_fx_enabled, content_x, y_reshade, content_w, font_bold, font_small, accent, None);

            draw_divider(112.0);
            draw_text(graphics, "RenoDX v4.7", font_subhead, content_x, 120.0, content_w, 18.0, 0xFFFFFFFF, STRING_ALIGN_NEAR, STRING_ALIGN_CENTER);

            let y_struct = 142.0;
            draw_slider(graphics, "Structure Intensity", state.structure_intensity, 0.0, 2.0, content_x, y_struct, content_w, font_body, font_mono, accent);
            let y_tone = 170.0;
            draw_slider(graphics, "Global Tone Intensity", state.tone_intensity, 0.0, 2.0, content_x, y_tone, content_w, font_body, font_mono, accent);
            let y_dlss = 200.0;
            draw_checkbox(graphics, "Enable DLSS Neural Rendering", state.dlss_neural_rendering, content_x, y_dlss, content_w, font_bold, font_small, accent, None);
            let y_mask = 224.0;
            draw_checkbox(graphics, "Automatic / Character Mask", state.character_mask, content_x, y_mask, content_w, font_bold, font_small, accent, None);
            let y_char = 250.0;
            draw_slider(graphics, "Character/Skin Structure", state.char_structure, -1.0, 2.0, content_x, y_char, content_w, font_body, font_mono, accent);
            let y_overall = 278.0;
            draw_slider(graphics, "Overall Intensity", state.overall_intensity, 0.0, 2.0, content_x, y_overall, content_w, font_body, font_mono, accent);
            let y_localtone = 306.0;
            draw_slider(graphics, "Local Tone Intensity", state.local_tone, 0.0, 2.0, content_x, y_localtone, content_w, font_body, font_mono, accent);
            let y_diffuse = 334.0;
            draw_slider(graphics, "Diffuse White (nits)", state.diffuse_white, 80.0, 500.0, content_x, y_diffuse, content_w, font_body, font_mono, accent);
            let y_motx = 362.0;
            draw_slider(graphics, "Motion Scale X", state.motion_x, -2.0, 2.0, content_x, y_motx, content_w, font_body, font_mono, accent);
            let y_moty = 390.0;
            draw_slider(graphics, "Motion Scale Y", state.motion_y, -2.0, 2.0, content_x, y_moty, content_w, font_body, font_mono, accent);
            let y_uicorr = 420.0;
            draw_checkbox(graphics, "NR UI Correction", state.nr_ui_correction, content_x, y_uicorr, content_w, font_bold, font_small, accent, None);
            let y_upscale = 444.0;
            draw_checkbox(graphics, "Enable Upscaling (WIP)", state.enable_upscaling, content_x, y_upscale, content_w, font_bold, font_small, accent, None);

            draw_divider(card_y + card_h - 36.0);
            let foot_y = card_y + card_h - 28.0;
            draw_text(graphics, "F8: show/hide • Drag header: move • Esc: close • In-Game Render FPS: Alt+R / Home", font_small, content_x, foot_y, content_w, 20.0, 0xFFCBD5E1, STRING_ALIGN_NEAR, STRING_ALIGN_NEAR);
        }

        // Mode switcher buttons at the bottom matching CSS .ol-live-modes
        let btn_y = card_y + card_h + 10.0;
        let btn_w = (card_w - 12.0) / 2.0;
        let btn_h = 38.0;

        // [ Live tools ]
        let live_active = state.mode == 1;
        let p1_x = card_x;
        if live_active {
            draw_rounded_rect(graphics, p1_x, btn_y, btn_w, btn_h, 8.0, Some(0xFF3E4A27), Some(accent), 1.5);
            draw_text(graphics, "Live tools", font_bold, p1_x, btn_y, btn_w, btn_h, bright, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
        } else {
            draw_rounded_rect(graphics, p1_x, btn_y, btn_w, btn_h, 8.0, Some(0xFF1E2632), Some(0xFF64748B), 1.0);
            draw_text(graphics, "Live tools", font_bold, p1_x, btn_y, btn_w, btn_h, 0xFFF1F5F9, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
        }

        // [ DLSS controls ]
        let dlss_active = state.mode == 0;
        let p2_x = card_x + btn_w + 12.0;
        if dlss_active {
            draw_rounded_rect(graphics, p2_x, btn_y, btn_w, btn_h, 8.0, Some(0xFF3E4A27), Some(accent), 1.5);
            draw_text(graphics, "DLSS controls", font_bold, p2_x, btn_y, btn_w, btn_h, bright, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
        } else {
            draw_rounded_rect(graphics, p2_x, btn_y, btn_w, btn_h, 8.0, Some(0xFF1E2632), Some(0xFF64748B), 1.0);
            draw_text(graphics, "DLSS controls", font_bold, p2_x, btn_y, btn_w, btn_h, 0xFFF1F5F9, STRING_ALIGN_CENTER, STRING_ALIGN_CENTER);
        }

        // Copy pixels to out_buffer with straight alpha un-premultiplication
        let rect_lock = RectI { x: 0, y: 0, width, height };
        let mut bitmap_data = BitmapData {
            width: 0,
            height: 0,
            stride: 0,
            pixel_format: 0,
            scan0: std::ptr::null_mut(),
            reserved: 0,
        };

        if GdipBitmapLockBits(bitmap, &rect_lock, 1, PIXEL_FORMAT_32BPP_PARGB, &mut bitmap_data) == 0 {
            let scan0 = bitmap_data.scan0;
            let stride = bitmap_data.stride as usize;

            for row in 0..height as usize {
                let row_ptr = scan0.add(row * stride);
                for col in 0..width as usize {
                    let p = row_ptr.add(col * 4);
                    let b = *p;
                    let g = *p.add(1);
                    let r = *p.add(2);
                    let a = *p.add(3);

                    if a > 0 && a < 255 {
                        let scale = 255.0 / (a as f32);
                        let r_straight = ((r as f32) * scale).min(255.0).round() as u8;
                        let g_straight = ((g as f32) * scale).min(255.0).round() as u8;
                        let b_straight = ((b as f32) * scale).min(255.0).round() as u8;
                        out_buffer.push(r_straight);
                        out_buffer.push(g_straight);
                        out_buffer.push(b_straight);
                        out_buffer.push(a);
                    } else {
                        out_buffer.push(r);
                        out_buffer.push(g);
                        out_buffer.push(b);
                        out_buffer.push(a);
                    }
                }
            }

            GdipBitmapUnlockBits(bitmap, &mut bitmap_data);
        }

        GdipDeleteFont(font_header);
        GdipDeleteFont(font_badge);
        GdipDeleteFont(font_subhead);
        GdipDeleteFont(font_bold);
        GdipDeleteFont(font_body);
        GdipDeleteFont(font_small);
        GdipDeleteFont(font_mono);
        GdipDeleteFontFamily(family_segoe);
        GdipDeleteFontFamily(family_cons);
        GdipDeleteGraphics(graphics);
        GdipDisposeImage(bitmap);
        GdiplusShutdown(startup_token);
    }

    out_buffer
}

/// Dispatches a 24-byte command packet down the named pipe to the add-on
pub fn send_command_packet(pipe: isize, cmd: &OverlayCommandPacket) {
    let mut written: u32 = 0;
    let mut bytes = [0u8; 24];
    bytes[0..4].copy_from_slice(&cmd.magic.to_le_bytes());
    bytes[4..8].copy_from_slice(&cmd.version.to_le_bytes());
    bytes[8..12].copy_from_slice(&cmd.epoch.to_le_bytes());
    bytes[12..16].copy_from_slice(&cmd.control_id.to_le_bytes());
    bytes[16..20].copy_from_slice(&cmd.control_kind.to_le_bytes());
    bytes[20..24].copy_from_slice(&cmd.value.to_le_bytes());

    unsafe {
        let _ = WriteFile(pipe, bytes.as_ptr(), 24, &mut written, std::ptr::null_mut());
    }
}

/// Processes an incoming input event packet from the in-game addon
pub fn handle_input_packet(
    state: &mut OverlayUiState,
    action: u32,
    x: i32,
    y: i32,
    value: i32,
) -> bool {
    let mut changed = false;

    match action {
        0 => {} // Ping / Frame ACK
        1 => {
            // Mouse Move
            state.mouse_x = x;
            state.mouse_y = y;

            if state.mouse_down {
                if let Some(ref slider) = state.active_slider.clone() {
                    let card_x = 12.0;
                    let content_x = card_x + 24.0;
                    let content_w = (PANEL_WIDTH - 24) as f32 - 48.0;
                    let track_x = content_x + SLIDER_LABEL_WIDTH;
                    let num_w = SLIDER_NUM_WIDTH;
                    let track_w = content_w - SLIDER_LABEL_WIDTH - num_w - SLIDER_GAP;

                    let ratio = ((x as f32 - track_x) / track_w).clamp(0.0, 1.0);
                    let ep = state.current_epoch;

                    match slider.as_str() {
                        "structure_intensity" => {
                            state.structure_intensity = (ratio * 2.0 * 100.0).round() / 100.0;
                            state.pending_command = Some(OverlayCommandPacket {
                                magic: COMMAND_MAGIC,
                                version: 1,
                                epoch: ep,
                                control_id: 101,
                                control_kind: 0,
                                value: state.structure_intensity,
                            });
                            changed = true;
                        }
                        "tone_intensity" => {
                            state.tone_intensity = (ratio * 2.0 * 100.0).round() / 100.0;
                            state.pending_command = Some(OverlayCommandPacket {
                                magic: COMMAND_MAGIC,
                                version: 1,
                                epoch: ep,
                                control_id: 102,
                                control_kind: 0,
                                value: state.tone_intensity,
                            });
                            changed = true;
                        }
                        "char_structure" => {
                            state.char_structure = (-1.0 + ratio * 3.0 * 100.0).round() / 100.0;
                            state.pending_command = Some(OverlayCommandPacket {
                                magic: COMMAND_MAGIC,
                                version: 1,
                                epoch: ep,
                                control_id: 105,
                                control_kind: 0,
                                value: state.char_structure,
                            });
                            changed = true;
                        }
                        "overall_intensity" => {
                            state.overall_intensity = (ratio * 2.0 * 100.0).round() / 100.0;
                            state.pending_command = Some(OverlayCommandPacket {
                                magic: COMMAND_MAGIC,
                                version: 1,
                                epoch: ep,
                                control_id: 106,
                                control_kind: 0,
                                value: state.overall_intensity,
                            });
                            changed = true;
                        }
                        "local_tone" => {
                            state.local_tone = (ratio * 2.0 * 100.0).round() / 100.0;
                            state.pending_command = Some(OverlayCommandPacket {
                                magic: COMMAND_MAGIC,
                                version: 1,
                                epoch: ep,
                                control_id: 107,
                                control_kind: 0,
                                value: state.local_tone,
                            });
                            changed = true;
                        }
                        "diffuse_white" => {
                            state.diffuse_white = (80.0 + ratio * 420.0).round();
                            state.pending_command = Some(OverlayCommandPacket {
                                magic: COMMAND_MAGIC,
                                version: 1,
                                epoch: ep,
                                control_id: 108,
                                control_kind: 0,
                                value: state.diffuse_white,
                            });
                            changed = true;
                        }
                        "motion_x" => {
                            state.motion_x = (-2.0 + ratio * 4.0 * 100.0).round() / 100.0;
                            state.pending_command = Some(OverlayCommandPacket {
                                magic: COMMAND_MAGIC,
                                version: 1,
                                epoch: ep,
                                control_id: 109,
                                control_kind: 0,
                                value: state.motion_x,
                            });
                            changed = true;
                        }
                        "motion_y" => {
                            state.motion_y = (-2.0 + ratio * 4.0 * 100.0).round() / 100.0;
                            state.pending_command = Some(OverlayCommandPacket {
                                magic: COMMAND_MAGIC,
                                version: 1,
                                epoch: ep,
                                control_id: 110,
                                control_kind: 0,
                                value: state.motion_y,
                            });
                            changed = true;
                        }
                        _ => {}
                    }
                    if changed && state.has_presr {
                        sync_presr_to_optiscaler_ini(state);
                    }
                }
            }
        }
        2 => {
            // Mouse Down
            state.mouse_down = true;
            let fx = x as f32;
            let fy = y as f32;
            let ep = state.current_epoch;

            let card_x = 12.0;
            let card_y = 10.0;
            let card_w = (PANEL_WIDTH - 24) as f32;
            let card_h = 638.0;

            // Bottom mode switcher pills
            let btn_y = card_y + card_h + 10.0;
            let btn_w = (card_w - 12.0) / 2.0;
            let btn_h = 38.0;

            if fy >= btn_y && fy <= btn_y + btn_h {
                if fx >= card_x && fx <= card_x + btn_w {
                    if state.mode != 1 {
                        state.mode = 1;
                        changed = true;
                    }
                } else if fx >= card_x + btn_w + 12.0 && fx <= card_x + card_w {
                    if state.mode != 0 {
                        state.mode = 0;
                        changed = true;
                    }
                }
                return changed;
            }

            let content_x = card_x + 24.0;
            let content_w = card_w - 48.0;
            let track_x = content_x + SLIDER_LABEL_WIDTH;
            let num_w = SLIDER_NUM_WIDTH;
            let track_w = content_w - SLIDER_LABEL_WIDTH - num_w - SLIDER_GAP;

            if state.mode == 0 {
                // ---------------- TAB 0 CLICKS ----------------
                let offsets = compute_tab0_offsets(state.has_mfg, state.has_presr);
                let half_w = (content_w - 12.0) / 2.0;

                // 1. DLSS ON (left half)
                // When OptiScaler Pre-SR is active, DLSS ON controls Pre-SR in OptiScaler.ini and MUST NOT
                // dispatch control_id 103 to ReShade (which causes double-hook collision and pipeline crashes).
                if fy >= offsets.y_top - 6.0 && fy <= offsets.y_top + 24.0 && fx >= content_x && fx <= content_x + half_w {
                    state.dlss_on = !state.dlss_on;
                    state.dlss_neural_rendering = state.dlss_on;
                    if state.has_presr {
                        state.presr_enabled = state.dlss_on;
                        sync_presr_to_optiscaler_ini(state);
                    } else {
                        let val = if state.dlss_on { 1.0 } else { 0.0 };
                        state.pending_command = Some(OverlayCommandPacket {
                            magic: COMMAND_MAGIC,
                            version: 1,
                            epoch: ep,
                            control_id: 103,
                            control_kind: 1,
                            value: val,
                        });
                    }
                    changed = true;
                }

                // 2. ReShade FX (right half) -> DISPATCHES CONTROL ID 0, KIND 3!
                if fy >= offsets.y_top - 6.0 && fy <= offsets.y_top + 24.0 && fx >= content_x + half_w + 12.0 && fx <= content_x + content_w {
                    state.reshade_fx_enabled = !state.reshade_fx_enabled;
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 0,
                        control_kind: 3,
                        value: if state.reshade_fx_enabled { 1.0 } else { 0.0 },
                    });
                    changed = true;
                }

                // 3. MULTI FRAME GENERATION (MFG) section
                if offsets.has_mfg {
                    // Checkbox
                    if fy >= offsets.y_mfg_check - 4.0 && fy <= offsets.y_mfg_check + 22.0 && fx >= content_x && fx <= content_x + content_w {
                        state.mfg_enabled = !state.mfg_enabled;
                        if let Some(ref dir) = state.active_game_dir {
                            update_mfg_ini(dir, state.mfg_enabled, state.mfg_multiplier);
                        }
                        changed = true;
                    }
                    // 4 Pills: [ 1x (Auto) ] [ 2x ] [ 3x ] [ 4x ]
                    if fy >= offsets.y_mfg_pills && fy <= offsets.y_mfg_pills + 26.0 {
                        let pill_w = (content_w - 24.0) / 4.0;
                        for i in 0..4 {
                            let px = content_x + i as f32 * (pill_w + 8.0);
                            if fx >= px && fx <= px + pill_w {
                                state.mfg_multiplier = (i + 1) as u32;
                                if let Some(ref dir) = state.active_game_dir {
                                    update_mfg_ini(dir, state.mfg_enabled, state.mfg_multiplier);
                                }
                                changed = true;
                            }
                        }
                    }
                }

                // 4. PRE-SR MULTIPASS CLARITY section
                if offsets.has_presr {
                    // Checkbox: toggles Pre-SR in OptiScaler.ini and mirrors state.dlss_on
                    if fy >= offsets.y_presr_check - 4.0 && fy <= offsets.y_presr_check + 22.0 && fx >= content_x && fx <= content_x + content_w {
                        state.presr_enabled = !state.presr_enabled;
                        state.dlss_on = state.presr_enabled;
                        state.dlss_neural_rendering = state.presr_enabled;
                        sync_presr_to_optiscaler_ini(state);
                        changed = true;
                    }
                    // 3 Pills: [ 1x Pass ] [ 2x Passes ] [ 3x Passes ]
                    if fy >= offsets.y_presr_pills && fy <= offsets.y_presr_pills + 26.0 {
                        let pill_w = (content_w - 16.0) / 3.0;
                        for i in 0..3 {
                            let px = content_x + i as f32 * (pill_w + 8.0);
                            if fx >= px && fx <= px + pill_w {
                                state.presr_passes = (i + 1) as u32;
                                sync_presr_to_optiscaler_ini(state);
                                changed = true;
                            }
                        }
                    }
                }

                // 5. Structure Intensity
                if fy >= offsets.y_struct - 12.0 && fy <= offsets.y_struct + 18.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                    state.active_slider = Some("structure_intensity".to_string());
                    let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                    state.structure_intensity = (ratio * 2.0 * 100.0).round() / 100.0;
                    if state.has_presr {
                        sync_presr_to_optiscaler_ini(state);
                    }
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 101,
                        control_kind: 0,
                        value: state.structure_intensity,
                    });
                    changed = true;
                }

                // 6. Tone Intensity
                if fy >= offsets.y_tone - 10.0 && fy <= offsets.y_tone + 20.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                    state.active_slider = Some("tone_intensity".to_string());
                    let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                    state.tone_intensity = (ratio * 2.0 * 100.0).round() / 100.0;
                    if state.has_presr {
                        sync_presr_to_optiscaler_ini(state);
                    }
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 102,
                        control_kind: 0,
                        value: state.tone_intensity,
                    });
                    changed = true;
                }

                // 7. CHARACTER MASK
                if fy >= offsets.y_mask_check - 12.0 && fy <= offsets.y_mask_check + 16.0 && fx >= content_x && fx <= content_x + content_w {
                    state.character_mask = !state.character_mask;
                    if state.has_presr {
                        sync_presr_to_optiscaler_ini(state);
                    }
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 104,
                        control_kind: 1,
                        value: if state.character_mask { 1.0 } else { 0.0 },
                    });
                    changed = true;
                }

                // 8. Character/Skin Structure
                if fy >= offsets.y_char_struct - 10.0 && fy <= offsets.y_char_struct + 20.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                    state.active_slider = Some("char_structure".to_string());
                    let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                    state.char_structure = (-1.0 + ratio * 3.0 * 100.0).round() / 100.0;
                    if state.has_presr {
                        sync_presr_to_optiscaler_ini(state);
                    }
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 105,
                        control_kind: 0,
                        value: state.char_structure,
                    });
                    changed = true;
                }

                // 9. NR STYLE Pills
                let pill_w = (content_w - 16.0) / 3.0;
                if fy >= offsets.y_style_pills - 10.0 && fy <= offsets.y_style_pills + 40.0 {
                    for idx in 0..3 {
                        let px = content_x + idx as f32 * (pill_w + 8.0);
                        if fx >= px && fx <= px + pill_w {
                            if state.nr_style != idx {
                                state.nr_style = idx;
                                if state.has_presr {
                                    sync_presr_to_optiscaler_ini(state);
                                }
                                state.pending_command = Some(OverlayCommandPacket {
                                    magic: COMMAND_MAGIC,
                                    version: 1,
                                    epoch: ep,
                                    control_id: 114,
                                    control_kind: 4,
                                    value: idx as f32,
                                });
                                changed = true;
                            }
                        }
                    }
                }

                // 10. MORE RENODX CONTROLS Viewport
                let scroll_top = offsets.y_scroll_view;
                let scroll_bottom = scroll_top + offsets.scroll_view_h;
                if fy >= scroll_top && fy <= scroll_bottom {
                    let sc_y = fy + state.more_scroll;
                    let base_y = scroll_top;

                    // Overall Intensity (base sc_y = base_y)
                    if sc_y >= base_y - 12.0 && sc_y <= base_y + 18.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                        state.active_slider = Some("overall_intensity".to_string());
                        let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                        state.overall_intensity = (ratio * 2.0 * 100.0).round() / 100.0;
                        state.pending_command = Some(OverlayCommandPacket {
                            magic: COMMAND_MAGIC,
                            version: 1,
                            epoch: ep,
                            control_id: 106,
                            control_kind: 0,
                            value: state.overall_intensity,
                        });
                        changed = true;
                    }

                    // Local Tone Intensity (base sc_y = base_y + 28.0)
                    let y_loc = base_y + 28.0;
                    if sc_y >= y_loc - 10.0 && sc_y <= y_loc + 18.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                        state.active_slider = Some("local_tone".to_string());
                        let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                        state.local_tone = (ratio * 2.0 * 100.0).round() / 100.0;
                        state.pending_command = Some(OverlayCommandPacket {
                            magic: COMMAND_MAGIC,
                            version: 1,
                            epoch: ep,
                            control_id: 107,
                            control_kind: 0,
                            value: state.local_tone,
                        });
                        changed = true;
                    }

                    // Diffuse White (base sc_y = base_y + 56.0)
                    let y_diff = base_y + 56.0;
                    if sc_y >= y_diff - 10.0 && sc_y <= y_diff + 18.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                        state.active_slider = Some("diffuse_white".to_string());
                        let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                        state.diffuse_white = (80.0 + ratio * 420.0).round();
                        state.pending_command = Some(OverlayCommandPacket {
                            magic: COMMAND_MAGIC,
                            version: 1,
                            epoch: ep,
                            control_id: 108,
                            control_kind: 0,
                            value: state.diffuse_white,
                        });
                        changed = true;
                    }

                    // Motion Scale X (base sc_y = base_y + 84.0)
                    let y_motx = base_y + 84.0;
                    if sc_y >= y_motx - 10.0 && sc_y <= y_motx + 18.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                        state.active_slider = Some("motion_x".to_string());
                        let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                        state.motion_x = (-2.0 + ratio * 4.0 * 100.0).round() / 100.0;
                        state.pending_command = Some(OverlayCommandPacket {
                            magic: COMMAND_MAGIC,
                            version: 1,
                            epoch: ep,
                            control_id: 109,
                            control_kind: 0,
                            value: state.motion_x,
                        });
                        changed = true;
                    }

                    // Motion Scale Y (base sc_y = base_y + 112.0)
                    let y_moty = base_y + 112.0;
                    if sc_y >= y_moty - 10.0 && sc_y <= y_moty + 18.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                        state.active_slider = Some("motion_y".to_string());
                        let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                        state.motion_y = (-2.0 + ratio * 4.0 * 100.0).round() / 100.0;
                        state.pending_command = Some(OverlayCommandPacket {
                            magic: COMMAND_MAGIC,
                            version: 1,
                            epoch: ep,
                            control_id: 110,
                            control_kind: 0,
                            value: state.motion_y,
                        });
                        changed = true;
                    }

                    // NR UI Correction (base sc_y = base_y + 142.0)
                    let y_uic = base_y + 142.0;
                    if sc_y >= y_uic - 10.0 && sc_y <= y_uic + 16.0 && fx >= content_x && fx <= content_x + content_w {
                        state.nr_ui_correction = !state.nr_ui_correction;
                        state.pending_command = Some(OverlayCommandPacket {
                            magic: COMMAND_MAGIC,
                            version: 1,
                            epoch: ep,
                            control_id: 111,
                            control_kind: 1,
                            value: if state.nr_ui_correction { 1.0 } else { 0.0 },
                        });
                        changed = true;
                    }

                    // Enable Upscaling (base sc_y = base_y + 168.0)
                    let y_ups = base_y + 168.0;
                    if sc_y >= y_ups - 10.0 && sc_y <= y_ups + 16.0 && fx >= content_x && fx <= content_x + content_w {
                        state.enable_upscaling = !state.enable_upscaling;
                        state.pending_command = Some(OverlayCommandPacket {
                            magic: COMMAND_MAGIC,
                            version: 1,
                            epoch: ep,
                            control_id: 112,
                            control_kind: 1,
                            value: if state.enable_upscaling { 1.0 } else { 0.0 },
                        });
                        changed = true;
                    }
                }
            } else {
                // ---------------- TAB 1 CLICKS (LIVE TOOLS) ----------------
                // 1. ReShade shader effects (cy = 84.0)
                if fy >= 72.0 && fy <= 102.0 && fx >= content_x && fx <= content_x + content_w {
                    state.reshade_fx_enabled = !state.reshade_fx_enabled;
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 0,
                        control_kind: 3,
                        value: if state.reshade_fx_enabled { 1.0 } else { 0.0 },
                    });
                    changed = true;
                }

                // 2. Structure Intensity (cy = 142.0)
                if fy >= 130.0 && fy <= 160.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                    state.active_slider = Some("structure_intensity".to_string());
                    let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                    state.structure_intensity = (ratio * 2.0 * 100.0).round() / 100.0;
                    if state.has_presr {
                        sync_presr_to_optiscaler_ini(state);
                    }
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 101,
                        control_kind: 0,
                        value: state.structure_intensity,
                    });
                    changed = true;
                }

                // 3. Global Tone Intensity (cy = 170.0)
                if fy >= 160.0 && fy <= 190.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                    state.active_slider = Some("tone_intensity".to_string());
                    let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                    state.tone_intensity = (ratio * 2.0 * 100.0).round() / 100.0;
                    if state.has_presr {
                        sync_presr_to_optiscaler_ini(state);
                    }
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 102,
                        control_kind: 0,
                        value: state.tone_intensity,
                    });
                    changed = true;
                }

                // 4. Enable DLSS Neural Rendering (cy = 200.0)
                if fy >= 188.0 && fy <= 216.0 && fx >= content_x && fx <= content_x + content_w {
                    state.dlss_neural_rendering = !state.dlss_neural_rendering;
                    state.dlss_on = state.dlss_neural_rendering;
                    if state.has_presr {
                        state.presr_enabled = state.dlss_neural_rendering;
                        sync_presr_to_optiscaler_ini(state);
                    } else {
                        let val = if state.dlss_neural_rendering { 1.0 } else { 0.0 };
                        state.pending_command = Some(OverlayCommandPacket {
                            magic: COMMAND_MAGIC,
                            version: 1,
                            epoch: ep,
                            control_id: 103,
                            control_kind: 1,
                            value: val,
                        });
                    }
                    changed = true;
                }

                // 5. Automatic / Character Mask (cy = 224.0)
                if fy >= 216.0 && fy <= 242.0 && fx >= content_x && fx <= content_x + content_w {
                    state.character_mask = !state.character_mask;
                    if state.has_presr {
                        sync_presr_to_optiscaler_ini(state);
                    }
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 104,
                        control_kind: 1,
                        value: if state.character_mask { 1.0 } else { 0.0 },
                    });
                    changed = true;
                }

                // 6. Character/Skin Structure (cy = 250.0)
                if fy >= 240.0 && fy <= 268.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                    state.active_slider = Some("char_structure".to_string());
                    let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                    state.char_structure = (-1.0 + ratio * 3.0 * 100.0).round() / 100.0;
                    if state.has_presr {
                        sync_presr_to_optiscaler_ini(state);
                    }
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 105,
                        control_kind: 0,
                        value: state.char_structure,
                    });
                    changed = true;
                }

                // 7. Overall Intensity (cy = 278.0)
                if fy >= 268.0 && fy <= 296.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                    state.active_slider = Some("overall_intensity".to_string());
                    let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                    state.overall_intensity = (ratio * 2.0 * 100.0).round() / 100.0;
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 106,
                        control_kind: 0,
                        value: state.overall_intensity,
                    });
                    changed = true;
                }

                // 8. Local Tone Intensity (cy = 306.0)
                if fy >= 296.0 && fy <= 324.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                    state.active_slider = Some("local_tone".to_string());
                    let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                    state.local_tone = (ratio * 2.0 * 100.0).round() / 100.0;
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 107,
                        control_kind: 0,
                        value: state.local_tone,
                    });
                    changed = true;
                }

                // 9. Diffuse White (cy = 334.0)
                if fy >= 324.0 && fy <= 352.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                    state.active_slider = Some("diffuse_white".to_string());
                    let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                    state.diffuse_white = (80.0 + ratio * 420.0).round();
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 108,
                        control_kind: 0,
                        value: state.diffuse_white,
                    });
                    changed = true;
                }

                // 10. Motion Scale X (cy = 362.0)
                if fy >= 352.0 && fy <= 380.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                    state.active_slider = Some("motion_x".to_string());
                    let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                    state.motion_x = (-2.0 + ratio * 4.0 * 100.0).round() / 100.0;
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 109,
                        control_kind: 0,
                        value: state.motion_x,
                    });
                    changed = true;
                }

                // 11. Motion Scale Y (cy = 390.0)
                if fy >= 380.0 && fy <= 408.0 && fx >= track_x && fx <= track_x + track_w + num_w + 8.0 {
                    state.active_slider = Some("motion_y".to_string());
                    let ratio = ((fx - track_x) / track_w).clamp(0.0, 1.0);
                    state.motion_y = (-2.0 + ratio * 4.0 * 100.0).round() / 100.0;
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 110,
                        control_kind: 0,
                        value: state.motion_y,
                    });
                    changed = true;
                }

                // 12. NR UI Correction (cy = 420.0)
                if fy >= 408.0 && fy <= 436.0 && fx >= content_x && fx <= content_x + content_w {
                    state.nr_ui_correction = !state.nr_ui_correction;
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 111,
                        control_kind: 1,
                        value: if state.nr_ui_correction { 1.0 } else { 0.0 },
                    });
                    changed = true;
                }

                // 13. Enable Upscaling (cy = 444.0)
                if fy >= 436.0 && fy <= 464.0 && fx >= content_x && fx <= content_x + content_w {
                    state.enable_upscaling = !state.enable_upscaling;
                    state.pending_command = Some(OverlayCommandPacket {
                        magic: COMMAND_MAGIC,
                        version: 1,
                        epoch: ep,
                        control_id: 112,
                        control_kind: 1,
                        value: if state.enable_upscaling { 1.0 } else { 0.0 },
                    });
                    changed = true;
                }
            }
        }
        3 => {
            // Mouse Up
            state.mouse_down = false;
            state.active_slider = None;
        }
        4 => {
            // Mouse Wheel
            let wheel_delta = value as f32;
            if state.mode == 0 {
                let max_scroll = 140.0;
                let next = (state.more_scroll - wheel_delta * 0.25).clamp(0.0, max_scroll);
                if (next - state.more_scroll).abs() > 0.5 {
                    state.more_scroll = next;
                    changed = true;
                }
            } else {
                let max_scroll = 200.0;
                let next = (state.live_scroll - wheel_delta * 0.25).clamp(0.0, max_scroll);
                if (next - state.live_scroll).abs() > 0.5 {
                    state.live_scroll = next;
                    changed = true;
                }
            }
        }
        _ => {}
    }

    changed
}

fn get_appdata_dir() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        PathBuf::from(appdata).join("dlss-5-studio")
    } else {
        PathBuf::from(".").join(".appdata")
    }
}

fn log_message(_msg: &str) {}

#[derive(Default, Deserialize)]
struct OverlayPreferencesConfig {
    #[serde(default)]
    theme: String,
    #[serde(default)]
    hotkey: u32,
    #[serde(default)]
    enabled: bool,
}

pub fn get_active_overlay_theme() -> String {
    let pref_file = get_appdata_dir().join("overlay-preferences.json");
    if let Ok(c) = fs::read_to_string(pref_file) {
        if let Ok(cfg) = serde_json::from_str::<OverlayPreferencesConfig>(&c) {
            return cfg.theme;
        }
    }
    "emerald".to_string()
}

pub fn get_overlay_hotkey() -> String {
    let pref_file = get_appdata_dir().join("overlay-preferences.json");
    if let Ok(c) = fs::read_to_string(pref_file) {
        if let Ok(cfg) = serde_json::from_str::<OverlayPreferencesConfig>(&c) {
            return match cfg.hotkey {
                112 => "F1", 113 => "F2", 114 => "F3", 115 => "F4", 116 => "F5",
                117 => "F6", 118 => "F7", 119 => "F8", 120 => "F9", 121 => "F10",
                122 => "F11", 123 => "F12", _ => "F8",
            }.to_string();
        }
    }
    "F8".to_string()
}

#[derive(Default, Deserialize)]
struct MinimalGameEntry {
    #[serde(default)]
    dir: String,
}

#[derive(Default, Deserialize)]
struct MinimalLibraryConfig {
    #[serde(default)]
    cached_games: Vec<MinimalGameEntry>,
}

fn get_cached_game_dirs() -> Vec<PathBuf> {
    let p = get_appdata_dir().join("library.json");
    if let Ok(c) = fs::read_to_string(p) {
        if let Ok(cfg) = serde_json::from_str::<MinimalLibraryConfig>(&c) {
            return cfg.cached_games.into_iter().map(|g| PathBuf::from(g.dir)).collect();
        }
    }
    Vec::new()
}

/// Starts the in-game overlay bridge named pipe server in a background thread.
pub fn start_overlay_bridge() {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    thread::spawn(|| {
        let appdata = get_appdata_dir();
        let endpoint_file = appdata.join("overlay-bridge.endpoint");

        let mut token = String::new();
        if endpoint_file.exists() {
            if let Ok(existing) = fs::read_to_string(&endpoint_file) {
                let trimmed = existing.trim().to_string();
                if trimmed.len() == 32 {
                    token = trimmed;
                }
            }
        }

        if token.is_empty() {
            use std::time::SystemTime;
            let seed = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            token = format!("{:032x}", seed);
            let _ = fs::write(&endpoint_file, &token);
        }

        let overlay_hotkey = get_overlay_hotkey();
        let overlay_theme = get_active_overlay_theme();
        let pipe_name = format!(r"\\.\pipe\dlss5-swapper-overlay-{}", token);
        let mut pipe_wide: Vec<u16> = pipe_name.encode_utf16().collect();
        pipe_wide.push(0);

        log_message(&format!("@{{log_overlay_initialized|{}|{}}}", &overlay_hotkey, &pipe_name[..40.min(pipe_name.len())]));

        let mut ui_state = OverlayUiState::default();

        match overlay_theme.as_str() {
            "blue" | "azure" => {
                ui_state.accent_color = 0xFF4A_A8EE;
                ui_state.soft_color = 0x254A_A8EE;
                ui_state.back_color = 0xFF0D_1724;
                ui_state.bright_color = 0xFF91_D1FF;
            }
            "purple" | "amethyst" => {
                ui_state.accent_color = 0xFFB4_5DEA;
                ui_state.soft_color = 0x25B4_5DEA;
                ui_state.back_color = 0xFF1B_1127;
                ui_state.bright_color = 0xFFDD_B0FF;
            }
            "green" | "emerald" => {
                ui_state.accent_color = 0xFF8B_C400;
                ui_state.soft_color = 0x258B_C400;
                ui_state.back_color = 0xFF11_1A10;
                ui_state.bright_color = 0xFFC2_EC66;
            }
            custom_id => {
                if let Some((r, g, b)) = parse_hex_color(custom_id) {
                    ui_state.accent_color = 0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                    ui_state.soft_color = 0x2500_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
                    ui_state.back_color = 0xFF11_1A10;
                    ui_state.bright_color = 0xFFC2_EC66;
                }
            }
        }

        // 24-byte hello reply: 15 (0x0F) enables live_peer, nr_peer, and feed_peer
        let mut hello_reply = [0u8; 24];
        hello_reply[0..4].copy_from_slice(&HELLO_REPLY_MAGIC.to_le_bytes());
        hello_reply[4..8].copy_from_slice(&1u32.to_le_bytes());
        hello_reply[8..12].copy_from_slice(&15u32.to_le_bytes());

        while RUNNING.load(Ordering::SeqCst) {
            unsafe {
                let h_pipe = CreateNamedPipeW(
                    pipe_wide.as_ptr(),
                    0x0000_0003, // PIPE_ACCESS_DUPLEX
                    0x0000_0000, // PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT
                    1,           // 1 instance
                    PANEL_WIDTH * PANEL_HEIGHT * 4 + 1024,
                    65536,
                    0,
                    std::ptr::null(),
                );

                if h_pipe == -1 || h_pipe == 0 {
                    thread::sleep(Duration::from_millis(500));
                    continue;
                }

                let connected = ConnectNamedPipe(h_pipe, std::ptr::null_mut());
                if connected != 0 || std::io::Error::last_os_error().raw_os_error() == Some(535) {
                    log_message("@{log_overlay_connected}");

                    // Detect client PID and resolve game directory for MFG / Pre-SR config detection
                    let mut client_pid: u32 = 0;
                    if GetNamedPipeClientProcessId(h_pipe, &mut client_pid) != 0 && client_pid > 0 {
                        log_message(&format!("@{{log_overlay_client_pid|{}}}", client_pid));
                        let mut game_dir: Option<PathBuf> = None;

                        let h_proc = OpenProcess(0x1000 /* PROCESS_QUERY_LIMITED_INFORMATION */, 0, client_pid);
                        if h_proc != 0 {
                            let mut exe_buf = [0u16; 1024];
                            let mut size = exe_buf.len() as u32;
                            if QueryFullProcessImageNameW(h_proc, 0, exe_buf.as_mut_ptr(), &mut size) != 0 && size > 0 {
                                let exe_str = String::from_utf16_lossy(&exe_buf[..size as usize]);
                                let exe_path = PathBuf::from(&exe_str);
                                if let Some(parent) = exe_path.parent() {
                                    game_dir = Some(parent.to_path_buf());
                                }
                            }
                            CloseHandle(h_proc);
                        }

                        if game_dir.is_none() {
                            for d in get_cached_game_dirs() {
                                if d.join("ReShade.ini").exists() || d.join("renodx-mfgunlock.addon64").exists() || d.join("OptiScaler.ini").exists() {
                                    game_dir = Some(d);
                                    break;
                                }
                            }
                        }

                        if let Some(dir) = game_dir {
                            let (has_mfg, mfg_en, mfg_mult, has_presr, presr_en, presr_p) = detect_game_mods(&dir);
                            ui_state.has_mfg = has_mfg;
                            ui_state.mfg_enabled = mfg_en;
                            ui_state.mfg_multiplier = mfg_mult;
                            ui_state.has_presr = has_presr;
                            ui_state.presr_enabled = presr_en;
                            ui_state.presr_passes = presr_p;
                            if has_presr {
                                ui_state.dlss_on = presr_en;
                                ui_state.dlss_neural_rendering = presr_en;
                            }
                            ui_state.active_game_dir = Some(dir);
                        }
                    }

                    let mut in_buf = [0u8; 65536];
                    let mut bytes_read: u32 = 0;
                    let mut sequence: u32 = 1;
                    let mut last_packet_time = std::time::Instant::now();
                    let mut fps_samples: Vec<f32> = Vec::with_capacity(20);
                    let mut last_fps_update = std::time::Instant::now();

                    // Handshake
                    if ReadFile(h_pipe, in_buf.as_mut_ptr(), 20, &mut bytes_read, std::ptr::null_mut()) != 0 && bytes_read >= 20 {
                        let magic = u32::from_le_bytes([in_buf[0], in_buf[1], in_buf[2], in_buf[3]]);
                        if magic == INPUT_MAGIC {
                            let mut written: u32 = 0;
                            let _ = WriteFile(h_pipe, hello_reply.as_ptr(), 24, &mut written, std::ptr::null_mut());

                            let frame = render_overlay_surface(&ui_state, sequence);
                            let _ = WriteFile(h_pipe, frame.as_ptr(), frame.len() as u32, &mut written, std::ptr::null_mut());
                        }
                    }

                    // Event loop
                    while RUNNING.load(Ordering::SeqCst) {
                        let mut read_bytes: u32 = 0;
                        if ReadFile(h_pipe, in_buf.as_mut_ptr(), in_buf.len() as u32, &mut read_bytes, std::ptr::null_mut()) == 0 || read_bytes == 0 {
                            break;
                        }

                        // Update real-time frame timing
                        let now = std::time::Instant::now();
                        let delta_secs = now.duration_since(last_packet_time).as_secs_f32();
                        last_packet_time = now;

                        if delta_secs > 0.0005 && delta_secs < 0.5 {
                            let sample_fps = 1.0 / delta_secs;
                            fps_samples.push(sample_fps);
                            if fps_samples.len() > 15 {
                                fps_samples.remove(0);
                            }
                        }

                        let mut offset = 0;
                        let mut state_changed = false;

                        if now.duration_since(last_fps_update).as_millis() >= 250 && !fps_samples.is_empty() {
                            let avg_fps: f32 = fps_samples.iter().sum::<f32>() / (fps_samples.len() as f32);
                            let avg_ms = if avg_fps > 0.0 { 1000.0 / avg_fps } else { 0.0 };
                            if (ui_state.fps - avg_fps).abs() >= 1.0 {
                                ui_state.fps = avg_fps.round();
                                ui_state.frametime_ms = (avg_ms * 10.0).round() / 10.0;
                                state_changed = true;
                            }
                            last_fps_update = now;
                        }

                        while offset + 8 <= read_bytes as usize {
                            let magic = u32::from_le_bytes([in_buf[offset], in_buf[offset + 1], in_buf[offset + 2], in_buf[offset + 3]]);

                            if magic == STATE_JSON_MAGIC {
                                let json_len = u32::from_le_bytes([in_buf[offset + 4], in_buf[offset + 5], in_buf[offset + 6], in_buf[offset + 7]]) as usize;
                                offset += 8;
                                if offset + json_len <= read_bytes as usize {
                                    if let Ok(json_str) = std::str::from_utf8(&in_buf[offset..offset + json_len]) {
                                        if let Ok(status) = serde_json::from_str::<AddonStatusJson>(json_str) {
                                            ui_state.current_epoch = status.epoch;
                                            ui_state.nr_available = status.nr_available;
                                            ui_state.nr_enabled = status.nr_enabled;
                                            ui_state.reshade_fx_enabled = status.effects;

                                            // 1. One-time RenoDX bridge wakeup handshake (ID 200) matching original JS app
                                            if !status.nr_tools.is_empty() && !status.nr_enabled && ui_state.last_enabled_epoch != status.epoch {
                                                ui_state.last_enabled_epoch = status.epoch;
                                                let wakeup_cmd = OverlayCommandPacket {
                                                    magic: COMMAND_MAGIC,
                                                    version: 1,
                                                    epoch: status.epoch,
                                                    control_id: 200,
                                                    control_kind: 1,
                                                    value: 1.0,
                                                };
                                                send_command_packet(h_pipe, &wakeup_cmd);
                                            }

                                            // 2. Two-way state sync when user is NOT actively dragging
                                            if ui_state.active_slider.is_none() && !ui_state.mouse_down {
                                                ui_state.available_tools.clear();
                                                for tool in &status.nr_tools {
                                                    if tool.available {
                                                        ui_state.available_tools.insert(tool.id);
                                                    }
                                                    match tool.id {
                                                        101 => ui_state.structure_intensity = tool.value,
                                                        102 => ui_state.tone_intensity = tool.value,
                                                        103 => {
                                                            ui_state.dlss_on = tool.value != 0.0;
                                                            ui_state.dlss_neural_rendering = ui_state.dlss_on;
                                                        }
                                                        104 => ui_state.character_mask = tool.value != 0.0,
                                                        105 => ui_state.char_structure = tool.value,
                                                        106 => ui_state.overall_intensity = tool.value,
                                                        107 => ui_state.local_tone = tool.value,
                                                        108 => ui_state.diffuse_white = tool.value,
                                                        109 => ui_state.motion_x = tool.value,
                                                        110 => ui_state.motion_y = tool.value,
                                                        111 => ui_state.nr_ui_correction = tool.value != 0.0,
                                                        112 => ui_state.enable_upscaling = tool.value != 0.0,
                                                        113 => ui_state.nr_preset = tool.value as usize,
                                                        114 => ui_state.nr_style = (tool.value as usize).min(2),
                                                        115 => ui_state.depth_convention = tool.value as usize,
                                                        _ => {}
                                                    }
                                                }
                                                state_changed = true;
                                            }
                                        }
                                    }
                                    offset += json_len;
                                }
                                continue;
                            }

                            if magic == INPUT_MAGIC && offset + 20 <= read_bytes as usize {
                                let action = u32::from_le_bytes([in_buf[offset + 4], in_buf[offset + 5], in_buf[offset + 6], in_buf[offset + 7]]);
                                let x = i32::from_le_bytes([in_buf[offset + 8], in_buf[offset + 9], in_buf[offset + 10], in_buf[offset + 11]]);
                                let y = i32::from_le_bytes([in_buf[offset + 12], in_buf[offset + 13], in_buf[offset + 14], in_buf[offset + 15]]);
                                let value = i32::from_le_bytes([in_buf[offset + 16], in_buf[offset + 17], in_buf[offset + 18], in_buf[offset + 19]]);

                                if handle_input_packet(&mut ui_state, action, x, y, value) {
                                    state_changed = true;

                                    if let Some(cmd) = ui_state.pending_command.take() {
                                        send_command_packet(h_pipe, &cmd);
                                    }
                                    for cmd in ui_state.pending_commands.drain(..) {
                                        send_command_packet(h_pipe, &cmd);
                                    }
                                }
                                offset += 20;
                            } else {
                                offset += 4;
                            }
                        }

                        if state_changed {
                            sequence += 1;
                            let frame = render_overlay_surface(&ui_state, sequence);
                            let mut written: u32 = 0;
                            let _ = WriteFile(h_pipe, frame.as_ptr(), frame.len() as u32, &mut written, std::ptr::null_mut());
                        }
                    }

                    log_message("@{log_overlay_disconnected}");
                    DisconnectNamedPipe(h_pipe);
                }

                CloseHandle(h_pipe);
            }
        }
    });
}

/// Stops the in-game overlay bridge
pub fn stop_overlay_bridge() {
    RUNNING.store(false, Ordering::SeqCst);
}

fn parse_hex_color(hex: &str) -> Option<(u8, u8, u8)> {
    let clean = hex.trim().trim_start_matches('#');
    if clean.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&clean[0..2], 16).ok()?;
    let g = u8::from_str_radix(&clean[2..4], 16).ok()?;
    let b = u8::from_str_radix(&clean[4..6], 16).ok()?;
    Some((r, g, b))
}




