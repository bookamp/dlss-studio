use dlss_studio::core::i18n::*;

#[test]
fn test_supported_languages_metadata() {
    assert_eq!(SUPPORTED_LANGS.len(), 14);
    for lang in SUPPORTED_LANGS {
        assert!(!lang.code.is_empty());
        assert!(!lang.label.is_empty());
        assert!(!lang.native.is_empty());
        if lang.code == "ar" {
            assert_eq!(lang.dir, "rtl");
        } else {
            assert_eq!(lang.dir, "ltr");
        }
    }
}

#[test]
fn test_bp_options_screen_japanese_localization() {
    assert_eq!(t("ja", "bp_select_deployment_profile"), "展開プロファイルの選択:");
    assert_eq!(t("ja", "bp_feature_tuning_title"), "機能チューニングと互換性:");
    assert_eq!(t("ja", "bp_profile_vanilla"), "バニラ（元のバックアップを復元）");
    assert_eq!(t("ja", "bp_vanilla_no_tuning"), "バニラにはチューニング パラメータは不要です。");
    assert_eq!(t("ja", "bp_badge_active"), "● 有効");
    assert_eq!(t("ja", "bp_badge_mfg_active"), "● 4X MFG 有効");
    assert_eq!(t("ja", "badge_no_dlss"), "DLSSなし");
    assert_eq!(t("ja", "bp_addon_label"), "アドオン");
    assert_eq!(t("ja", "advisory_vanilla_title"), "🛡️ バニラ（初期状態）");
    assert_eq!(t("ja", "advisory_compatible_title"), "✅ ルートの互換性を確認済み");

    let reason = "Missing Native DLSS-G: Injected 4x Multi-Frame Generation requires native DLSS 3 Frame Generation or Vulkan Streamline.";
    assert_eq!(translate_advisory_reason("ja", reason), "ネイティブDLSS-G欠如: 4x MFGにはネイティブDLSS 3 Frame Generationが必要です。");

    let rec = "DLSS 5 Feeder route provides generic frame interception and image reconstruction for this game.";
    assert_eq!(translate_advisory_recommendation("ja", rec), "DLSS 5 フィーダールートは、このゲームに対して汎用的なフレームインターセプトと画像再構成を提供します。");
}

#[test]
fn test_uninstaller_and_setup_button_translations() {
    // English
    assert_eq!(t("en", "uninstall_btn_cancel"), "Cancel");
    assert_eq!(t("en", "uninstall_btn_confirm"), "Uninstall");
    assert_eq!(t("en", "setup_btn_install"), "Install Now");
    assert_eq!(t("en", "setup_btn_install_now"), "Install Now");
    assert_eq!(t("en", "uninstall_confirm_desc"), "Are you sure you want to remove DLSS 5 Studio from your computer?");
    assert_eq!(t("en", "uninstall_clean_appdata_desc"), "Original game backups in your game folders remain untouched.");

    // German
    assert_eq!(t("de", "uninstall_btn_cancel"), "Abbrechen");
    assert_eq!(t("de", "uninstall_btn_confirm"), "Deinstallieren");
    assert_eq!(t("de", "setup_btn_install_now"), "Jetzt installieren");

    // French
    assert_eq!(t("fr", "uninstall_btn_cancel"), "Annuler");
    assert_eq!(t("fr", "uninstall_btn_confirm"), "Désinstaller");

    // Spanish
    assert_eq!(t("es", "uninstall_btn_cancel"), "Cancelar");
    assert_eq!(t("es", "uninstall_btn_confirm"), "Desinstalar");

    // Japanese
    assert_eq!(t("ja", "uninstall_btn_cancel"), "キャンセル");
    assert_eq!(t("ja", "uninstall_btn_confirm"), "アンインストール");

    // Simplified Chinese
    assert_eq!(t("zh", "uninstall_btn_cancel"), "取消");
    assert_eq!(t("zh", "uninstall_btn_confirm"), "卸载");

    // Korean
    assert_eq!(t("ko", "uninstall_btn_cancel"), "취소");
    assert_eq!(t("ko", "uninstall_btn_confirm"), "제거");

    // Russian
    assert_eq!(t("ru", "uninstall_btn_cancel"), "Отмена");
    assert_eq!(t("ru", "uninstall_btn_confirm"), "Удалить");

    // Italian
    assert_eq!(t("it", "uninstall_btn_cancel"), "Annulla");
    assert_eq!(t("it", "uninstall_btn_confirm"), "Disinstalla");

    // Polish
    assert_eq!(t("pl", "uninstall_btn_cancel"), "Anuluj");
    assert_eq!(t("pl", "uninstall_btn_confirm"), "Odinstaluj");

    // Turkish
    assert_eq!(t("tr", "uninstall_btn_cancel"), "İptal");
    assert_eq!(t("tr", "uninstall_btn_confirm"), "Kaldır");

    // Arabic
    assert_eq!(t("ar", "uninstall_btn_cancel"), "إلغاء");
    assert_eq!(t("ar", "uninstall_btn_confirm"), "إلغاء التثبيت");

    // Hindi
    assert_eq!(t("hi", "uninstall_btn_cancel"), "रद्द करें");
    assert_eq!(t("hi", "uninstall_btn_confirm"), "अनइंस्टॉल करें");
}
