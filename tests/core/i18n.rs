use dlss_studio::core::i18n::*;

#[test]
fn test_translations() {
    let langs = ["en", "de", "es", "fr", "it", "pt", "ru", "zh", "ja", "ko", "pl", "tr", "ar", "unknown"];
    let keys = [
        "nav_home", "nav_games", "nav_addons", "nav_settings", "restore", "install",
        "addons_hint", "addon_renodx_desc", "addon_dlss5_d3d12_fix_desc", "addon_mfg_desc", "addon_feeder_desc",
        "settings_appearance", "settings_system", "settings_scanners", "settings_lang",
        "sheet_config", "sheet_deploy", "sheet_open_folder", "col_date", "col_game",
        "unknown_key_fallback"
    ];
    for l in &langs {
        for k in &keys {
            let res = t(l, k);
            assert!(!res.is_empty());
        }
    }
    assert_eq!(t("en", "unknown_key_fallback"), "unknown_key_fallback");
    assert_eq!(t("de", "restore"), "Originale wiederherstellen");
    assert_eq!(t("ar", "restore"), "استعادة الملفات الأصلية");
    assert_eq!(t("zh", "sheet_deploy"), "一键部署 / 更新注入");
}

#[test]
fn test_format_log_entry_multilingual_substitution() {
    let raw = "@{log_library_loaded|7}";
    assert_eq!(format_log_entry("en", raw), "Library loaded: 7 games in cache");
    assert_eq!(format_log_entry("fr", raw), "Bibliothèque chargée : 7 jeux en cache");
    assert_eq!(format_log_entry("de", raw), "Bibliothek geladen: 7 Spiele im Cache");
    assert_eq!(format_log_entry("ar", raw), "تم تحميل المكتبة: 7 ألعاب في الذاكرة المؤقتة");
    assert_eq!(format_log_entry("zh", raw), "游戏库已加载：缓存中存在 7 款游戏");
    assert_eq!(format_log_entry("hi", raw), "लाइब्रेरी लोड हुई: 7 गेम कैश में");

    let raw_toggle = "@{log_addon_status|builtin:renodx|@log_addon_activated}";
    assert_eq!(format_log_entry("en", raw_toggle), "Add-on builtin:renodx activated");
    assert_eq!(format_log_entry("fr", raw_toggle), "Extension builtin:renodx activée");
    assert_eq!(format_log_entry("de", raw_toggle), "Erweiterung builtin:renodx aktiviert");

    let raw_restore = "@{log_restore_start|Left 4 Dead}";
    assert_eq!(format_log_entry("en", raw_restore), "Restoring vanilla files for Left 4 Dead...");
    assert_eq!(format_log_entry("de", raw_restore), "Originaldateien für Left 4 Dead wiederherstellen...");
    assert_eq!(format_log_entry("zh", raw_restore), "正在恢复 Left 4 Dead 的原始游戏文件...");

    let raw_restore_ok = "@{log_restore_success}";
    assert_eq!(format_log_entry("en", raw_restore_ok), "Original game files restored successfully");
    assert_eq!(format_log_entry("de", raw_restore_ok), "Originale Spieledateien erfolgreich wiederhergestellt");
    assert_eq!(format_log_entry("es", raw_restore_ok), "Archivos originales del juego restaurados con éxito");

    let plain = "Unformatted plain text diagnostic message";
    assert_eq!(format_log_entry("fr", plain), plain);

    assert_eq!(format_log_entry("en", "Ready"), "Ready");
    assert_eq!(format_log_entry("zh", "Ready"), "就绪");
    assert_eq!(format_log_entry("de", "Ready"), "Bereit");
    assert_eq!(format_log_entry("ja", "Ready"), "準備完了");
}

