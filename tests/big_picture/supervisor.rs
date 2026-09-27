use dlss_studio::big_picture::supervisor::*;
use std::path::Path;

#[test]
fn test_launch_nonexistent_executable_fails_gracefully() {
    let dummy = Path::new("Z:\\nonexistent_folder_12345\\game.exe");
    let res = launch_and_supervise("Fake Game", dummy, || {}, || {});
    assert!(res.is_err());
    assert!(res.unwrap_err().contains("Target executable does not exist"));
}
