use dlss_studio::core::vibepollo::apps_json::*;

#[test]
fn test_apps_json_manipulation_mock() {
    let initial_json = serde_json::json!({
        "apps": [
            {
                "name": "Desktop",
                "image-path": "desktop.png",
                "uuid": "11111111-2222-3333-4444-555555555555"
            }
        ],
        "env": {},
        "version": 2
    });

    let mut doc = initial_json.clone();
    let apps = doc.get_mut("apps").unwrap().as_array_mut().unwrap();

    // Register DLSS Studio
    let entry = serde_json::json!({
        "name": DLSS_STUDIO_APP_NAME,
        "cmd": "\"C:\\Path\\dlss-studio.exe\" --big-picture",
        "working-dir": "C:\\Path",
        "image-path": "C:\\Path\\poster.png",
        "uuid": DLSS_STUDIO_UUID
    });
    apps.push(entry);

    assert_eq!(apps.len(), 2);
    assert!(apps.iter().any(|a| a.get("name").and_then(|n| n.as_str()) == Some("DLSS Studio")));

    // Unregister DLSS Studio
    apps.retain(|a| a.get("name").and_then(|n| n.as_str()) != Some("DLSS Studio"));
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].get("name").and_then(|n| n.as_str()), Some("Desktop"));
}

#[test]
fn test_constants_definitions() {
    assert_eq!(DLSS_STUDIO_APP_NAME, "DLSS Studio");
    assert!(DLSS_STUDIO_UUID.contains("DLSS5STUDIO2"));
}

#[test]
fn test_write_apps_json_direct() {
    let temp = crate::common::TempDir::new("vibepollo_write");
    let target = temp.join("apps.json");
    let content = "{\"apps\":[],\"version\":2}";

    let res = write_apps_json(&target, content);
    assert!(res.is_ok(), "Direct write to temporary directory should succeed: {:?}", res);
    assert_eq!(std::fs::read_to_string(&target).unwrap(), content);
}

#[test]
fn test_write_apps_json_creates_parent_directories() {
    let temp = crate::common::TempDir::new("vibepollo_nested");
    let target = temp.join("deep").join("config").join("apps.json");
    let content = "{\"apps\":[],\"version\":2}";

    let res = write_apps_json(&target, content);
    assert!(res.is_ok());
    assert!(target.exists());
    assert_eq!(std::fs::read_to_string(&target).unwrap(), content);
}