#[test]
fn test_all_new_dictionary_keys_coverage() {
    let langs = ["en", "de", "es", "fr", "it", "pt", "ru", "zh", "ja", "ko", "pl", "tr", "ar", "hi"];
    let test_keys = [
        "status_ready",
        "sheet_detected_modules",
        "module_dlss_sr",
        "module_dlss_fg",
        "module_dlss_rr",
        "module_fsr_fg",
        "module_sl_dlss",
        "module_sl_fg",
        "module_sl_core",
        "module_sl_reflex",
        "module_optiscaler",
        "module_generic_dll",
        "tooltip_toggle_modules",
        "log_backup_saved",
        "log_clean_purged",
        "log_clean_removed",
        "log_clean_start",
        "log_copy_deployed",
        "log_deploy_start",
        "log_restore_no_backup",
        "log_restore_start",
        "log_restore_success",
        "log_routing_target",
        "log_vulkan_registered",
        "log_write_configured",
        "log_addon_activated",
        "log_addon_deactivated",
        "log_addon_imported",
        "log_addon_status",
        "log_custom_cover_updated",
        "log_download_error",
        "log_found_games",
        "log_game_launched",
        "log_game_launched_modded",
        "log_game_launched_vanilla",
        "log_library_loaded",
        "log_library_ready",
        "log_overlay_client_pid",
        "log_overlay_connected",
        "log_overlay_disconnected",
        "log_overlay_initialized",
        "log_scanning_background",
        "col_path",
        "history_action_clean",
        "history_action_install",
        "history_action_install_feeder",
        "history_action_install_native",
        "history_action_install_optiscaler",
        "history_action_restore",
        "history_changes_format",
        "history_date_recently",
        "about_github_repo",
        "about_releases",
        "about_report_issue",
        "btn_clear_log",
        "btn_open_log",
        "home_activity",
        "meta_native_backend",
        "settings_add_folder",
        "tab_games",
        "tag_mandatory_core",
        "tooltip_open_logfile",
        "tooltip_switch_dark",
        "tooltip_switch_light",
        "feeder_label_dx11",
        "feeder_label_dx12",
        "feeder_label_general",
        "feeder_label_opengl",
        "feeder_label_vulkan",
        "about_desc_main",
        "about_desc_sub",
        "btn_add",
        "btn_copy_history",
        "btn_save",
        "dlg_add_addon_title",
        "dlg_field_desc",
        "dlg_field_name",
        "dlg_field_tag",
        "dlg_hint_notes",
        "dlg_placeholder_tag",
        "feature_mfg_desc",
        "feature_mfg_dx11_note",
        "feature_mfg_general_note",
        "feature_mfg_tag",
        "feature_mfg_title",
        "feature_nr_style_desc",
        "feature_nr_style_label",
        "feature_nr_style_tag",
        "feature_nr_style_title",
        "feature_presr_desc",
        "feature_presr_passes",
        "feature_presr_tag",
        "feature_presr_title",
        "history_mods_cleaned",
        "history_orig_restored",
        "hotkey_dialog_title",
        "opt_dlss5_feeder",
        "opt_native_dlss",
        "opt_native_dlss_bp",
        "opt_optiscaler_dlssnr",
        "preview_char_mask",
        "preview_char_structure",
        "preview_dialog_title",
        "preview_diffuse_white",
        "preview_dlss_on",
        "preview_footer",
        "preview_global_controls",
        "preview_interactive_tag",
        "preview_local_tone",
        "preview_model_a",
        "preview_model_b",
        "preview_model_c",
        "preview_more_controls",
        "preview_motion_x",
        "preview_motion_y",
        "preview_nr_style",
        "preview_overall_intensity",
        "preview_structure_intensity",
        "preview_tone_intensity",
        "settings_hidden_count_suffix",
        "sheet_backend_restriction",
        "tag_custom",
        "theme_create_color",
        "theme_create_hint",
        "theme_create_name",
        "theme_create_title",
        "theme_preview_simulated",
        "tooltip_activate_addon",
        "tooltip_deactivate_addon",
        "activity_log",
        "addon_optiscaler_desc",
        "addon_reshade_desc",
        "addon_streamline_desc",
        "addon_dgvoodoo_desc",
        "backend_reshade_default",
        "badge_no_dlss",
        "badge_ready_dlss5",
        "btn_apply_theme",
        "btn_cancel",
        "btn_clean_mods",
        "btn_clear",
        "btn_close",
        "btn_copy_log",
        "btn_execution_log",
        "btn_open_log_file",
        "btn_open_desktop_log",
        "btn_open_bp_log",
        "btn_save_apply",
        "btn_working",
        "diag_ada_arch",
        "diag_ada_standard",
        "diag_ada_supported",
        "diag_primary_gpu",
        "diag_vram",
        "filter_all",
        "filter_all_apis",
        "filter_all_status",
        "filter_dlss5_installed",
        "filter_has_dlss",
        "filter_installed",
        "filter_no_dlss",
        "filter_ready_dlss5",
        "games_no_matches",
        "games_no_matches_hint",
        "home_browse_folder",
        "home_or",
        "home_play",
        "home_recent_games",
        "home_recents_empty",
        "home_view_all",
        "label_source",
        "meta_full_addon_support",
        "meta_presr_multipass",
        "meta_sha256_verified",
        "meta_streamline_interposer",
        "settings_autoscan",
        "settings_autoscan_desc",
        "settings_hidden_games",
        "settings_lib_config",
        "settings_managed_folders",
        "settings_managed_folders_desc",
        "settings_none_configured",
        "settings_reset",
        "settings_unhide_all",
        "sheet_anticheat_desc",
        "sheet_anticheat_title",
        "sheet_backend_label",
        "sheet_executable",
        "sheet_launch_game",
        "sheet_route_label",
        "spec_addon",
        "spec_architecture",
        "spec_frame_generation",
        "spec_installed_backend",
        "spec_presr_passes",
        "spec_rendering_api",
        "spec_target_exe",
        "btn_retry_download",
        "status_download_failed",
        "status_downloading_components",
        "status_found_games",
        "status_queued",
        "status_ready",
        "status_scanning_folder",
        "status_scanning_launchers",
        "status_scanning_library",
        "tag_companion",
        "tag_graphics_hook",
        "tag_interposer",
        "tag_legacy_wrapper",
        "tag_neural_reconstruction",
        "tag_rtx40",
        "tag_universal_intercept",
        "toast_activity_copied",
        "toast_addon_registered",
        "toast_addon_removed",
        "toast_config_reset",
        "toast_history_copied",
        "tooltip_change_cover",
        "tooltip_click_change_cover",
        "tooltip_hide_game",
        "tooltip_launch_game",
        "tooltip_launch_game_modded",
        "tooltip_launch_game_vanilla",
        "tooltip_open_explorer",
        "tooltip_open_log_file",
        "tooltip_open_desktop_log",
        "tooltip_open_bp_log",
        "tooltip_remove_addon",
        "tooltip_remove_folder",
        "tooltip_rename_game",
        "tooltip_retry_download",
        "tooltip_save_name",
        "tooltip_toggle_theme",
        "val_fg_supported",
        "val_fg_unsupported",
        "val_none",
        "val_not_installed",
        "time_just_now",
        "time_minutes_ago",
        "time_hours_ago",
        "time_yesterday",
        "time_days_ago",
        "setup_title",
        "setup_existing_detected",
        "setup_existing_detected_generic",
        "setup_existing_help",
        "setup_tag_update",
        "setup_tag_uninstall",
        "setup_install_location",
        "setup_browse",
        "setup_warn_protected_install",
        "setup_system_prefs",
        "setup_pref_startup",
        "setup_pref_background",
        "setup_pref_desktop",
        "setup_advanced_options",
        "setup_storage_location",
        "setup_storage_help",
        "setup_warn_protected_storage",
        "setup_btn_install",
        "setup_btn_install_now",
        "setup_btn_update",
        "setup_btn_cancel",
        "setup_status_preparing",
        "setup_status_target",
        "setup_status_extracting",
        "setup_status_complete",
        "setup_complete_title",
        "setup_complete_desc",
        "setup_btn_launch",
        "setup_btn_finish",
        "setup_failed_title",
        "setup_btn_back",
        "setup_btn_close",
        "setup_btn_retry",
        "uninstall_confirm_title",
        "uninstall_confirm_desc",
        "uninstall_target_label",
        "uninstall_cleanup_opts",
        "uninstall_clean_appdata",
        "uninstall_clean_appdata_desc",
        "uninstall_btn_cancel",
        "uninstall_btn_confirm",
        "uninstall_status_preparing",
        "uninstall_status_closing",
        "uninstall_status_shortcuts",
        "uninstall_status_purging",
        "uninstall_status_complete",
        "uninstall_complete_title",
        "uninstall_complete_desc",
        "uninstall_failed_title",
        "download_checking",
        "download_connecting",
        "download_downloading",
        "download_retrying",
        "download_verifying",
        "val_not_present",
        "settings_vibepollo_label",
        "settings_vibepollo_desc",
        "settings_vibepollo_detected",
        "settings_vibepollo_not_detected",
        "settings_vibe_multiple_detected",
        "settings_vibe_sync_all_desc",
        "settings_vibe_badge_running",
        "settings_vibe_badge_uac",
        "settings_vibe_badge_registered",
        "settings_vibe_desc_uac_notice",
        "settings_vibe_desc_not_detected",
        "toast_vibepollo_added",
        "toast_vibepollo_removed",
        "toast_vibepollo_uac_cancelled",
        "bp_store_label",
        "bp_btn_desktop",
        "bp_options_presets",
        "bp_card_presets_hint",
        "bp_apply_and_play",
        "bp_hud_apply",
        "bp_hud_applying",
        "bp_hud_applied",
        "bp_launching_prefix",
        "bp_returning_to_bp",
        "bp_hud_toggle_option",
        "bp_hud_cycle_value",
        "bp_exit_title",
        "bp_exit_desc",
        "bp_exit_btn_desktop",
        "bp_exit_btn_cancel",
        "bp_hud_select",
        "bp_hud_back",
        "bp_hud_play",
        "bp_hud_switch_store",
        "bp_select_deployment_profile",
        "bp_feature_tuning_title",
        "bp_profile_vanilla",
        "bp_tag_recommended",
        "bp_badge_active",
        "bp_badge_mfg_active",
        "bp_badge_optiscaler_active",
        "bp_badge_dlss_active",
        "bp_badge_unpatched_vanilla",
        "bp_addon_label",
        "bp_target_executable_title",
        "bp_target_executable_label",
        "bp_target_executable_desc",
        "bp_mfg_mult_1x",
        "bp_mfg_mult_4x",
        "sheet_mfg_multiplier",
        "advisory_tag_warn",
        "advisory_warning_high",
        "advisory_intro_restrictions",
        "advisory_vanilla_title",
        "advisory_vanilla_desc",
        "advisory_compatible_title",
        "advisory_compatible_desc",
        "bp_vanilla_no_tuning",
        "bp_vanilla_deploy_hint",
        "advisory_reason_32bit",
        "advisory_reason_dx11_interop",
        "advisory_reason_no_native_upscaler",
        "advisory_reason_mod_dlss",
        "advisory_reason_legacy_api",
        "advisory_reason_missing_native_dlss",
        "advisory_reason_non_dx12_native",
        "advisory_reason_dx11_mfg",
        "advisory_reason_missing_dlssg",
        "advisory_reason_hardware_rtx40",
        "advisory_rec_feeder_generic",
        "advisory_rec_feeder_api",
        "advisory_rec_feeder_sr_dlaa",
    ];
    for k in &test_keys {
        for l in &langs {
            let res = t(l, k);
            assert!(!res.is_empty(), "Key '{}' returned empty string for lang '{}'", k, l);
            assert_ne!(res, *k, "Key '{}' was not localized for lang '{}'", k, l);
        }
    }
    assert_eq!(t("de", "status_ready"), "Bereit");
    assert_eq!(t("fr", "status_ready"), "Prêt");
    assert_eq!(t("es", "status_ready"), "Listo");
    assert_eq!(t("zh", "status_ready"), "就绪");
    assert_eq!(t("ar", "status_ready"), "جاهز");
    assert_eq!(t("hi", "status_ready"), "तैयार");
}

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

