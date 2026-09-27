//! Pure Rust TV & Virtual Display Detection and Sleep Inhibitor for Big Picture Mode.
//!
//! Enumerates connected display adapters to identify physical TVs and virtual
//! displays created by Vibepollo / Apollo / Sunshine (IddCx / Virtual Display driver).
//! Also handles Win32 thread execution state to prevent TV/display sleep while navigating.

use windows::core::PCWSTR;
use windows::Win32::Graphics::Gdi::{
    EnumDisplayDevicesW, DISPLAY_DEVICEW, DISPLAY_DEVICE_ACTIVE,
    DISPLAY_DEVICE_ATTACHED_TO_DESKTOP,
};
use windows::Win32::System::Power::{
    SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
};

#[derive(Debug, Clone)]
pub struct TvDisplayInfo {
    pub adapter_name: String,
    pub monitor_name: String,
    pub monitor_string: String,
    pub is_tv: bool,
    pub is_virtual_stream_display: bool,
}

/// Enumerates active display adapters and monitors to locate physical TVs or
/// virtual display devices used by Vibepollo/Sunshine streaming.
pub fn enumerate_displays() -> Vec<TvDisplayInfo> {
    let mut displays = Vec::new();
    let mut dev_num = 0;

    loop {
        let mut adapter: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        adapter.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

        let ok = unsafe { EnumDisplayDevicesW(PCWSTR::null(), dev_num, &mut adapter, 0) };
        if !ok.as_bool() {
            break;
        }

        let is_attached = (adapter.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP) != 0;
        if is_attached {
            let adapter_name = String::from_utf16_lossy(
                &adapter.DeviceName[..adapter.DeviceName.iter().position(|&c| c == 0).unwrap_or(adapter.DeviceName.len())],
            );

            // Now enumerate monitors attached to this adapter
            let mut mon_num = 0;
            loop {
                let mut monitor: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
                monitor.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;

                let mon_ok = unsafe {
                    EnumDisplayDevicesW(
                        PCWSTR::from_raw(adapter.DeviceName.as_ptr()),
                        mon_num,
                        &mut monitor,
                        0,
                    )
                };

                if !mon_ok.as_bool() {
                    break;
                }

                let mon_active = (monitor.StateFlags & DISPLAY_DEVICE_ACTIVE) != 0;
                if mon_active {
                    let mon_name = String::from_utf16_lossy(
                        &monitor.DeviceName[..monitor.DeviceName.iter().position(|&c| c == 0).unwrap_or(monitor.DeviceName.len())],
                    );
                    let mon_string = String::from_utf16_lossy(
                        &monitor.DeviceString[..monitor.DeviceString.iter().position(|&c| c == 0).unwrap_or(monitor.DeviceString.len())],
                    );

                    let mon_lower = mon_string.to_lowercase();
                    let is_tv = mon_lower.contains("tv")
                        || mon_lower.contains("oled")
                        || mon_lower.contains("lg")
                        || mon_lower.contains("samsung")
                        || mon_lower.contains("sony")
                        || mon_lower.contains("bravia")
                        || mon_lower.contains("tcl")
                        || mon_lower.contains("hisense")
                        || mon_lower.contains("vizio");

                    let is_virtual_stream_display = mon_lower.contains("iddcx")
                        || mon_lower.contains("virtual")
                        || mon_lower.contains("apollo")
                        || mon_lower.contains("sunshine")
                        || mon_lower.contains("moonlight")
                        || adapter_name.to_lowercase().contains("iddcx");

                    displays.push(TvDisplayInfo {
                        adapter_name: adapter_name.clone(),
                        monitor_name: mon_name,
                        monitor_string: mon_string,
                        is_tv,
                        is_virtual_stream_display,
                    });
                }

                mon_num += 1;
            }
        }

        dev_num += 1;
    }

    displays
}

/// Finds the most preferred display for Big Picture:
/// Prioritizes active virtual streaming monitors (Moonlight / Vibepollo),
/// then physical TVs, then falls back to primary desktop display.
pub fn find_preferred_display() -> Option<TvDisplayInfo> {
    let displays = enumerate_displays();

    // 1. Virtual streaming monitor (Vibepollo / Moonlight active)
    if let Some(disp) = displays.iter().find(|d| d.is_virtual_stream_display) {
        return Some(disp.clone());
    }

    // 2. Physical TV (OLED / Living room setup)
    if let Some(disp) = displays.iter().find(|d| d.is_tv) {
        return Some(disp.clone());
    }

    // 3. Fallback to first available display
    displays.into_iter().next()
}

/// RAII Guard that keeps the display and system awake while Big Picture is active.
pub struct TvSleepInhibitor {
    pub active: bool,
}

impl TvSleepInhibitor {
    pub fn acquire() -> Self {
        unsafe {
            SetThreadExecutionState(ES_CONTINUOUS | ES_DISPLAY_REQUIRED | ES_SYSTEM_REQUIRED);
        }
        Self { active: true }
    }
}

impl Drop for TvSleepInhibitor {
    fn drop(&mut self) {
        if self.active {
            unsafe {
                SetThreadExecutionState(ES_CONTINUOUS);
            }
            self.active = false;
        }
    }
}
