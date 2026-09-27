//! Pure Rust Win32 XInput Gamepad Engine for DLSS Studio Big Picture Mode.
//!
//! Dynamically loads XInput (xinput1_4.dll -> xinput1_3.dll -> xinput9_1_0.dll)
//! and provides adaptive 60Hz polling, thumbstick deadzones, hold-to-repeat
//! debouncing, and seamless integration with Vibepollo / Moonlight virtual controllers.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;
use windows::core::{s, w, PCWSTR};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};

// XInput constants & button bitmasks
pub const XINPUT_GAMEPAD_DPAD_UP: u16 = 0x0001;
pub const XINPUT_GAMEPAD_DPAD_DOWN: u16 = 0x0002;
pub const XINPUT_GAMEPAD_DPAD_LEFT: u16 = 0x0004;
pub const XINPUT_GAMEPAD_DPAD_RIGHT: u16 = 0x0008;
pub const XINPUT_GAMEPAD_START: u16 = 0x0010;
pub const XINPUT_GAMEPAD_BACK: u16 = 0x0020;
pub const XINPUT_GAMEPAD_LEFT_THUMB: u16 = 0x0040;
pub const XINPUT_GAMEPAD_RIGHT_THUMB: u16 = 0x0080;
pub const XINPUT_GAMEPAD_LEFT_SHOULDER: u16 = 0x0100;
pub const XINPUT_GAMEPAD_RIGHT_SHOULDER: u16 = 0x0200;
pub const XINPUT_GAMEPAD_GUIDE: u16 = 0x0400; // Ordinal 100 extension
pub const XINPUT_GAMEPAD_A: u16 = 0x1000;
pub const XINPUT_GAMEPAD_B: u16 = 0x2000;
pub const XINPUT_GAMEPAD_X: u16 = 0x4000;
pub const XINPUT_GAMEPAD_Y: u16 = 0x8000;

pub const XINPUT_GAMEPAD_LEFT_THUMB_DEADZONE: i16 = 7849;
pub const STICK_THRESHOLD: i16 = 16000;

const ERROR_SUCCESS: u32 = 0;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct XInputGamepad {
    pub w_buttons: u16,
    pub b_left_trigger: u8,
    pub b_right_trigger: u8,
    pub s_thumb_lx: i16,
    pub s_thumb_ly: i16,
    pub s_thumb_rx: i16,
    pub s_thumb_ry: i16,
}

#[repr(C)]
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct XInputState {
    pub dw_packet_number: u32,
    pub gamepad: XInputGamepad,
}

type FnXInputGetState = unsafe extern "system" fn(dw_user_index: u32, p_state: *mut XInputState) -> u32;

/// Hardware brand and generation of the connected controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerKind {
    XboxSeriesX,
    XboxOne,
    PlayStation5,
    PlayStation4,
    NintendoSwitch,
    GenericXInput,
    Disconnected,
}