#[test]
fn test_registration_lifecycle_sandboxed() {
    let temp = crate::common::TempDir::new("vibepollo_reg_lifecycle");
    let apps_path = temp.join("apps.json");

    // 1. Initial state: file does not exist
    assert!(!is_app_registered_at_path(&apps_path));

    // 2. Unregister when file doesn't exist should be harmless Ok(())
    assert!(unregister_app_at_path(&apps_path).is_ok());

    // 3. Registering creates apps.json and adds the entry
    let res = register_app_at_path(&apps_path);
    assert!(res.is_ok(), "Registration must succeed in sandbox: {:?}", res);
    assert!(apps_path.exists());
    assert!(is_app_registered_at_path(&apps_path));

    // Assert registered app entry has "cmd" with "--big-picture"
    let content = std::fs::read_to_string(&apps_path).unwrap();
    let val: serde_json::Value = serde_json::from_str(&content).unwrap();
    let apps = val.get("apps").unwrap().as_array().unwrap();
    let dlss_app = apps.iter().find(|a| a.get("name").and_then(|n| n.as_str()) == Some("DLSS Studio")).unwrap();
    let cmd_str = dlss_app.get("cmd").and_then(|c| c.as_str()).unwrap();
    assert!(cmd_str.contains("--big-picture"), "cmd must contain --big-picture argument");
    assert_eq!(dlss_app.get("auto-detach").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(dlss_app.get("wait-all").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(dlss_app.get("exit-timeout").and_then(|v| v.as_i64()), Some(5));

    // 4. Registering again should update the existing entry without duplicating
    let res2 = register_app_at_path(&apps_path);
    assert!(res2.is_ok());
    let content2 = std::fs::read_to_string(&apps_path).unwrap();
    let val2: serde_json::Value = serde_json::from_str(&content2).unwrap();
    let apps2 = val2.get("apps").unwrap().as_array().unwrap();
    assert_eq!(apps2.len(), 1, "Must not create duplicate entries for DLSS Studio");

    // 5. Unregister removes the entry
    let unreg = unregister_app_at_path(&apps_path);
    assert!(unreg.is_ok());
    assert!(!is_app_registered_at_path(&apps_path));

    // 6. Top-level calls run safely
    let _ = is_app_registered();
    let _ = unregister_app();
}

#[test]
fn test_apps_json_invalid_structure_and_error_handling() {
    let temp = crate::common::TempDir::new("vibepollo_errors");
    let apps_path = temp.join("apps.json");

    // 1. Invalid JSON in is_app_registered_at_path
    std::fs::write(&apps_path, b"NOT_JSON").unwrap();
    assert!(!is_app_registered_at_path(&apps_path));

    // 2. Invalid JSON in register_app_at_path
    assert!(register_app_at_path(&apps_path).is_err());

    // 3. Invalid JSON in unregister_app_at_path
    assert!(unregister_app_at_path(&apps_path).is_err());

    // 4. Missing "apps" key in JSON
    std::fs::write(&apps_path, r#"{"version": 2}"#).unwrap();
    assert!(!is_app_registered_at_path(&apps_path));
    assert!(register_app_at_path(&apps_path).is_err());

    // 5. Unregister when app already not present
    let initial_valid = r#"{"apps":[{"name":"OtherApp","uuid":"111"}],"version":2}"#;
    std::fs::write(&apps_path, initial_valid).unwrap();
    let unreg_noop = unregister_app_at_path(&apps_path);
    assert!(unreg_noop.is_ok());
}

#[test]
fn test_apps_json_legacy_uuid_migration() {
    let temp = crate::common::TempDir::new("vibepollo_legacy");
    let apps_path = temp.join("apps.json");

    // Initial state with legacy v1 UUID
    let legacy_json = r#"{
        "apps": [
            {
                "name": "DLSS 5 Swapper",
                "uuid": "4C640001-A480-4D56-9132-DLSS5STUDIO1"
            }
        ],
        "version": 2
    }"#;
    std::fs::write(&apps_path, legacy_json).unwrap();

    // Registering should recognize the legacy UUID and update it to DLSS Studio
    let reg = register_app_at_path(&apps_path);
    assert!(reg.is_ok());

    let content = std::fs::read_to_string(&apps_path).unwrap();
    let val: serde_json::Value = serde_json::from_str(&content).unwrap();
    let apps = val.get("apps").unwrap().as_array().unwrap();
    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].get("uuid").and_then(|u| u.as_str()), Some(DLSS_STUDIO_UUID));
    assert_eq!(apps[0].get("name").and_then(|n| n.as_str()), Some(DLSS_STUDIO_APP_NAME));

    // Reset with legacy UUID and test unregister
    std::fs::write(&apps_path, legacy_json).unwrap();
    let unreg = unregister_app_at_path(&apps_path);
    assert!(unreg.is_ok());

    let content_after = std::fs::read_to_string(&apps_path).unwrap();
    let val_after: serde_json::Value = serde_json::from_str(&content_after).unwrap();
    let apps_after = val_after.get("apps").unwrap().as_array().unwrap();
    assert!(apps_after.is_empty(), "Unregister must purge legacy UUID entry");
}

#[test]
fn test_register_app_system_call_and_invalid_write_path() {
    // 1. Top-level register_app call
    let _ = register_app();

    // 2. Invalid path in write_apps_json triggers non-permission I/O error
    let invalid_res = write_apps_json(std::path::Path::new(""), "content");
    assert!(invalid_res.is_err());
}


