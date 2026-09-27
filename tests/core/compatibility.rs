use dlss_studio::core::compatibility::managed_mod_root;
use crate::common::TempDir;

#[test]
fn test_managed_mod_root_detection() {
    let temp = TempDir::new("mo2");
    let game_dir = temp.create_dir_all("Stock Game");
    let mo2_exe = temp.write_file("ModOrganizer.exe", b"dummy");

    let detected = managed_mod_root(&game_dir, None);
    assert_eq!(detected, Some(temp.path().to_path_buf()));

    // Negative check without ModOrganizer.exe
    let _ = std::fs::remove_file(&mo2_exe);
    assert_eq!(managed_mod_root(&game_dir, None), None);
}