impl ControllerKind {
    pub fn badge_text(&self) -> &'static str {
        match self {
            ControllerKind::XboxSeriesX => "🎮 Xbox Series X|S",
            ControllerKind::XboxOne => "🎮 Xbox One",
            ControllerKind::PlayStation5 => "🎮 DualSense (PS5)",
            ControllerKind::PlayStation4 => "🎮 DualShock 4",
            ControllerKind::NintendoSwitch => "🎮 Switch Pro",
            ControllerKind::GenericXInput => "🎮 Controller Connected",
            ControllerKind::Disconnected => "🎮 No Controller",
        }
    }

    /// Primary button (Play / Launch) glyph: (class, symbol)
    pub fn play_glyph(&self) -> (&'static str, &'static str) {
        match self {
            ControllerKind::PlayStation5 | ControllerKind::PlayStation4 => ("btn-glyph ps-cross", "✕"),
            ControllerKind::NintendoSwitch => ("btn-glyph n-b", "B"),
            _ => ("btn-glyph xb-a", "A"),
        }
    }

    /// Secondary button (Patch Options) glyph: (class, symbol)
    pub fn patch_glyph(&self) -> (&'static str, &'static str) {
        match self {
            ControllerKind::PlayStation5 | ControllerKind::PlayStation4 => ("btn-glyph ps-square", "▢"),
            ControllerKind::NintendoSwitch => ("btn-glyph n-y", "Y"),
            _ => ("btn-glyph xb-x", "X"),
        }
    }

    /// Tertiary button (Filter Store) glyph: (class, symbol)
    pub fn filter_glyph(&self) -> (&'static str, &'static str) {
        match self {
            ControllerKind::PlayStation5 | ControllerKind::PlayStation4 => ("btn-glyph ps-triangle", "△"),
            ControllerKind::NintendoSwitch => ("btn-glyph n-x", "X"),
            _ => ("btn-glyph xb-y", "Y"),
        }
    }

    /// Back button glyph: (class, symbol)
    pub fn back_glyph(&self) -> (&'static str, &'static str) {
        match self {
            ControllerKind::PlayStation5 | ControllerKind::PlayStation4 => ("btn-glyph ps-circle", "○"),
            ControllerKind::NintendoSwitch => ("btn-glyph n-a", "A"),
            _ => ("btn-glyph xb-b", "B"),
        }
    }

    /// Shoulder bumpers: (left, right)
    pub fn bumper_labels(&self) -> (&'static str, &'static str) {
        match self {
            ControllerKind::PlayStation5 | ControllerKind::PlayStation4 => ("L1", "R1"),
            ControllerKind::NintendoSwitch => ("L", "R"),
            _ => ("LB", "RB"),
        }
    }

    /// Menu / Options button (Game Options & Presets) glyph: (class, symbol)
    pub fn menu_glyph(&self) -> (&'static str, &'static str) {
        match self {
            ControllerKind::PlayStation5 | ControllerKind::PlayStation4 => ("btn-glyph ps-options", "OPTIONS"),
            ControllerKind::NintendoSwitch => ("btn-glyph n-plus", "+"),
            _ => ("btn-glyph xb-menu", "☰"),
        }
    }
}

/// High-level navigation actions emitted by the Gamepad engine for the 10-foot UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamepadNavAction {
    NavigateLeft,
    NavigateRight,
    NavigateUp,
    NavigateDown,
    PageLeft,        // LB
    PageRight,       // RB
    PrimaryAction,   // A / Cross (Play / Launch)
    SecondaryAction, // X / Square (Patch Options)
    TertiaryAction,  // Y / Triangle (Filter store)
    Back,            // B / Circle (Exit / Back)
    Start,           // Start button
    Guide,           // Xbox Guide / PS Home
    ControllerChanged(ControllerKind),
}

/// Classifies a controller by USB Vendor ID (VID) and Product ID (PID).
pub fn classify_controller_vid_pid(vid: u32, pid: u32) -> ControllerKind {
    match vid {
        // Sony Interactive Entertainment
        0x054C => match pid {
            0x0CE6 | 0x0DF2 => ControllerKind::PlayStation5, // DualSense & DualSense Edge
            0x05C4 | 0x09CC | 0x0BA0 => ControllerKind::PlayStation4, // DualShock 4 & Wireless Adapter
            _ => ControllerKind::PlayStation5,
        },
        // Microsoft Corporation
        0x045E => match pid {
            0x0B12 | 0x0B13 | 0x0B20 | 0x0B21 | 0x0B22 => ControllerKind::XboxSeriesX,
            0x02D1 | 0x02DD | 0x02E3 | 0x02EA | 0x0B00 | 0x0B05 => ControllerKind::XboxOne,
            0x028E => ControllerKind::GenericXInput, // Xbox 360 / Virtual ViGEm / Vibepollo
            _ => ControllerKind::XboxSeriesX,
        },
        // Nintendo
        0x057E => match pid {
            0x2009 | 0x2006 | 0x2007 => ControllerKind::NintendoSwitch, // Switch Pro & Joy-Cons
            _ => ControllerKind::NintendoSwitch,
        },
        _ => ControllerKind::GenericXInput,
    }
}

