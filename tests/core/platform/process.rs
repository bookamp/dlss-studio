use dlss_studio::core::install_guards::{get_running_processes, has_anti_cheat};
use crate::common::TempDir;

#[test]
fn test_has_anti_cheat_detection() {
    let temp = TempDir::new("ac");
    assert!(!has_anti_cheat(temp.path()));

    // EAC
    let eac_file = temp.write_file("EasyAntiCheat_x64.dll", b"dummy");
    assert!(has_anti_cheat(temp.path()));
    let _ = std::fs::remove_file(eac_file);

    // BattlEye in subfolder
    let be_file = temp.write_file("Binaries/BEService_x64.exe", b"dummy");
    assert!(has_anti_cheat(temp.path()));
    let _ = std::fs::remove_file(be_file);

    // Vanguard
    let vgk_file = temp.write_file("Binaries/vgk.sys", b"dummy");
    assert!(has_anti_cheat(temp.path()));
    let _ = std::fs::remove_file(vgk_file);
}

#[test]
fn test_running_processes_snapshot() {
    let procs = get_running_processes();
    assert!(!procs.is_empty());
}

#[test]
fn test_assert_game_closed() {
    let temp = TempDir::new("assert_closed");
    // None exe_path always succeeds
    assert!(dlss_studio::core::install_guards::assert_game_closed(temp.path(), None).is_ok());

    // Non-running game succeeds
    let non_running = std::path::Path::new("unlikely_game_binary_9999.exe");
    assert!(dlss_studio::core::install_guards::assert_game_closed(temp.path(), Some(non_running)).is_ok());

    // If we pass an exe name known to be currently running
    let procs = get_running_processes();
    if let Some(first) = procs.first() {
        let running_path = std::path::Path::new(&first.name);
        let res = dlss_studio::core::install_guards::assert_game_closed(temp.path(), Some(running_path));
        assert!(res.is_err(), "Asserting closed for running process {} should fail", first.name);
    }
}

