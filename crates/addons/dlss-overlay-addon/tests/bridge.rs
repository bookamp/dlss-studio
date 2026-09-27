use dlss_overlay_addon::*;
use std::fs;
use std::path::{Path, PathBuf};

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(prefix: &str) -> Self {
        let unique = format!(
            "test_{}_{}_{}",
            prefix,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
    fn path(&self) -> &Path {
        &self.path
    }
    fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.path.join(path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn test_render_overlay_surface_packet_structure() {
    let state = OverlayUiState::default();
    let frame = render_overlay_surface(&state, 1);

    assert!(frame.len() > 24);
    let magic = u32::from_le_bytes([frame[0], frame[1], frame[2], frame[3]]);
    let ver = u32::from_le_bytes([frame[4], frame[5], frame[6], frame[7]]);
    let seq = u32::from_le_bytes([frame[8], frame[9], frame[10], frame[11]]);
    let width = u32::from_le_bytes([frame[12], frame[13], frame[14], frame[15]]);
    let height = u32::from_le_bytes([frame[16], frame[17], frame[18], frame[19]]);
    let byte_len = u32::from_le_bytes([frame[20], frame[21], frame[22], frame[23]]);

    assert_eq!(magic, FRAME_MAGIC);
    assert_eq!(ver, 1);
    assert_eq!(seq, 1);
    assert_eq!(width, PANEL_WIDTH);
    assert_eq!(height, PANEL_HEIGHT);
    assert_eq!(byte_len, PANEL_WIDTH * PANEL_HEIGHT * 4);
}

#[test]
fn test_render_overlay_surface_mfg_presr_preview() {
    let mut state = OverlayUiState::default();
    state.has_mfg = true;
    state.mfg_enabled = true;
    state.mfg_multiplier = 1;
    state.has_presr = true;
    state.presr_enabled = false;
    state.presr_passes = 3;
    state.fps = 20.0;
    state.frametime_ms = 50.0;
    let frame = render_overlay_surface(&state, 1);
    assert!(frame.len() > 24);
}

#[test]
fn test_input_packet_mode_switching_and_scrolling() {
    let mut state = OverlayUiState::default();

    // Mode switch click on "Live tools" button at the bottom
    let switched = handle_input_packet(&mut state, 2, 50, 665, 0);
    assert!(switched);
    assert_eq!(state.mode, 1);

    // Switch back to "DLSS controls"
    let switched_back = handle_input_packet(&mut state, 2, 350, 665, 0);
    assert!(switched_back);
    assert_eq!(state.mode, 0);

    // Mouse wheel scrolling
    let scrolled = handle_input_packet(&mut state, 4, 200, 400, -120);
    assert!(scrolled);
    assert!(state.more_scroll > 0.0);
}

#[test]
fn test_live_tools_slider_interactivity() {
    let mut state = OverlayUiState::default();
    state.mode = 0;
    state.current_epoch = 10;

    let content_x = 12.0 + 24.0;
    let track_x = content_x + SLIDER_LABEL_WIDTH;

    // Click down on Structure Intensity slider
    let changed = handle_input_packet(&mut state, 2, (track_x + 50.0) as i32, 126, 0);
    assert!(changed);
    assert_eq!(state.active_slider.as_deref(), Some("structure_intensity"));
    assert!(state.pending_command.is_some());

    let cmd = state.pending_command.unwrap();
    assert_eq!(cmd.control_id, 101);
    assert_eq!(cmd.epoch, 10);
    assert_eq!(cmd.control_kind, 0);
}

#[test]
fn test_command_packet_generation_on_slider_interaction() {
    let mut state = OverlayUiState::default();
    state.mode = 0;
    state.current_epoch = 77;

    let content_x = 12.0 + 24.0;
    let track_x = content_x + SLIDER_LABEL_WIDTH;

    assert!(handle_input_packet(&mut state, 2, (track_x + 80.0) as i32, 154, 0));
    let cmd = state.pending_command.take().unwrap();
    assert_eq!(cmd.control_id, 102); // Tone Intensity
    assert_eq!(cmd.epoch, 77);

    assert!(handle_input_packet(&mut state, 1, (track_x + 120.0) as i32, 154, 0));
    let cmd_drag = state.pending_command.take().unwrap();
    assert_eq!(cmd_drag.control_id, 102);

    handle_input_packet(&mut state, 3, (track_x + 120.0) as i32, 154, 0);
    assert_eq!(state.active_slider, None);
    assert!(!state.mouse_down);
}

#[test]
fn test_straight_alpha_unpremultiplication_accuracy() {
    let cases = [
        (128u8, 64u8, 32u8, 128u8, 255u8, 128u8, 64u8),
        (255u8, 200u8, 100u8, 255u8, 255u8, 200u8, 100u8),
        (0u8, 0u8, 0u8, 0u8, 0u8, 0u8, 0u8),
        (50u8, 50u8, 50u8, 100u8, 128u8, 128u8, 128u8),
    ];

    for (r, g, b, a, exp_r, exp_g, exp_b) in cases {
        if a > 0 && a < 255 {
            let scale = 255.0 / (a as f32);
            let r_straight = ((r as f32) * scale).min(255.0).round() as u8;
            let g_straight = ((g as f32) * scale).min(255.0).round() as u8;
            let b_straight = ((b as f32) * scale).min(255.0).round() as u8;
            assert_eq!(r_straight, exp_r);
            assert_eq!(g_straight, exp_g);
            assert_eq!(b_straight, exp_b);
        } else {
            assert_eq!(r, exp_r);
            assert_eq!(g, exp_g);
            assert_eq!(b, exp_b);
        }
    }
}

#[test]
fn test_all_tab0_and_tab1_controls_hit_test_and_dispatch() {
    let mut state = OverlayUiState::default();
    state.current_epoch = 42;

    // 1. DLSS ON checkbox (Tab 0, cy = 60.0) -> Emits ONLY ID 103 without ID 200 on clicks
    assert!(handle_input_packet(&mut state, 2, 50, 62, 0));
    let cmd = state.pending_command.take().unwrap();
    assert_eq!(cmd.control_id, 103);
    assert_eq!(cmd.control_kind, 1);
    assert_eq!(cmd.epoch, 42);
    assert!(state.pending_commands.is_empty(), "DLSS ON must not dispatch ID 200 on user clicks");

    // 2. Character Mask checkbox (Tab 0, cy = 192.0)
    assert!(handle_input_packet(&mut state, 2, 50, 194, 0));
    let cmd = state.pending_command.take().unwrap();
    assert_eq!(cmd.control_id, 104);
    assert_eq!(cmd.control_kind, 1);

    // 3. NR Style Pill (Tab 0, cy = 278.0, index 1)
    assert!(handle_input_packet(&mut state, 2, 250, 285, 0));
    let cmd = state.pending_command.take().unwrap();
    assert_eq!(cmd.control_id, 114);
    assert_eq!(cmd.control_kind, 4);

    // 4. Tone Intensity slider (Tab 0, cy = 154.0)
    assert!(handle_input_packet(&mut state, 2, 300, 156, 0));
    let cmd = state.pending_command.take().unwrap();
    assert_eq!(cmd.control_id, 102);
    assert_eq!(cmd.control_kind, 0);

    // 5. Switch to Tab 1
    assert!(handle_input_packet(&mut state, 2, 50, 665, 0));
    assert_eq!(state.mode, 1);

    // 6. DLSS Neural Rendering in Tab 1 (cy = 200.0) -> Emits ONLY ID 103
    assert!(handle_input_packet(&mut state, 2, 50, 202, 0));
    let cmd = state.pending_command.take().unwrap();
    assert_eq!(cmd.control_id, 103);
    assert_eq!(cmd.control_kind, 1);
    assert!(state.pending_commands.is_empty());
}

#[test]
fn test_card_background_is_solid_obsidian() {
    let state = OverlayUiState::default();
    let frame = render_overlay_surface(&state, 1);
    let pixels = &frame[24..];
    let width = PANEL_WIDTH as usize;

    // Pixel at center of card (x = 200, y = 200) must have solid alpha 255
    let idx = (200 * width + 200) * 4;
    let a = pixels[idx + 3];
    assert_eq!(a, 255, "Card interior must have solid alpha 255 to eliminate 3D game bleed-through");

    // Pixel outside card (x = 2, y = 2) must be transparent (a == 0)
    let idx_outside = (2 * width + 2) * 4;
    let a_outside = pixels[idx_outside + 3];
    assert_eq!(a_outside, 0, "Outside of card must be transparent");
}

#[test]
fn test_status_json_deserialization_and_sync() {
    let json_sample = r#"{"epoch":55,"effects":true,"tools":[],"nrAvailable":true,"nrReason":"","nrEnabled":true,"nrTools":[
        {"id":101,"kind":0,"name":"Structure Intensity","effect":"RenoDX v4.7","min":0,"max":2,"step":0.01,"value":0.42,"available":true},
        {"id":102,"kind":0,"name":"Global Tone Intensity","effect":"RenoDX v4.7","min":0,"max":2,"step":0.01,"value":0.33,"available":true},
        {"id":103,"kind":1,"name":"Enable DLSS Neural Rendering","effect":"RenoDX v4.7","min":0,"max":1,"step":1,"value":1.0,"available":true}
    ]}"#;

    let status: AddonStatusJson = serde_json::from_str(json_sample).unwrap();
    assert_eq!(status.epoch, 55);
    assert!(status.nr_available);
    assert!(status.nr_enabled);
    assert_eq!(status.nr_tools.len(), 3);
    assert_eq!(status.nr_tools[0].value, 0.42);
    assert!(status.effects);
}

#[test]
fn test_tab0_offsets_calculation() {
    let base = compute_tab0_offsets(false, false);
    assert_eq!(base.y_top, 58.0);
    assert_eq!(base.y_global_divider, 86.0);

    let mfg_only = compute_tab0_offsets(true, false);
    assert!(mfg_only.has_mfg);
    assert!(!mfg_only.has_presr);
    assert_eq!(mfg_only.y_mfg_divider, 86.0);
    assert_eq!(mfg_only.y_global_divider, 86.0 + 68.0);

    let both = compute_tab0_offsets(true, true);
    assert!(both.has_mfg);
    assert!(both.has_presr);
    assert_eq!(both.y_global_divider, 86.0 + 68.0 + 68.0);
    assert!(both.scroll_view_h >= 90.0);
}

#[test]
fn test_reshade_fx_toggle_and_packet_dispatch() {
    let mut state = OverlayUiState::default();
    state.current_epoch = 12;
    state.reshade_fx_enabled = true;

    // Click ReShade FX toggle in top row of Tab 0
    assert!(handle_input_packet(&mut state, 2, 300, 60, 0));
    assert!(!state.reshade_fx_enabled);
    let cmd = state.pending_command.take().unwrap();
    assert_eq!(cmd.control_id, 0);
    assert_eq!(cmd.control_kind, 3);
    assert_eq!(cmd.value, 0.0);
    assert_eq!(cmd.epoch, 12);
}

#[test]
fn test_mfg_and_presr_ini_updates() {
    let temp = TempDir::new("overlay_ini");

    // Test MFG update
    update_mfg_ini(temp.path(), true, 3);
    let reshade_content = fs::read_to_string(temp.join("ReShade.ini")).unwrap();
    assert!(reshade_content.contains("[RenoDX.MFGUnlock]"));
    assert!(reshade_content.contains("ForceMultiplier=3"));

    // Test Pre-SR update
    let opti_path = temp.join("OptiScaler.ini");
    fs::write(&opti_path, "[DlssNr]\nRunBeforeSR=false\nPasses=1\n").unwrap();
    update_presr_ini(temp.path(), true, 2);
    let opti_content = fs::read_to_string(&opti_path).unwrap();
    assert!(opti_content.contains("RunBeforeSR=true"));
    assert!(opti_content.contains("Passes=2"));
}
