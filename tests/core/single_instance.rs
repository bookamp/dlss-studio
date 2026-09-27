use dlss_studio::core::single_instance::{acquire_named_mutex, signal_existing_instance};

#[test]
fn test_single_instance_guard_lifecycle() {
    let test_mutex = windows::core::w!("Local\\DLSS5_Studio_Test_Mutex_9999");
    let guard = acquire_named_mutex(test_mutex);
    assert!(guard.is_some(), "First acquisition in test must succeed");
    let second = acquire_named_mutex(test_mutex);
    assert!(second.is_none(), "Second acquisition while guard is held must be None");
    drop(guard);
    let third = acquire_named_mutex(test_mutex);
    assert!(third.is_some(), "Acquisition after dropping guard must succeed");
}

#[test]
fn test_signal_existing_instance_call() {
    signal_existing_instance(false);
    signal_existing_instance(true);
}
