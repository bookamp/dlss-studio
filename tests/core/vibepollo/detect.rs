use dlss_studio::core::vibepollo::detect::*;
use std::path::PathBuf;

#[test]
fn test_constants_definitions() {
    assert!(DEFAULT_APPS_JSON.contains("apps.json"));
    assert!(DEFAULT_APOLLO_DIR.contains("Apollo"));
    assert!(DEFAULT_SUNSHINE_DIR.contains("Sunshine"));
}

#[test]
fn test_parse_service_install_root_variations() {
    // 1. Quoted path with arguments and tools subfolder (typical ApolloService / Vibepollo)
    let apollo_service = r#""C:\Program Files\Apollo\tools\sunshinesvc.exe" --service"#;
    let root = parse_service_install_root(apollo_service);
    assert_eq!(root, Some(PathBuf::from(r"C:\Program Files\Apollo")));

    // 2. Quoted path with bin subfolder
    let custom_bin = r#""D:\Streaming\Sunshine\bin\sunshinesvc.exe" -d"#;
    let root_bin = parse_service_install_root(custom_bin);
    assert_eq!(root_bin, Some(PathBuf::from(r"D:\Streaming\Sunshine")));

    // 3. Quoted path directly at installation root
    let root_binary = r#""C:\Program Files\Sunshine\sunshine.exe""#;
    let root_direct = parse_service_install_root(root_binary);
    assert_eq!(root_direct, Some(PathBuf::from(r"C:\Program Files\Sunshine")));

    // 4. Unquoted path with arguments
    let unquoted = r#"C:\Program Files\Apollo\tools\sunshinesvc.exe --service"#;
    let root_unquoted = parse_service_install_root(unquoted);
    assert_eq!(root_unquoted, Some(PathBuf::from(r"C:\Program Files\Apollo")));

    // 5. Empty or whitespace
    assert_eq!(parse_service_install_root(""), None);
    assert_eq!(parse_service_install_root("   "), None);
}

#[test]
fn test_get_vibepollo_apps_path_resolution() {
    if is_vibepollo_installed() {
        let path = get_vibepollo_apps_path();
        assert!(path.is_some(), "Installed host must resolve apps.json path");
        let p = path.unwrap();
        assert!(p.ends_with("apps.json"), "Path must terminate in apps.json");
        let path_str = p.to_string_lossy().to_lowercase();
        assert!(
            path_str.contains("apollo") || path_str.contains("sunshine") || path_str.contains("vibepollo"),
            "Resolved path must belong to Apollo, Sunshine, or Vibepollo"
        );
    }
}

#[test]
fn test_get_service_install_root_discovery() {
    if let Some(root) = get_service_install_root() {
        assert!(root.exists(), "Discovered service root directory must exist on disk");
        let root_str = root.to_string_lossy().to_lowercase();
        assert!(
            root_str.contains("apollo") || root_str.contains("sunshine") || root_str.contains("vibepollo"),
            "Discovered service root must be Apollo, Sunshine, or Vibepollo"
        );
    }
}

#[cfg(windows)]
#[test]
fn test_expand_env_vars_and_reg_queries() {
    let unexpanded = "%SystemRoot%\\System32";
    let expanded = expand_env_vars(unexpanded);
    assert!(!expanded.contains("%SystemRoot%"));
    assert!(expanded.contains("System32"));

    // String with no env vars should return unchanged
    assert_eq!(expand_env_vars("NoVarsHere"), "NoVarsHere");

    // Non-existent registry query returns None
    let non_existent = read_hklm_reg_string("SOFTWARE\\NonExistentKey12345", "NonExistentVal");
    assert!(non_existent.is_none());
}

#[test]
fn test_is_vibepollo_installed_execution() {
    // Tests execution path across services, paths, processes, and registry
    let _ = is_vibepollo_installed();
}

#[cfg(windows)]
#[test]
fn test_expand_env_vars_multi_token_and_casing() {
    let input = "%programfiles%\\Apollo\\%SYSTEMROOT%";
    let expanded = expand_env_vars(input);
    assert!(!expanded.contains("%programfiles%"));
    assert!(!expanded.contains("%SYSTEMROOT%"));

    let unknown = "%UNKNOWN_VAR_ABC%\\folder";
    assert_eq!(expand_env_vars(unknown), unknown);
}

#[test]
fn test_parse_service_install_root_edge_cases() {
    // Uppercase BIN folder
    let upper = r#""C:\Streaming\Apollo\BIN\svc.exe""#;
    assert_eq!(parse_service_install_root(upper), Some(PathBuf::from(r"C:\Streaming\Apollo")));

    // No .exe extension
    let no_ext = r#"C:\Streaming\Apollo\tools\service --run"#;
    assert_eq!(parse_service_install_root(no_ext), Some(PathBuf::from(r"C:\Streaming\Apollo")));
}

#[test]
fn test_get_vibepollo_apps_path_with_programdata() {
    let temp = crate::common::TempDir::new("pd_apollo");
    let config_dir = temp.join("Apollo").join("config");
    std::fs::create_dir_all(&config_dir).unwrap();
    let apps_json = config_dir.join("apps.json");
    std::fs::write(&apps_json, b"{}").unwrap();

    let orig_pd = std::env::var("ProgramData").ok();
    unsafe {
        std::env::set_var("ProgramData", temp.path());
    }

    let resolved = get_vibepollo_apps_path();
    assert!(resolved.is_some());

    // 2. Stage LizardByte Sunshine path
    let lb_dir = temp.join("LizardByte").join("Sunshine");
    std::fs::create_dir_all(&lb_dir).unwrap();
    std::fs::write(lb_dir.join("apps.json"), b"{}").unwrap();
    let resolved_lb = get_vibepollo_apps_path();
    assert!(resolved_lb.is_some());

    if let Some(val) = orig_pd {
        unsafe { std::env::set_var("ProgramData", val); }
    } else {
        unsafe { std::env::remove_var("ProgramData"); }
    }
}

#[test]
fn test_parse_service_install_root_unclosed_quotes_and_case() {
    // Unclosed quote
    let unclosed = "\"C:\\Streaming\\Apollo\\tools\\sunshinesvc.exe";
    let res1 = parse_service_install_root(unclosed);
    assert_eq!(res1, Some(PathBuf::from(r"C:\Streaming\Apollo")));

    // Case-insensitive .EXE
    let uppercase_exe = "C:\\Streaming\\Sunshine\\SUNSHINE.EXE --flag";
    let res2 = parse_service_install_root(uppercase_exe);
    assert_eq!(res2, Some(PathBuf::from(r"C:\Streaming\Sunshine")));
}

#[cfg(windows)]
#[test]
fn test_expand_env_vars_programdata_and_localappdata() {
    let s1 = "%ProgramData%\\Apollo";
    let e1 = expand_env_vars(s1);
    assert!(!e1.contains("%ProgramData%"));

    let s2 = "%LOCALAPPDATA%\\Sunshine";
    let e2 = expand_env_vars(s2);
    assert!(!e2.contains("%LOCALAPPDATA%"));
}

