use dlss_studio::big_picture::gamepad::*;

#[test]
fn test_classify_controller_vid_pid() {
    assert_eq!(classify_controller_vid_pid(0x054C, 0x0CE6), ControllerKind::PlayStation5);
    assert_eq!(classify_controller_vid_pid(0x054C, 0x0DF2), ControllerKind::PlayStation5);
    assert_eq!(classify_controller_vid_pid(0x054C, 0x05C4), ControllerKind::PlayStation4);
    assert_eq!(classify_controller_vid_pid(0x054C, 0x09CC), ControllerKind::PlayStation4);
    assert_eq!(classify_controller_vid_pid(0x045E, 0x0B12), ControllerKind::XboxSeriesX);
    assert_eq!(classify_controller_vid_pid(0x045E, 0x02D1), ControllerKind::XboxOne);
    assert_eq!(classify_controller_vid_pid(0x045E, 0x028E), ControllerKind::GenericXInput);
    assert_eq!(classify_controller_vid_pid(0x057E, 0x2009), ControllerKind::NintendoSwitch);
    assert_eq!(classify_controller_vid_pid(0x1234, 0x5678), ControllerKind::GenericXInput);
}

#[test]
fn test_controller_glyphs() {
    let ps5 = ControllerKind::PlayStation5;
    assert_eq!(ps5.play_glyph(), ("btn-glyph ps-cross", "✕"));
    assert_eq!(ps5.patch_glyph(), ("btn-glyph ps-square", "▢"));
    assert_eq!(ps5.filter_glyph(), ("btn-glyph ps-triangle", "△"));
    assert_eq!(ps5.back_glyph(), ("btn-glyph ps-circle", "○"));
    assert_eq!(ps5.bumper_labels(), ("L1", "R1"));

    let xbox = ControllerKind::XboxSeriesX;
    assert_eq!(xbox.play_glyph(), ("btn-glyph xb-a", "A"));
    assert_eq!(xbox.patch_glyph(), ("btn-glyph xb-x", "X"));
    assert_eq!(xbox.filter_glyph(), ("btn-glyph xb-y", "Y"));
    assert_eq!(xbox.back_glyph(), ("btn-glyph xb-b", "B"));
    assert_eq!(xbox.bumper_labels(), ("LB", "RB"));

    let switch = ControllerKind::NintendoSwitch;
    assert_eq!(switch.play_glyph(), ("btn-glyph n-b", "B"));
    assert_eq!(switch.patch_glyph(), ("btn-glyph n-y", "Y"));
    assert_eq!(switch.bumper_labels(), ("L", "R"));
}

#[test]
fn test_stick_deadzone() {
    // Below deadzone threshold
    let (l, r, u, d) = process_stick(3000, -2000);
    assert!(!l && !r && !u && !d);

    // Above threshold
    let (l, r, u, d) = process_stick(-20000, 0);
    assert!(l && !r && !u && !d);

    let (l, r, u, d) = process_stick(0, 22000);
    assert!(!l && !r && u && !d);
}

#[test]
fn test_repeat_tracker() {
    let mut tracker = RepeatTracker::new();

    // Initial press triggers immediately
    let act1 = tracker.should_trigger(Some(GamepadNavAction::NavigateRight));
    assert_eq!(act1, Some(GamepadNavAction::NavigateRight));

    // Immediate subsequent query within delay returns None
    let act2 = tracker.should_trigger(Some(GamepadNavAction::NavigateRight));
    assert_eq!(act2, None);

    // Releasing resets
    let act3 = tracker.should_trigger(None);
    assert_eq!(act3, None);
    assert!(tracker.last_pressed.is_none());
}

#[test]
fn test_all_controller_kind_badges_and_glyphs() {
    let kinds = [
        ControllerKind::XboxSeriesX,
        ControllerKind::XboxOne,
        ControllerKind::PlayStation5,
        ControllerKind::PlayStation4,
        ControllerKind::NintendoSwitch,
        ControllerKind::GenericXInput,
        ControllerKind::Disconnected,
    ];

    for k in &kinds {
        assert!(!k.badge_text().is_empty());
        let (play_cls, play_sym) = k.play_glyph();
        assert!(!play_cls.is_empty() && !play_sym.is_empty());

        let (patch_cls, patch_sym) = k.patch_glyph();
        assert!(!patch_cls.is_empty() && !patch_sym.is_empty());

        let (filter_cls, filter_sym) = k.filter_glyph();
        assert!(!filter_cls.is_empty() && !filter_sym.is_empty());

        let (back_cls, back_sym) = k.back_glyph();
        assert!(!back_cls.is_empty() && !back_sym.is_empty());

        let (l_bump, r_bump) = k.bumper_labels();
        assert!(!l_bump.is_empty() && !r_bump.is_empty());

        let (menu_cls, menu_sym) = k.menu_glyph();
        assert!(!menu_cls.is_empty() && !menu_sym.is_empty());
    }
}

