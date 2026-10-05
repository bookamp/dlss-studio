use dlss_studio::core::vibepollo::session::*;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[test]
fn test_stream_session_event_lifecycle() {
    let event_name = "Local\\DLSS5_Test_StreamSession_Lifecycle";
    reset_stream_session_event_named(event_name);

    let unblocked = Arc::new(AtomicBool::new(false));
    let unblocked_clone = unblocked.clone();

    let handle = thread::spawn(move || {
        let ok = wait_for_stream_session_exit_named(event_name, 3000);
        unblocked_clone.store(true, Ordering::SeqCst);
        ok
    });

    // Ensure thread is waiting
    thread::sleep(Duration::from_millis(50));
    assert!(!unblocked.load(Ordering::SeqCst), "Session holder must block while event is nonsignaled");

    // Signal stream session exit
    signal_stream_session_exit_named(event_name);

    let res = handle.join().expect("Thread join must succeed");
    assert!(res, "wait_for_stream_session_exit must return true upon receiving signal");
    assert!(unblocked.load(Ordering::SeqCst));
}

#[test]
fn test_wait_for_stream_session_exit_timeout() {
    let event_name = "Local\\DLSS5_Test_StreamSession_Timeout";
    reset_stream_session_event_named(event_name);

    let start = std::time::Instant::now();
    let res = wait_for_stream_session_exit_named(event_name, 50);
    let elapsed = start.elapsed();

    assert!(!res, "wait_for_stream_session_exit must return false upon timeout");
    assert!(elapsed >= Duration::from_millis(40), "Must wait at least ~50ms before timing out");
}

#[test]
fn test_is_streaming_session_heuristics() {
    // Normal environment check
    let _ = is_streaming_session();

    // With Sunshine resolution env vars
    std::env::set_var("SUNSHINE_CLIENT_WIDTH", "1920");
    std::env::set_var("SUNSHINE_CLIENT_HEIGHT", "1080");
    assert!(is_streaming_session(), "is_streaming_session must return true when SUNSHINE_CLIENT_WIDTH is set");

    std::env::remove_var("SUNSHINE_CLIENT_WIDTH");
    std::env::remove_var("SUNSHINE_CLIENT_HEIGHT");

    // Standard function call smoke test
    reset_stream_session_event();
    signal_stream_session_exit();
}

#[test]
fn test_get_main_app_process_handle_safety() {
    #[cfg(windows)]
    {
        // Safe invocation when no separate tray window exists in headless test runner
        let h = get_main_app_process_handle();
        let _ = h;
    }
}
