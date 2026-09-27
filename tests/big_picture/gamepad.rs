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