/// Pure Rust Win32 Raw Input controller scanner detecting connected device hardware.
pub fn detect_controller_kind() -> ControllerKind {
    unsafe {
        use windows::Win32::UI::Input::*;
        let mut num_devices = 0u32;
        let res = GetRawInputDeviceList(
            None,
            &mut num_devices,
            std::mem::size_of::<RAWINPUTDEVICELIST>() as u32,
        );
        if res == u32::MAX || num_devices == 0 {
            return ControllerKind::GenericXInput;
        }

        let mut devices = vec![RAWINPUTDEVICELIST::default(); num_devices as usize];
        let count = GetRawInputDeviceList(
            Some(devices.as_mut_ptr()),
            &mut num_devices,
            std::mem::size_of::<RAWINPUTDEVICELIST>() as u32,
        );
        if count == u32::MAX {
            return ControllerKind::GenericXInput;
        }

        let mut found_controller = None;

        for dev in devices.iter().take(count as usize) {
            let mut info: RID_DEVICE_INFO = std::mem::zeroed();
            info.cbSize = std::mem::size_of::<RID_DEVICE_INFO>() as u32;
            let mut size = info.cbSize;

            let info_res = GetRawInputDeviceInfoW(
                dev.hDevice,
                RIDI_DEVICEINFO,
                Some(&mut info as *mut _ as *mut _),
                &mut size,
            );
            if info_res != u32::MAX && info.dwType == RIM_TYPEHID {
                let hid = info.Anonymous.hid;
                let is_gamepad_usage = hid.usUsagePage == 0x01 && (hid.usUsage == 0x04 || hid.usUsage == 0x05);
                let is_known_vendor = hid.dwVendorId == 0x054C || hid.dwVendorId == 0x045E || hid.dwVendorId == 0x057E;

                if is_gamepad_usage || is_known_vendor {
                    let kind = classify_controller_vid_pid(hid.dwVendorId, hid.dwProductId);
                    if kind != ControllerKind::GenericXInput {
                        return kind;
                    } else if is_gamepad_usage {
                        found_controller = Some(ControllerKind::GenericXInput);
                    }
                }
            }
        }

        found_controller.unwrap_or(ControllerKind::GenericXInput)
    }
}

/// Dynamic XInput loader maintaining zero static dependencies on external DLLs.
pub struct DynamicXInput {
    _module: HMODULE,
    get_state: FnXInputGetState,
    pub is_ordinal_100: bool,
}

// Safety: HMODULE and function pointers in Win32 are thread-safe for immutable calls.
unsafe impl Send for DynamicXInput {}
unsafe impl Sync for DynamicXInput {}

impl DynamicXInput {
    pub fn load() -> Option<Self> {
        let candidates: &[PCWSTR] = &[
            w!("xinput1_4.dll"),
            w!("xinput1_3.dll"),
            w!("xinput9_1_0.dll"),
        ];

        for &dll in candidates {
            if let Ok(module) = unsafe { LoadLibraryW(dll) } {
                if module.is_invalid() {
                    continue;
                }

                // First attempt Ordinal 100 (secret Guide button support)
                let ordinal_100_proc = unsafe { GetProcAddress(module, windows::core::PCSTR(100 as *const u8)) };
                if let Some(proc) = ordinal_100_proc {
                    let get_state: FnXInputGetState = unsafe { std::mem::transmute(proc) };
                    return Some(Self {
                        _module: module,
                        get_state,
                        is_ordinal_100: true,
                    });
                }

                // Fallback to standard "XInputGetState"
                let standard_proc = unsafe { GetProcAddress(module, s!("XInputGetState")) };
                if let Some(proc) = standard_proc {
                    let get_state: FnXInputGetState = unsafe { std::mem::transmute(proc) };
                    return Some(Self {
                        _module: module,
                        get_state,
                        is_ordinal_100: false,
                    });
                }
            }
        }

        None
    }

    pub fn get_state(&self, user_index: u32) -> Result<XInputState, u32> {
        let mut state = XInputState::default();
        let res = unsafe { (self.get_state)(user_index, &mut state) };
        if res == ERROR_SUCCESS {
            Ok(state)
        } else {
            Err(res)
        }
    }
}