#[test]
fn test_download_progress_localization() {
    use dlss_studio::core::downloader::{format_download_progress, DownloadProgress, DownloadStage};

    // 1. Checking stage
    let prog_check = DownloadProgress::checking();
    assert_eq!(format_download_progress("en", &prog_check), "Checking components...");
    assert_eq!(format_download_progress("de", &prog_check), "Komponenten werden überprüft...");
    assert_eq!(format_download_progress("fr", &prog_check), "Vérification des composants...");
    assert_eq!(format_download_progress("zh", &prog_check), "正在检查核心组件...");

    // 2. Connecting stage
    let mut prog_conn = DownloadProgress::default();
    prog_conn.component_name = "Streamline Runtime v2.14.1".to_string();
    prog_conn.stage = DownloadStage::Connecting;
    assert_eq!(format_download_progress("en", &prog_conn), "Connecting to Streamline Runtime v2.14.1...");
    assert_eq!(format_download_progress("de", &prog_conn), "Verbindung zu Streamline Runtime v2.14.1 wird hergestellt...");
    assert_eq!(format_download_progress("es", &prog_conn), "Conectando a Streamline Runtime v2.14.1...");

    // 3. Downloading stage with byte progress
    let mut prog_dl = DownloadProgress::default();
    prog_dl.component_name = "Streamline Runtime v2.14.1".to_string();
    prog_dl.downloaded_bytes = 106_731_930; // ~101.79 MB
    prog_dl.total_bytes = Some(144_021_914); // ~137.35 MB
    prog_dl.stage = DownloadStage::Downloading;
    assert_eq!(
        format_download_progress("en", &prog_dl),
        "Downloading Streamline Runtime v2.14.1... (101.79 MB / 137.35 MB)"
    );
    assert_eq!(
        format_download_progress("de", &prog_dl),
        "Streamline Runtime v2.14.1 wird heruntergeladen... (101.79 MB / 137.35 MB)"
    );
    assert_eq!(
        format_download_progress("fr", &prog_dl),
        "Téléchargement de Streamline Runtime v2.14.1... (101.79 MB / 137.35 MB)"
    );
    assert_eq!(
        format_download_progress("ja", &prog_dl),
        "Streamline Runtime v2.14.1 をダウンロード中... (101.79 MB / 137.35 MB)"
    );

    // 4. Verifying stage
    let mut prog_ver = DownloadProgress::default();
    prog_ver.stage = DownloadStage::Verifying;
    assert_eq!(format_download_progress("en", &prog_ver), "Verifying checksum...");
    assert_eq!(format_download_progress("de", &prog_ver), "Prüfsumme wird überprüft...");
    assert_eq!(format_download_progress("zh", &prog_ver), "正在校验完整性与哈希值...");

    // 5. Retrying stage
    let mut prog_retry = DownloadProgress::default();
    prog_retry.component_name = "ReShade 6.8.0".to_string();
    prog_retry.stage = DownloadStage::Retrying(2);
    assert_eq!(format_download_progress("en", &prog_retry), "Retrying ReShade 6.8.0 (attempt 2/3)...");
    assert_eq!(format_download_progress("de", &prog_retry), "ReShade 6.8.0 wird wiederholt (Versuch 2/3)...");
}