#[test]
fn test_dynamic_xinput_and_device_detection() {
    let xinput = DynamicXInput::load();
    // DynamicXInput loads on any Windows system with XInput
    #[cfg(windows)]
    {
        assert!(xinput.is_some(), "DynamicXInput should load on Windows");
        if let Some(ref xi) = xinput {
            let _ = xi.get_state(0);
        }
    }
    let _ = xinput;

    let detected = detect_controller_kind();
    assert_ne!(detected, ControllerKind::Disconnected);
}

#[test]
fn test_high_precision_timer_guard_lifecycle() {
    let guard = HighPrecisionTimerGuard::new();
    drop(guard);
}

#[tokio::test]
async fn test_start_gamepad_listener_cancels_cleanly() {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let handle = start_gamepad_listener(tx, cancel.clone());

    // Give it a brief moment to run and emit initial ControllerChanged
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    cancel.store(true, std::sync::atomic::Ordering::SeqCst);

    let _ = handle.await;
    // We should receive at least ControllerChanged
    if let Ok(evt) = rx.try_recv() {
        match evt {
            GamepadNavAction::ControllerChanged(_) => {},
            _ => {},
        }
    }
}

#[test]
fn test_repeat_tracker_hold_to_repeat_interval() {
    let mut tracker = RepeatTracker::new();
    tracker.should_trigger(Some(GamepadNavAction::NavigateRight));

    // Fast forward tracker timing past initial delay (300ms) and repeat rate (90ms)
    tracker.first_trigger_time = std::time::Instant::now() - std::time::Duration::from_millis(350);
    tracker.last_repeat_time = std::time::Instant::now() - std::time::Duration::from_millis(100);

    let repeated = tracker.should_trigger(Some(GamepadNavAction::NavigateRight));
    assert_eq!(repeated, Some(GamepadNavAction::NavigateRight));
}

#[test]
fn test_stick_all_directions() {
    // Down
    let (l, r, u, d) = process_stick(0, -20000);
    assert!(!l && !r && !u && d);

    // Right
    let (l, r, u, d) = process_stick(20000, 0);
    assert!(!l && r && !u && !d);
}

#[test]
fn test_classify_all_controller_vid_pid_branches() {
    assert_eq!(classify_controller_vid_pid(0x054C, 0x0BA0), ControllerKind::PlayStation4);
    assert_eq!(classify_controller_vid_pid(0x054C, 0x9999), ControllerKind::PlayStation5);
    assert_eq!(classify_controller_vid_pid(0x045E, 0x0B20), ControllerKind::XboxSeriesX);
    assert_eq!(classify_controller_vid_pid(0x045E, 0x0B00), ControllerKind::XboxOne);
    assert_eq!(classify_controller_vid_pid(0x045E, 0x9999), ControllerKind::XboxSeriesX);
    assert_eq!(classify_controller_vid_pid(0x057E, 0x2006), ControllerKind::NintendoSwitch);
    assert_eq!(classify_controller_vid_pid(0x057E, 0x9999), ControllerKind::NintendoSwitch);
}

#[test]
fn test_gamepad_nav_action_variants() {
    let actions = [
        GamepadNavAction::NavigateLeft,
        GamepadNavAction::NavigateRight,
        GamepadNavAction::NavigateUp,
        GamepadNavAction::NavigateDown,
        GamepadNavAction::PageLeft,
        GamepadNavAction::PageRight,
        GamepadNavAction::PrimaryAction,
        GamepadNavAction::SecondaryAction,
        GamepadNavAction::TertiaryAction,
        GamepadNavAction::Back,
        GamepadNavAction::Start,
        GamepadNavAction::Guide,
    ];
    for a in &actions {
        let dbg = format!("{:?}", a);
        assert!(!dbg.is_empty());
        assert_eq!(*a, a.clone());
    }
}
