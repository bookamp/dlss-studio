#![allow(dead_code)]

use std::thread;
use crate::bridge::{
    render_overlay_surface, handle_input_packet, OverlayUiState, PANEL_WIDTH, PANEL_HEIGHT
};

#[repr(C)]
struct BitmapInfoHeader {
    bi_size: u32,
    bi_width: i32,
    bi_height: i32,
    bi_planes: u16,
    bi_bit_count: u16,
    bi_compression: u32,
    bi_size_image: u32,
    bi_x_pels_per_meter: i32,
    bi_y_pels_per_meter: i32,
    bi_clr_used: u32,
    bi_clr_important: u32,
}

#[repr(C)]
struct PaintStruct {
    hdc: isize,
    f_erase: i32,
    rc_paint: [i32; 4],
    f_restore: i32,
    f_inc_update: i32,
    rgb_reserved: [u8; 32],
}

#[repr(C)]
struct Msg {
    hwnd: isize,
    message: u32,
    w_param: usize,
    l_param: isize,
    time: u32,
    pt_x: i32,
    pt_y: i32,
}

#[repr(C)]
struct WndClassW {
    style: u32,
    lpfn_wnd_proc: unsafe extern "system" fn(isize, u32, usize, isize) -> isize,
    cb_cls_extra: i32,
    cb_wnd_extra: i32,
    h_instance: isize,
    h_icon: isize,
    h_cursor: isize,
    hbr_background: isize,
    lpsz_menu_name: *const u16,
    lpsz_class_name: *const u16,
}

