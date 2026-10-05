use dlss_studio::core::ini::*;

#[test]
fn test_get_ini_and_set_ini() {
    let original = "[General]\nTarget=None\n[DlssNr]\nEnabled=false\nStyle=0\n";
    let val = get_ini(original, "DlssNr", "Enabled");
    assert_eq!(val, Some("false".to_string()));

    let updated = set_ini(original, "DlssNr", "Enabled", "true");
    assert_eq!(get_ini(&updated, "DlssNr", "Enabled"), Some("true".to_string()));
    assert_eq!(get_ini(&updated, "DlssNr", "Style"), Some("0".to_string()));
}

#[test]
fn test_set_ini_adds_section_if_missing() {
    let original = "[General]\nFoo=Bar\n";
    let updated = set_ini(original, "NewSection", "Key", "Val");
    assert!(updated.contains("[NewSection]"));
    assert_eq!(get_ini(&updated, "NewSection", "Key"), Some("Val".to_string()));
}

#[test]
fn test_configure_section() {
    let original = "[General]\nFoo=Bar\n\n[SectionA]\nKey1=Old1\nKey2=Old2\n";
    let pairs = [("Key1", "New1"), ("Key3", "New3")];
    let updated = configure_section(original, "SectionA", &pairs);
    assert_eq!(get_ini(&updated, "SectionA", "Key1"), Some("New1".to_string()));
    assert_eq!(get_ini(&updated, "SectionA", "Key3"), Some("New3".to_string()));
}

#[test]
fn test_remove_section() {
    let original = "[General]\nFoo=Bar\n\n[ToRemove]\nKey=Val\n\n[Keep]\nStay=Yes\n";
    let updated = remove_section(original, "ToRemove");
    assert!(!updated.contains("[ToRemove]"));
    assert!(updated.contains("[Keep]"));
    assert!(updated.contains("[General]"));
}

#[test]
fn test_mfg_multiplier_values() {
    let out_4x = configure_mfg_unlock_ini("", Some(4));
    assert!(out_4x.contains("ForceMultiplier=4"));
    assert!(out_4x.contains("MaxCount=4"));

    let out_2x = configure_mfg_unlock_ini("", Some(2));
    assert!(out_2x.contains("ForceMultiplier=2"));

    let out_3x = configure_mfg_unlock_ini("", Some(3));
    assert!(out_3x.contains("ForceMultiplier=3"));

    let out_1x = configure_mfg_unlock_ini("", Some(1));
    assert!(out_1x.contains("ForceMultiplier=0"));

    let out_default = configure_mfg_unlock_ini("", None);
    assert!(out_default.contains("ForceMultiplier=4"));
}

#[test]
fn test_mfg_unlock_ini_section_replacement() {
    let existing = "[General]\nFoo=Bar\n\n[RenoDX.MFGUnlock]\nForceMultiplier=2\nOld=1\n\n[NextSection]\nKey=Val";
    let updated = configure_mfg_unlock_ini(existing, Some(4));
    assert!(updated.contains("[General]"));
    assert!(updated.contains("[NextSection]"));
    assert!(updated.contains("ForceMultiplier=4"));
    assert!(!updated.contains("Old=1"));
}
