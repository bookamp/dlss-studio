use dlss_studio::big_picture::tv::*;

#[test]
fn test_enumerate_displays_safety() {
    // Must never crash or cause access violation
    let displays = enumerate_displays();
    for d in &displays {
        println!("[Display] {} -> {} (TV={}, Virtual={})", d.adapter_name, d.monitor_string, d.is_tv, d.is_virtual_stream_display);
    }
}

#[test]
fn test_sleep_inhibitor_raii() {
    let guard = TvSleepInhibitor::acquire();
    assert!(guard.active);
    drop(guard);
}

#[test]
fn test_find_preferred_display() {
    let pref = find_preferred_display();
    // On systems with a display attached, pref will be Some
    let _ = pref;
}

