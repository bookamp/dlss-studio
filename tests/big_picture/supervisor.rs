use dlss_studio::big_picture::supervisor::*;
use std::path::Path;

#[test]
fn test_launch_nonexistent_executable_fails_gracefully() {
    let dummy = Path::new("Z:\\nonexistent_folder_12345\\game.exe");
    let res = launch_and_supervise("Fake Game", dummy, || {}, || {});
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("Target executable does not exist"));
}

#[tokio::test]
async fn test_launch_and_supervise_real_binary() {
    let host_exe = Path::new(r"C:\Windows\System32\hostname.exe");
    if host_exe.is_file() {
        let launched = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let exited = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

        let l_clone = launched.clone();
        let e_clone = exited.clone();

        let session = launch_and_supervise(
            "TestHostname",
            host_exe,
            move || { l_clone.store(true, std::sync::atomic::Ordering::SeqCst); },
            move || { e_clone.store(true, std::sync::atomic::Ordering::SeqCst); },
        );

        assert!(session.is_ok());
        let s = session.unwrap();
        assert_eq!(s.game_title, "TestHostname");
        assert!(launched.load(std::sync::atomic::Ordering::SeqCst));

        // Poll up to 4s for supervisor loop (1.5s initial grace period + poll interval)
        let mut did_exit = false;
        for _ in 0..40 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if exited.load(std::sync::atomic::Ordering::SeqCst) {
                did_exit = true;
                break;
            }
        }
        assert!(did_exit, "Supervisor should detect process exit and trigger on_exit");
    }
}

#[test]
fn test_cleanup_feeder_helper_safe_execution() {
    cleanup_feeder_helper();
}

#[test]
fn test_game_launch_session_clone_and_debug() {
    let session = GameLaunchSession {
        game_title: "Title".to_string(),
        exe_path: std::path::PathBuf::from("C:\\game.exe"),
        is_running: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
    };
    let cloned = session.clone();
    assert_eq!(cloned.game_title, "Title");
    assert!(cloned.is_running.load(std::sync::atomic::Ordering::SeqCst));
    let dbg = format!("{:?}", session);
    assert!(dbg.contains("GameLaunchSession"));
}

#[test]
fn test_launch_directory_fails_gracefully() {
    let temp = std::env::temp_dir();
    let res = launch_and_supervise("Dir Game", &temp, || {}, || {});
    assert!(res.is_err());
}

#[test]
fn test_launch_relative_nonexistent_fails() {
    let dummy = std::path::Path::new("nonexistent_relative_game_123.exe");
    let res = launch_and_supervise("Relative Game", dummy, || {}, || {});
    assert!(res.is_err());
}