extern "system" {
    fn GetModuleHandleW(lpModuleName: *const u16) -> isize;
    fn RegisterClassW(lpWndClass: *const WndClassW) -> u16;
    fn CreateWindowExW(
        dwExStyle: u32,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: u32,
        X: i32,
        Y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: isize,
        hMenu: isize,
        hInstance: isize,
        lpParam: *mut std::ffi::c_void,
    ) -> isize;
    fn ShowWindow(hWnd: isize, nCmdShow: i32) -> i32;
    fn UpdateWindow(hWnd: isize) -> i32;
    fn SetForegroundWindow(hWnd: isize) -> i32;
    fn InvalidateRect(hWnd: isize, lpRect: *const std::ffi::c_void, bErase: i32) -> i32;
    fn BeginPaint(hWnd: isize, lpPaint: *mut PaintStruct) -> isize;
    fn EndPaint(hWnd: isize, lpPaint: *const PaintStruct) -> i32;
    fn DefWindowProcW(hWnd: isize, Msg: u32, wParam: usize, lParam: isize) -> isize;
    fn PostQuitMessage(nExitCode: i32);
    fn GetMessageW(lpMsg: *mut Msg, hWnd: isize, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
    fn TranslateMessage(lpMsg: *const Msg) -> i32;
    fn DispatchMessageW(lpMsg: *const Msg) -> isize;
    fn SetDIBitsToDevice(
        hdc: isize,
        xDest: i32,
        yDest: i32,
        w: u32,
        h: u32,
        xSrc: i32,
        ySrc: i32,
        StartScan: u32,
        cLines: u32,
        lpvBits: *const u8,
        lpbmi: *const BitmapInfoHeader,
        ColorUse: u32,
    ) -> i32;
    fn ReleaseCapture() -> i32;
    fn SendMessageW(hWnd: isize, Msg: u32, wParam: usize, lParam: isize) -> isize;
    fn AdjustWindowRectEx(lpRect: *mut [i32; 4], dwStyle: u32, bMenu: i32, dwExStyle: u32) -> i32;
    fn LoadCursorW(hInstance: isize, lpCursorName: *const u16) -> isize;
    fn GetStockObject(i: i32) -> isize;
    fn OpenDesktopW(lpszDesktop: *const u16, dwFlags: u32, fInherit: i32, dwDesiredAccess: u32) -> isize;
    fn SetThreadDesktop(hDesktop: isize) -> i32;
}

static mut GLOBAL_STATE: Option<OverlayUiState> = None;
static mut GLOBAL_SEQ: u32 = 1;

unsafe extern "system" fn preview_wnd_proc(
    hwnd: isize,
    msg: u32,
    w_param: usize,
    l_param: isize,
) -> isize {
    match msg {
        0x000F => {
            let mut ps = PaintStruct {
                hdc: 0,
                f_erase: 0,
                rc_paint: [0; 4],
                f_restore: 0,
                f_inc_update: 0,
                rgb_reserved: [0; 32],
            };
            let hdc = BeginPaint(hwnd, &mut ps);
            if let Some(ref state) = GLOBAL_STATE {
                let frame = render_overlay_surface(state, GLOBAL_SEQ);
                if frame.len() >= 24 + (PANEL_WIDTH * PANEL_HEIGHT * 4) as usize {
                    let rgba = &frame[24..];
                    let mut bgra = Vec::with_capacity(rgba.len());
                    for chunk in rgba.chunks_exact(4) {
                        bgra.push(chunk[2]);
                        bgra.push(chunk[1]);
                        bgra.push(chunk[0]);
                        bgra.push(chunk[3]);
                    }

                    let bmi = BitmapInfoHeader {
                        bi_size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                        bi_width: PANEL_WIDTH as i32,
                        bi_height: -(PANEL_HEIGHT as i32),
                        bi_planes: 1,
                        bi_bit_count: 32,
                        bi_compression: 0,
                        bi_size_image: 0,
                        bi_x_pels_per_meter: 0,
                        bi_y_pels_per_meter: 0,
                        bi_clr_used: 0,
                        bi_clr_important: 0,
                    };

                    SetDIBitsToDevice(
                        hdc,
                        0,
                        0,
                        PANEL_WIDTH,
                        PANEL_HEIGHT,
                        0,
                        0,
                        0,
                        PANEL_HEIGHT,
                        bgra.as_ptr(),
                        &bmi,
                        0,
                    );
                }
            }
            EndPaint(hwnd, &ps);
            0
        }
        0x0201 => {
            let x = (l_param & 0xFFFF) as i16 as i32;
            let y = ((l_param >> 16) & 0xFFFF) as i16 as i32;

            if let Some(ref mut state) = GLOBAL_STATE {
                if handle_input_packet(state, 2, x, y, 0) {
                    GLOBAL_SEQ += 1;
                    if let Some(cmd) = state.pending_command.take() {
                        println!(
                            "[OVERLAY DISPATCH] Control ID: {}, Value: {:.2}",
                            cmd.control_id, cmd.value
                        );
                    }
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
            }
            0
        }
        0x0200 => {
            let x = (l_param & 0xFFFF) as i16 as i32;
            let y = ((l_param >> 16) & 0xFFFF) as i16 as i32;

            if let Some(ref mut state) = GLOBAL_STATE {
                if state.mouse_down {
                    if handle_input_packet(state, 1, x, y, 0) {
                        GLOBAL_SEQ += 1;
                        if let Some(cmd) = state.pending_command.take() {
                            println!(
                                "[OVERLAY DISPATCH] Control ID: {}, Value: {:.2}",
                                cmd.control_id, cmd.value
                            );
                        }
                        InvalidateRect(hwnd, std::ptr::null(), 0);
                    }
                }
            }
            0
        }
        0x0202 => {
            let x = (l_param & 0xFFFF) as i16 as i32;
            let y = ((l_param >> 16) & 0xFFFF) as i16 as i32;

            if let Some(ref mut state) = GLOBAL_STATE {
                handle_input_packet(state, 3, x, y, 0);
                InvalidateRect(hwnd, std::ptr::null(), 0);
            }
            0
        }
        0x020A => {
            let delta = ((w_param >> 16) as i16) as i32;
            if let Some(ref mut state) = GLOBAL_STATE {
                if handle_input_packet(state, 4, 0, 0, delta) {
                    GLOBAL_SEQ += 1;
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
            }
            0
        }
        0x0100 => {
            if w_param == 0x1B {
                PostQuitMessage(0);
                return 0;
            }
            if w_param == 0x09 {
                if let Some(ref mut state) = GLOBAL_STATE {
                    state.mode = if state.mode == 0 { 1 } else { 0 };
                    GLOBAL_SEQ += 1;
                    InvalidateRect(hwnd, std::ptr::null(), 0);
                }
                return 0;
            }
            DefWindowProcW(hwnd, msg, w_param, l_param)
        }
        0x0002 => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, w_param, l_param),
    }
}

pub fn run_overlay_preview_window() {
    let handle = thread::spawn(|| {
        unsafe {
            // Attach this clean thread to the user's interactive physical desktop ("Default")
            let desk_name: Vec<u16> = "Default\0".encode_utf16().collect();
            let h_desk = OpenDesktopW(desk_name.as_ptr(), 0, 0, 0x01FF);
            if h_desk != 0 {
                SetThreadDesktop(h_desk);
            }

            let mut initial_state = OverlayUiState::default();
            initial_state.fps = 138.0;
            initial_state.frametime_ms = 7.2;
            initial_state.has_mfg = true;
            initial_state.has_presr = true;
            let theme = crate::bridge::get_active_overlay_theme();

            if let Some((r, g, b)) = match theme.as_str() {
                "blue" | "azure" => Some((0x4a, 0xa8, 0xee)),
                "purple" | "amethyst" => Some((0xb4, 0x5d, 0xea)),
                _ => Some((0xd4, 0xff, 0x00)),
            } {
                initial_state.accent_color = 0xFF00_0000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32);
            }

            GLOBAL_STATE = Some(initial_state);

            let class_name: Vec<u16> = "DLSS5OverlayPreviewDirectClass\0".encode_utf16().collect();
            let title: Vec<u16> = "DLSS 5 Studio In-Game Overlay (Live Desktop Test)\0".encode_utf16().collect();

            let h_instance = GetModuleHandleW(std::ptr::null());
            let h_cursor = LoadCursorW(0, 32512 as *const u16);
            let hbr_black = GetStockObject(4);

            let wc = WndClassW {
                style: 0x0003,
                lpfn_wnd_proc: preview_wnd_proc,
                cb_cls_extra: 0,
                cb_wnd_extra: 0,
                h_instance,
                h_icon: 0,
                h_cursor,
                hbr_background: hbr_black,
                lpsz_menu_name: std::ptr::null(),
                lpsz_class_name: class_name.as_ptr(),
            };

            RegisterClassW(&wc);

            let ex_style = 0x00000008; // WS_EX_TOPMOST
            let style = 0x10CA0000;    // WS_VISIBLE | WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX
            let mut rect = [0, 0, PANEL_WIDTH as i32, PANEL_HEIGHT as i32];
            AdjustWindowRectEx(&mut rect, style, 0, ex_style);

            let win_w = rect[2] - rect[0];
            let win_h = rect[3] - rect[1];

            let hwnd = CreateWindowExW(
                ex_style,
                class_name.as_ptr(),
                title.as_ptr(),
                style,
                250,
                100,
                win_w,
                win_h,
                0,
                0,
                h_instance,
                std::ptr::null_mut(),
            );

            if hwnd == 0 {
                println!("Failed to create preview window!");
                return;
            }

            ShowWindow(hwnd, 5); // SW_SHOW
            UpdateWindow(hwnd);
            SetForegroundWindow(hwnd);

            println!("============================================================");
            println!("DLSS 5 Studio In-Game Overlay Test Window Launched on Default Desktop!");
            println!("HWND: 0x{:X}", hwnd);
            println!("- Left-click and drag sliders; click checkboxes.");
            println!("- Click bottom pills to switch between DLSS controls and Live tools.");
            println!("- Press Tab to toggle mode. Press Esc to close.");
            println!("============================================================");

            let mut msg = Msg {
                hwnd: 0,
                message: 0,
                w_param: 0,
                l_param: 0,
                time: 0,
                pt_x: 0,
                pt_y: 0,
            };

            while GetMessageW(&mut msg, 0, 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    });

    let _ = handle.join();
}