/// Helper to apply circular deadzone and directional thresholding to analog stick.
pub fn process_stick(lx: i16, ly: i16) -> (bool, bool, bool, bool) {
    let mag_sq = (lx as i32).pow(2) + (ly as i32).pow(2);
    let deadzone_sq = (XINPUT_GAMEPAD_LEFT_THUMB_DEADZONE as i32).pow(2);

    if mag_sq < deadzone_sq {
        return (false, false, false, false);
    }

    let abs_x = (lx as i32).abs();
    let abs_y = (ly as i32).abs();

    if abs_y >= abs_x {
        // Vertical deflection dominates
        let up = ly > STICK_THRESHOLD;
        let down = ly < -STICK_THRESHOLD;
        (false, false, up, down)
    } else {
        // Horizontal deflection dominates
        let left = lx < -STICK_THRESHOLD;
        let right = lx > STICK_THRESHOLD;
        (left, right, false, false)
    }
}

/// State tracking for hold-to-repeat and press debounce.
#[doc(hidden)]
pub struct RepeatTracker {
    pub last_pressed: Option<GamepadNavAction>,
    pub first_trigger_time: Instant,
    pub last_repeat_time: Instant,
}

impl RepeatTracker {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            last_pressed: None,
            first_trigger_time: now,
            last_repeat_time: now,
        }
    }

    pub fn should_trigger(&mut self, action: Option<GamepadNavAction>) -> Option<GamepadNavAction> {
        let now = Instant::now();

        match (self.last_pressed, action) {
            (Some(prev), Some(curr)) if prev == curr => {
                // Repeating the same directional action
                let initial_delay = Duration::from_millis(300);
                let repeat_rate = Duration::from_millis(90);

                if now.duration_since(self.first_trigger_time) >= initial_delay {
                    if now.duration_since(self.last_repeat_time) >= repeat_rate {
                        self.last_repeat_time = now;
                        return Some(curr);
                    }
                }
                None
            }
            (_, Some(curr)) => {
                // New action pressed
                self.last_pressed = Some(curr);
                self.first_trigger_time = now;
                self.last_repeat_time = now;
                Some(curr)
            }
            (_, None) => {
                // Released
                self.last_pressed = None;
                None
            }
        }
    }
}

type FnTimeBeginPeriod = unsafe extern "system" fn(u32) -> u32;
type FnTimeEndPeriod = unsafe extern "system" fn(u32) -> u32;

/// RAII guard that requests a 1ms system timer interrupt resolution on Windows,
/// preventing the default ~15.625ms quantization during `thread::sleep`
/// and ensuring sub-frame latency for Moonlight / ViGEmBus controller events.
pub struct HighPrecisionTimerGuard {
    end_period_fn: Option<FnTimeEndPeriod>,
}

impl HighPrecisionTimerGuard {
    pub fn new() -> Self {
        #[cfg(windows)]
        unsafe {
            let winmm = windows::Win32::System::LibraryLoader::LoadLibraryW(windows::core::w!("winmm.dll"));
            if let Ok(module) = winmm {
                let begin_proc = windows::Win32::System::LibraryLoader::GetProcAddress(
                    module,
                    windows::core::s!("timeBeginPeriod"),
                );
                let end_proc = windows::Win32::System::LibraryLoader::GetProcAddress(
                    module,
                    windows::core::s!("timeEndPeriod"),
                );
                if let (Some(begin_raw), Some(end_raw)) = (begin_proc, end_proc) {
                    let begin_fn: FnTimeBeginPeriod = std::mem::transmute(begin_raw);
                    let end_fn: FnTimeEndPeriod = std::mem::transmute(end_raw);
                    let _ = begin_fn(1);
                    return Self {
                        end_period_fn: Some(end_fn),
                    };
                }
            }
        }
        Self { end_period_fn: None }
    }
}

impl Drop for HighPrecisionTimerGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        if let Some(end_fn) = self.end_period_fn {
            unsafe {
                let _ = end_fn(1);
            }
        }
    }
}

