use dlss_studio::core::mfg_unlock::configure_mfg_unlock_ini;

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
