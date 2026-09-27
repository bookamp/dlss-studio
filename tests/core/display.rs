use dlss_studio::core::display::*;

#[test]
fn test_enumerate_monitors_runs() {
    let monitors = enumerate_monitors();
    #[cfg(windows)]
    {
        assert!(!monitors.is_empty(), "Should detect at least one display monitor");
    }
    let _ = monitors;
}

#[test]
fn test_find_streaming_display_fallback() {
    let display = find_streaming_display();
    #[cfg(windows)]
    {
        assert!(display.is_some(), "Should resolve a streaming display target");
    }
    let _ = display;
}

#[test]
fn test_desktop_window_default_size_constants() {
    assert_eq!(DEFAULT_DESKTOP_WIDTH, 1280);
    assert_eq!(DEFAULT_DESKTOP_HEIGHT, 900);
}

#[test]
fn test_is_virtual_display_present_runs() {
    let _ = is_virtual_display_present();
}

#[test]
fn test_is_streaming_display_active_runs() {
    let _ = is_streaming_display_active(None);
    let _ = is_streaming_display_active(Some("Nonexistent Display"));
}

#[test]
fn test_is_streaming_display_active_with_sunshine_env() {
    // Test with Sunshine client environment variables
    unsafe {
        std::env::set_var("SUNSHINE_CLIENT_WIDTH", "1920");
    }
    assert!(is_streaming_display_active(None), "Should report active streaming when Sunshine env is present");
    unsafe {
        std::env::remove_var("SUNSHINE_CLIENT_WIDTH");
    }
}

#[test]
fn test_restore_desktop_window_geometry_safe_execution() {
    // Safe to call even during unit testing when app hwnd might or might not be present
    let _ = restore_desktop_window_geometry();
}
