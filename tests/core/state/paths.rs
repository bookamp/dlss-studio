use dlss_studio::core::state::paths::*;

#[test]
fn test_is_path_protected_detection() {
    assert!(is_path_protected(r"C:\Program Files\DLSS 5 Studio"));
    assert!(is_path_protected(r"C:\Program Files (x86)\DLSS 5 Studio"));
    assert!(is_path_protected("C:/Program Files/DLSS 5 Studio"));
    assert!(is_path_protected(r"C:\Windows\System32"));
    assert!(is_path_protected(r"C:\Program Files\WindowsApps\Game"));

    // Unprotected directories
    assert!(!is_path_protected(r"D:\Games\DLSS 5 Studio"));
    assert!(!is_path_protected(r"C:\Games\DLSS 5 Studio"));
    assert!(!is_path_protected(r"E:\Tools\ModManager"));
}

#[test]
fn test_resolve_appdata_dir_portable_priority() {
    let res = resolve_appdata_dir_internal(
        "dlss-studio-portable.exe",
        Some(std::path::Path::new(r"D:\Games\DLSS 5 Studio")),
        Some(r#"{"data_dir": "E:\\CustomStorage"}"#),
        Some(r"C:\ProgramData"),
        Some(r"C:\Users\Tester\AppData\Roaming"),
    );
    assert_eq!(res, std::path::PathBuf::from(r"C:\Users\Tester\AppData\Roaming\dlss-5-studio"));
}

#[test]
fn test_resolve_appdata_dir_storage_json_override() {
    let res = resolve_appdata_dir_internal(
        "dlss-studio.exe",
        Some(std::path::Path::new(r"C:\Program Files\DLSS 5 Studio")),
        Some(r#"{"data_dir": "D:\\MyCustomStorage"}"#),
        Some(r"C:\ProgramData"),
        Some(r"C:\Users\Tester\AppData\Roaming"),
    );
    assert_eq!(res, std::path::PathBuf::from(r"D:\MyCustomStorage"));
}

#[test]
fn test_resolve_appdata_dir_unprotected_install_defaults_to_data() {
    let res = resolve_appdata_dir_internal(
        "dlss-studio.exe",
        Some(std::path::Path::new(r"D:\Games\DLSS 5 Studio")),
        None,
        Some(r"C:\ProgramData"),
        Some(r"C:\Users\Tester\AppData\Roaming"),
    );
    assert_eq!(res, std::path::PathBuf::from(r"D:\Games\DLSS 5 Studio\data"));
}

#[test]
fn test_resolve_appdata_dir_protected_install_defaults_to_programdata() {
    let res = resolve_appdata_dir_internal(
        "dlss-studio.exe",
        Some(std::path::Path::new(r"C:\Program Files\DLSS 5 Studio")),
        None,
        Some(r"C:\ProgramData"),
        Some(r"C:\Users\Tester\AppData\Roaming"),
    );
    assert_eq!(res, std::path::PathBuf::from(r"C:\ProgramData\dlss-5-studio"));
}

#[test]
fn test_clean_path_separators_mixed_slashes_and_drive_casing() {
    let raw = std::path::Path::new("d:/program files (x86)/steam\\steamapps\\common\\left 4 dead");
    let cleaned = clean_path_separators(raw);
    assert_eq!(
        cleaned,
        std::path::PathBuf::from(r"D:\program files (x86)\steam\steamapps\common\left 4 dead")
    );

    let trailing = std::path::Path::new("c:/games/test/");
    assert_eq!(clean_path_separators(trailing), std::path::PathBuf::from(r"C:\games\test"));
}

#[test]
fn test_clean_path_separators_edge_cases() {
    let rel = std::path::Path::new("games/steam/left 4 dead");
    assert_eq!(clean_path_separators(rel), std::path::PathBuf::from(r"games\steam\left 4 dead"));

    let empty = std::path::Path::new("");
    assert_eq!(clean_path_separators(empty), std::path::PathBuf::from(""));

    let drive_only = std::path::Path::new("e:/");
    assert_eq!(clean_path_separators(drive_only), std::path::PathBuf::from("E:"));

    let unc = std::path::Path::new(r"\\server\share/games\steam");
    assert_eq!(clean_path_separators(unc), std::path::PathBuf::from(r"\\server\share\games\steam"));
}