/// Starts the pure Rust XInput polling loop in a Tokio task.
///
/// Designed for sub-millisecond latency when Vibepollo / Moonlight virtual controller
/// or native Xbox/PlayStation XInput controllers are connected.
pub fn start_gamepad_listener(
    action_tx: UnboundedSender<GamepadNavAction>,
    cancel_flag: Arc<AtomicBool>,
) -> tokio::task::JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let _timer_guard = HighPrecisionTimerGuard::new();
        let xinput = match DynamicXInput::load() {
            Some(x) => x,
            None => {
                eprintln!("[BigPicture::Gamepad] Failed to load any XInput DLL.");
                return;
            }
        };

        let mut tracker = RepeatTracker::new();
        let mut prev_buttons: u16 = 0;
        let mut last_detected_kind = detect_controller_kind();
        let _ = action_tx.send(GamepadNavAction::ControllerChanged(last_detected_kind));
        let mut poll_count: u32 = 0;

        while !cancel_flag.load(Ordering::Relaxed) {
            // Find active controller across player slots 0..4
            let active_state = (0..4).find_map(|slot| xinput.get_state(slot).ok());
            match active_state {
                Some(state) => {
                    poll_count = poll_count.wrapping_add(1);
                    if poll_count % 60 == 0 {
                        let current_kind = detect_controller_kind();
                        if current_kind != last_detected_kind {
                            last_detected_kind = current_kind;
                            let _ = action_tx.send(GamepadNavAction::ControllerChanged(current_kind));
                        }
                    }

                    let buttons = state.gamepad.w_buttons;
                    let (s_left, s_right, s_up, s_down) =
                        process_stick(state.gamepad.s_thumb_lx, state.gamepad.s_thumb_ly);

                    let d_left = (buttons & XINPUT_GAMEPAD_DPAD_LEFT) != 0 || s_left;
                    let d_right = (buttons & XINPUT_GAMEPAD_DPAD_RIGHT) != 0 || s_right;
                    let d_up = (buttons & XINPUT_GAMEPAD_DPAD_UP) != 0 || s_up;
                    let d_down = (buttons & XINPUT_GAMEPAD_DPAD_DOWN) != 0 || s_down;

                    // Directional repeat action
                    let dir_action = if d_left {
                        Some(GamepadNavAction::NavigateLeft)
                    } else if d_right {
                        Some(GamepadNavAction::NavigateRight)
                    } else if d_up {
                        Some(GamepadNavAction::NavigateUp)
                    } else if d_down {
                        Some(GamepadNavAction::NavigateDown)
                    } else {
                        None
                    };

                    if let Some(act) = tracker.should_trigger(dir_action) {
                        let _ = action_tx.send(act);
                    }

                    // Discrete button presses (edge-triggered)
                    let newly_pressed = buttons & !prev_buttons;

                    if newly_pressed & XINPUT_GAMEPAD_A != 0 {
                        let _ = action_tx.send(GamepadNavAction::PrimaryAction);
                    }
                    if newly_pressed & XINPUT_GAMEPAD_X != 0 {
                        let _ = action_tx.send(GamepadNavAction::SecondaryAction);
                    }
                    if newly_pressed & XINPUT_GAMEPAD_Y != 0 {
                        let _ = action_tx.send(GamepadNavAction::TertiaryAction);
                    }
                    if newly_pressed & XINPUT_GAMEPAD_B != 0 {
                        let _ = action_tx.send(GamepadNavAction::Back);
                    }
                    if newly_pressed & XINPUT_GAMEPAD_START != 0 {
                        let _ = action_tx.send(GamepadNavAction::Start);
                    }
                    if newly_pressed & XINPUT_GAMEPAD_LEFT_SHOULDER != 0 {
                        let _ = action_tx.send(GamepadNavAction::PageLeft);
                    }
                    if newly_pressed & XINPUT_GAMEPAD_RIGHT_SHOULDER != 0 {
                        let _ = action_tx.send(GamepadNavAction::PageRight);
                    }
                    if newly_pressed & XINPUT_GAMEPAD_GUIDE != 0 {
                        let _ = action_tx.send(GamepadNavAction::Guide);
                    }

                    prev_buttons = buttons;

                    // 125Hz sampling interval (8ms) with 1ms timer resolution for sub-frame couch controls
                    std::thread::sleep(Duration::from_millis(8));
                }
                None => {
                    // Gamepad not connected yet (e.g. Moonlight stream not yet initialized).
                    // Sleep longer to conserve CPU power.
                    if last_detected_kind != ControllerKind::Disconnected {
                        last_detected_kind = ControllerKind::Disconnected;
                        let _ = action_tx.send(GamepadNavAction::ControllerChanged(ControllerKind::Disconnected));
                    }
                    prev_buttons = 0;
                    tracker.last_pressed = None;
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }
    })
}
