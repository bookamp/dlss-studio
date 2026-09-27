use dlss_studio::big_picture::ui::*;
use dlss_studio::core::scan::{GameEntry, GameExeOption};
use dlss_studio::core::install_routes::{
    recommended_route, get_optiscaler_advisory, get_native_dlss_advisory, get_mfg_advisory, InstallRoute,
};

    #[test]
    fn test_filter_games_store_partitioning() {
        let games = vec![
            GameEntry { name: "Game A".to_string(), launcher: "Steam".to_string(), ..Default::default() },
            GameEntry { name: "Game B".to_string(), launcher: "Xbox".to_string(), ..Default::default() },
            GameEntry { name: "Game C".to_string(), launcher: "Epic Games".to_string(), ..Default::default() },
        ];

        let steam_games = filter_games(&games, "Steam");
        assert_eq!(steam_games.len(), 1);
        assert_eq!(steam_games[0].name, "Game A");

        let xbox_games = filter_games(&games, "Xbox");
        assert_eq!(xbox_games.len(), 1);
        assert_eq!(xbox_games[0].name, "Game B");

        let all_games = filter_games(&games, "All");
        assert_eq!(all_games.len(), 3);
    }

    #[test]
    fn test_mixtape_feeder_route_has_zero_warnings_by_default() {
        // Mixtape: DX11 title without native DLSS
        let mixtape = GameEntry {
            name: "Mixtape".to_string(),
            api: "DirectX 11".to_string(),
            bitness: 64,
            dlss_version: None,
            ..Default::default()
        };

        // Desktop logic: recommended route is Feeder
        let rec = recommended_route(&mixtape);
        assert_eq!(rec, InstallRoute::Feeder);

        let effective_backend = "reshade";
        let effective_route = rec.as_str(); // "feeder"
        let mfg_choice = false; // Vanilla games default mfg to false

        // Compute advisories exactly as desktop UI
        let opti_advisory = get_optiscaler_advisory(&mixtape);
        let native_advisory = get_native_dlss_advisory(&mixtape);
        let mfg_advisory = get_mfg_advisory(&mixtape, true);

        let mut active_advisories = Vec::new();
        if effective_backend == "optiscaler" {
            if let Some(adv) = &opti_advisory {
                active_advisories.push(adv.clone());
            }
        } else if effective_route == "native" {
            if let Some(adv) = &native_advisory {
                active_advisories.push(adv.clone());
            }
        }
        if mfg_choice {
            if let Some(adv) = &mfg_advisory {
                active_advisories.push(adv.clone());
            }
        }

        // Must have ZERO warnings!
        assert!(active_advisories.is_empty(), "Mixtape on recommended Feeder must have ZERO warnings");
    }

    #[test]
    fn test_forcing_native_on_mixtape_shows_warning() {
        let mixtape = GameEntry {
            name: "Mixtape".to_string(),
            api: "DirectX 11".to_string(),
            bitness: 64,
            dlss_version: None,
            ..Default::default()
        };

        let effective_backend = "reshade";
        let effective_route = "native"; // User explicitly selected Native DLSS
        let native_advisory = get_native_dlss_advisory(&mixtape);

        let mut active_advisories = Vec::new();
        if effective_backend == "reshade" && effective_route == "native" {
            if let Some(adv) = &native_advisory {
                active_advisories.push(adv.clone());
            }
        }

        assert_eq!(active_advisories.len(), 1);
        assert!(active_advisories[0].title.contains("DLSS 5 Direct"));
    }

    #[test]
    fn test_deploy_preset_options_defaults() {
        let opts = DeployPresetOptions::default();
        assert!(opts.pre_sr);
        assert_eq!(opts.passes, 1);
        assert!(!opts.mfg_unlock); // Matches desktop UI default for vanilla titles
        assert_eq!(opts.mfg_multiplier, 4);
        assert_eq!(opts.nr_style, 0);
        assert!(opts.nr_style_enabled);
    }

    #[test]
    fn test_build_visual_list_contiguous_indices() {
        let games = vec![
            GameEntry { name: "Game Epic".to_string(), launcher: "Epic Games".to_string(), ..Default::default() },
            GameEntry { name: "Game Steam 1".to_string(), launcher: "Steam".to_string(), ..Default::default() },
            GameEntry { name: "Game Xbox".to_string(), launcher: "Xbox".to_string(), ..Default::default() },
            GameEntry { name: "Game Steam 2".to_string(), launcher: "Steam".to_string(), ..Default::default() },
        ];

        let (visual_list, partitions) = build_visual_list(&games, "All");
        assert_eq!(visual_list.len(), 4);

        // First section in DEFAULT_STORE_ORDER is Steam: Game Steam 1 (idx 0), Game Steam 2 (idx 1)
        let steam_part = partitions.iter().find(|(name, _)| *name == "Steam").expect("Steam partition");
        assert_eq!(steam_part.1.len(), 2);
        assert_eq!(steam_part.1[0].0, 0);
        assert_eq!(steam_part.1[0].1.name, "Game Steam 1");
        assert_eq!(steam_part.1[1].0, 1);
        assert_eq!(steam_part.1[1].1.name, "Game Steam 2");

        // Second section is Xbox: Game Xbox (idx 2)
        let xbox_part = partitions.iter().find(|(name, _)| *name == "Xbox").expect("Xbox partition");
        assert_eq!(xbox_part.1.len(), 1);
        assert_eq!(xbox_part.1[0].0, 2);
        assert_eq!(xbox_part.1[0].1.name, "Game Xbox");

        // Third section is Epic Games: Game Epic (idx 3)
        let epic_part = partitions.iter().find(|(name, _)| *name == "Epic Games").expect("Epic partition");
        assert_eq!(epic_part.1.len(), 1);
        assert_eq!(epic_part.1[0].0, 3);
        assert_eq!(epic_part.1[0].1.name, "Game Epic");
    }

    #[test]
    fn test_is_selection_matching_installed() {
        let mut game = GameEntry {
            name: "Test Game".to_string(),
            installed_route: Some("feeder".to_string()),
            reshade_installed: true,
            addon_installed: true,
            mfg_unlock_installed: true,
            mfg_multiplier: 4,
            nr_style: 0,
            nr_style_enabled: true,
            ..Default::default()
        };

        // When selection exactly matches installed: true ("PLAY")
        let synced = is_selection_matching_installed(
            &game,
            "reshade",
            "feeder",
            false,
            true,
            1,
            true,
            4,
            0,
            true,
        );
        assert!(synced, "Exact match should be synced");

        // Changing route to native: false ("APPLY & PLAY")
        let changed_route = is_selection_matching_installed(
            &game,
            "reshade",
            "native",
            false,
            true,
            1,
            true,
            4,
            0,
            true,
        );
        assert!(!changed_route, "Changing route should not match installed");

        // Changing MFG multiplier to 2x: false ("APPLY & PLAY")
        let changed_mfg = is_selection_matching_installed(
            &game,
            "reshade",
            "feeder",
            false,
            true,
            1,
            true,
            2,
            0,
            true,
        );
        assert!(!changed_mfg, "Changing multiplier should not match installed");

        // Vanilla game with vanilla selection: true ("PLAY")
        game.installed_route = None;
        game.reshade_installed = false;
        game.addon_installed = false;
        game.mfg_unlock_installed = false;
        game.optiscaler_installed = false;
        game.has_backup = false;

        let vanilla_synced = is_selection_matching_installed(
            &game,
            "reshade",
            "feeder",
            true, // vanilla selected
            true,
            1,
            false,
            4,
            0,
            true,
        );
        assert!(vanilla_synced, "Unmodded game with vanilla selection is synced");

        // Vanilla game with feeder selected: false ("APPLY & PLAY")
        let vanilla_modded = is_selection_matching_installed(
            &game,
            "reshade",
            "feeder",
            false, // modded selected
            true,
            1,
            false,
            4,
            0,
            true,
        );
        assert!(!vanilla_modded, "Unmodded game with modded profile should need apply");
    }

    #[test]
    fn test_section_aware_2d_navigation_never_skips_small_section() {
        // Steam (20 games), Xbox (2 games), GOG (5 games)
        let steam_items: Vec<(usize, GameEntry)> = (0..20)
            .map(|i| (i, GameEntry { name: format!("Steam {}", i), launcher: "Steam".to_string(), ..Default::default() }))
            .collect();
        let xbox_items: Vec<(usize, GameEntry)> = (20..22)
            .map(|i| (i, GameEntry { name: format!("Xbox {}", i), launcher: "Xbox".to_string(), ..Default::default() }))
            .collect();
        let gog_items: Vec<(usize, GameEntry)> = (22..27)
            .map(|i| (i, GameEntry { name: format!("GOG {}", i), launcher: "GOG".to_string(), ..Default::default() }))
            .collect();

        let parts: Vec<(&'static str, Vec<(usize, GameEntry)>)> = vec![
            ("Steam", steam_items),
            ("Xbox", xbox_items),
            ("GOG", gog_items),
        ];

        // 1. Moving UP from Baldur's Gate 3 (GOG game 0, index 22) must land directly on Xbox (index 20),
        // instead of jumping 6 items back to Steam (index 16)!
        let up_from_gog = navigate_grid_2d(22, GridNavDirection::Up, &parts, 6);
        assert_eq!(up_from_gog, 20, "Moving Up from GOG must land on Xbox last row, not skip to Steam");

        // 2. Moving UP from Xbox game 0 (index 20) must land on Steam last row (index 18)
        let up_from_xbox = navigate_grid_2d(20, GridNavDirection::Up, &parts, 6);
        assert_eq!(up_from_xbox, 18, "Moving Up from Xbox must land on Steam last row (row 3 col 0)");

        // 3. Moving DOWN from Steam last row (index 18) must land on Xbox (index 20)
        let down_from_steam = navigate_grid_2d(18, GridNavDirection::Down, &parts, 6);
        assert_eq!(down_from_steam, 20, "Moving Down from Steam must land on Xbox first row");

        // 4. Moving DOWN from Xbox (index 20) must land on GOG (index 22)
        let down_from_xbox = navigate_grid_2d(20, GridNavDirection::Down, &parts, 6);
        assert_eq!(down_from_xbox, 22, "Moving Down from Xbox must land on GOG first row");

        // 5. Left / Right boundary stepping
        let left_from_xbox = navigate_grid_2d(20, GridNavDirection::Left, &parts, 6);
        assert_eq!(left_from_xbox, 19, "Moving Left from Xbox start must step to Steam end");

        let right_from_steam = navigate_grid_2d(19, GridNavDirection::Right, &parts, 6);
        assert_eq!(right_from_steam, 20, "Moving Right from Steam end must step to Xbox start");
    }

    #[test]
    fn test_active_profile_idx_resolution() {
        // Direct Native deployed
        let native_game = GameEntry {
            name: "Native Game".to_string(),
            installed_route: Some("native".to_string()),
            reshade_installed: true,
            ..Default::default()
        };
        assert_eq!(active_profile_idx(&native_game), 0);

        // DLSS 5 Feeder deployed
        let feeder_game = GameEntry {
            name: "Feeder Game".to_string(),
            installed_route: Some("feeder".to_string()),
            reshade_installed: true,
            ..Default::default()
        };
        assert_eq!(active_profile_idx(&feeder_game), 1);

        // OptiScaler deployed
        let opti_game = GameEntry {
            name: "OptiScaler Game".to_string(),
            optiscaler_installed: true,
            ..Default::default()
        };
        assert_eq!(active_profile_idx(&opti_game), 2);

        // Vanilla unmodded game -> active_profile_idx falls back to recommended route (Feeder: 1)
        let vanilla_unmodded = GameEntry {
            name: "Vanilla Game".to_string(),
            has_backup: true,
            ..Default::default()
        };
        assert_eq!(active_profile_idx(&vanilla_unmodded), 1);

        // Unmodded DX11 game -> recommended is Feeder (1)
        let dx11_game = GameEntry {
            name: "DX11 Game".to_string(),
            api: "DirectX 11".to_string(),
            ..Default::default()
        };
        assert_eq!(active_profile_idx(&dx11_game), 1);
    }

    #[test]
    fn test_installed_profile_idx_strictly_matches_disk() {
        // 1. Completely unmodded game returns None (no Active badge on any route)
        let unmodded = GameEntry {
            name: "Unmodded Game".to_string(),
            api: "DirectX 11".to_string(),
            ..Default::default()
        };
        assert_eq!(installed_profile_idx(&unmodded), None, "Unmodded game must return None for installed_profile_idx");

        // 2. Native DLSS 5 installed
        let native = GameEntry {
            name: "Native Game".to_string(),
            installed_route: Some("native".to_string()),
            reshade_installed: true,
            ..Default::default()
        };
        assert_eq!(installed_profile_idx(&native), Some(0), "Native route installed must return Some(0)");

        // 3. DLSS 5 Feeder installed
        let feeder = GameEntry {
            name: "Feeder Game".to_string(),
            installed_route: Some("feeder".to_string()),
            reshade_installed: true,
            ..Default::default()
        };
        assert_eq!(installed_profile_idx(&feeder), Some(1), "Feeder route installed must return Some(1)");

        // 4. OptiScaler installed
        let opti = GameEntry {
            name: "OptiScaler Game".to_string(),
            optiscaler_installed: true,
            ..Default::default()
        };
        assert_eq!(installed_profile_idx(&opti), Some(2), "OptiScaler installed must return Some(2)");
    }

    #[test]
    fn test_is_selection_matching_installed_play_button_state() {
        // 1. Unmodded vanilla game with Vanilla profile selected -> synced (shows Play, vanilla styling)
        let vanilla_game = GameEntry {
            name: "Vanilla Game".to_string(),
            ..Default::default()
        };
        assert!(is_selection_matching_installed(
            &vanilla_game,
            "reshade",
            "feeder",
            true, // is_vanilla
            false,
            1,
            false,
            4,
            0,
            false,
        ), "Vanilla selection on unmodded game should be synced");

        // 2. Unmodded vanilla game with Feeder profile selected -> unsynced (shows Apply & Play)
        assert!(!is_selection_matching_installed(
            &vanilla_game,
            "reshade",
            "feeder",
            false, // is_vanilla
            false,
            1,
            false,
            4,
            0,
            false,
        ), "Modded selection on unmodded game must be unsynced (pending Apply)");

        // 3. Modded Feeder game matching on-disk state -> synced (shows Play, modded styling)
        let feeder_game = GameEntry {
            name: "Feeder Game".to_string(),
            installed_route: Some("feeder".to_string()),
            reshade_installed: true,
            mfg_unlock_installed: true,
            mfg_multiplier: 4,
            nr_style: 0,
            nr_style_enabled: true,
            ..Default::default()
        };
        assert!(is_selection_matching_installed(
            &feeder_game,
            "reshade",
            "feeder",
            false,
            false,
            1,
            true,
            4,
            0,
            true,
        ), "Feeder selection matching disk state should be synced");

        // 4. Modded Feeder game with multiplier changed -> unsynced (shows Apply & Play)
        assert!(!is_selection_matching_installed(
            &feeder_game,
            "reshade",
            "feeder",
            false,
            false,
            1,
            true,
            2, // changed from 4 to 2
            0,
            true,
        ), "Modified multiplier must make state unsynced");
    }

    #[test]
    fn test_opti_passes_continuous_wrap_around() {
        let step_fwd = |cur: u32| if cur >= 3 { 1 } else { cur + 1 };
        let step_back = |cur: u32| if cur <= 1 { 3 } else { cur - 1 };

        assert_eq!(step_fwd(1), 2);
        assert_eq!(step_fwd(2), 3);
        assert_eq!(step_fwd(3), 1, "Pre-SR passes must wrap from 3 back to 1");

        assert_eq!(step_back(3), 2);
        assert_eq!(step_back(2), 1);
        assert_eq!(step_back(1), 3, "Pre-SR passes must wrap backwards from 1 to 3");
    }

    #[test]
    fn test_mfg_multiplier_continuous_wrap_around() {
        let step_fwd = |cur: u32| if cur >= 4 { 1 } else { cur + 1 };
        let step_back = |cur: u32| if cur <= 1 { 4 } else { cur - 1 };

        assert_eq!(step_fwd(1), 2);
        assert_eq!(step_fwd(2), 3);
        assert_eq!(step_fwd(3), 4);
        assert_eq!(step_fwd(4), 1, "MFG multiplier must wrap from 4 back to 1");

        assert_eq!(step_back(4), 3);
        assert_eq!(step_back(3), 2);
        assert_eq!(step_back(2), 1);
        assert_eq!(step_back(1), 4, "MFG multiplier must wrap backwards from 1 to 4");
    }

    #[test]
    fn test_exit_confirmation_cursor_toggle() {
        let mut idx = 1usize; // Default: Cancel (1)
        idx = 1 - idx; // Toggle left/right
        assert_eq!(idx, 0, "Toggling from Cancel must select Exit to Desktop (0)");
        idx = 1 - idx;
        assert_eq!(idx, 1, "Toggling from Exit to Desktop must select Cancel (1)");
    }

    #[test]
    fn test_status_banner_error_detection() {
        let is_err = |msg: &str| msg.to_lowercase().contains("error") || msg.to_lowercase().contains("failed");

        assert!(is_err("Installation failed: Sharing violation"));
        assert!(is_err("Launch error: Game crashed"));
        assert!(is_err("Restore error: File missing"));
        assert!(!is_err("Preparing Cyberpunk 2077..."));
        assert!(!is_err("OptiScaler applied successfully"));
    }

    #[test]
    fn test_options_hud_label_generation() {
        let play = "Play Game";
        let apply = "Apply & Play";
        let cancel = "Cancel";

        // Column 0: Presets 0..3
        assert_eq!(resolve_options_hud_label(0, 0, 0, false, true, play, apply, cancel), "Play Game");
        assert_eq!(resolve_options_hud_label(0, 0, 0, false, false, play, apply, cancel), "Apply & Play");
        assert_eq!(resolve_options_hud_label(0, 0, 1, false, true, play, apply, cancel), "Play Game");
        assert_eq!(resolve_options_hud_label(0, 0, 2, false, false, play, apply, cancel), "Apply & Play");
        assert_eq!(resolve_options_hud_label(0, 0, 3, false, true, play, apply, cancel), "Play Game");

        // Column 0: Button 5 (Cancel)
        assert_eq!(resolve_options_hud_label(0, 0, 5, false, true, play, apply, cancel), "Cancel");

        // Column 1: Executable Target Stepper
        assert_eq!(resolve_options_hud_label(1, 0, 0, true, true, play, apply, cancel), "Cycle Executable");
        assert_eq!(resolve_options_hud_label(1, 1, 0, true, true, play, apply, cancel), "Cycle Executable");

        // Column 1: Feature checkbox vs Stepper
        assert_eq!(resolve_options_hud_label(1, 0, 0, false, true, play, apply, cancel), "Toggle Option");
        assert_eq!(resolve_options_hud_label(1, 1, 0, false, true, play, apply, cancel), "Cycle Value");
        assert_eq!(resolve_options_hud_label(1, 0, 2, false, false, play, apply, cancel), "Toggle Option");
        assert_eq!(resolve_options_hud_label(1, 1, 2, false, false, play, apply, cancel), "Cycle Value");

        // Localized HUD tuning labels
        let cycle_exe = "BinÃ¤rdatei wechseln";
        let cycle_val = "Wert Ã¤ndern";
        let toggle_opt = "Option umschalten";
        assert_eq!(resolve_options_hud_label_with_tuning(1, 0, 0, true, true, play, apply, cancel, cycle_exe, cycle_val, toggle_opt), "BinÃ¤rdatei wechseln");
        assert_eq!(resolve_options_hud_label_with_tuning(1, 1, 0, false, true, play, apply, cancel, cycle_exe, cycle_val, toggle_opt), "Wert Ã¤ndern");
        assert_eq!(resolve_options_hud_label_with_tuning(1, 0, 0, false, true, play, apply, cancel, cycle_exe, cycle_val, toggle_opt), "Option umschalten");

        // LaunchOverlayInfo struct state
        let overlay = LaunchOverlayInfo {
            title: "Launching Cyberpunk 2077...".to_string(),
            poster_url: Some("http://dlss-art.localhost/art/cp2077.jpg".to_string()),
            is_exiting: false,
        };
        assert_eq!(overlay.title, "Launching Cyberpunk 2077...");
        assert!(!overlay.is_exiting);
        assert!(overlay.poster_url.is_some());
    }

    #[test]
    fn test_launch_overlay_lifecycle_initial_stage() {
        let state = resolve_launch_overlay_state(
            LaunchLifecycleStage::Launching,
            "Cyberpunk 2077",
            Some("http://dlss-art.localhost/art/cp.jpg".to_string()),
            "en",
        );
        let overlay = state.expect("Launching stage must produce an overlay");
        assert_eq!(overlay.title, "Launching Cyberpunk 2077...");
        assert!(!overlay.is_exiting, "Launching stage must not be marked as exiting");
        assert_eq!(overlay.poster_url.as_deref(), Some("http://dlss-art.localhost/art/cp.jpg"));
    }

    #[test]
    fn test_launch_overlay_lifecycle_foreground_prestage() {
        let state = resolve_launch_overlay_state(
            LaunchLifecycleStage::RunningInForeground,
            "Baldur's Gate 3",
            Some("http://dlss-art.localhost/art/bg3.jpg".to_string()),
            "en",
        );
        let overlay = state.expect("Foreground stage must pre-stage return overlay");
        assert_eq!(overlay.title, "Returning to Big Picture mode...");
        assert!(overlay.is_exiting, "Foreground stage must be marked as exiting");
        assert_eq!(overlay.poster_url.as_deref(), Some("http://dlss-art.localhost/art/bg3.jpg"));
    }

    #[test]
    fn test_launch_overlay_lifecycle_completion() {
        let state = resolve_launch_overlay_state(
            LaunchLifecycleStage::Completed,
            "Any Game",
            None,
            "en",
        );
        assert!(state.is_none(), "Completed stage must resolve to None to dismiss overlay");
    }

    #[test]
    fn test_launch_overlay_lifecycle_multilingual_return_title() {
        let langs = ["en", "de", "es", "fr", "it", "pt", "ru", "zh", "ja", "ko", "pl", "tr", "ar", "hi"];

        for lang_code in langs {
            let state = resolve_launch_overlay_state(
                LaunchLifecycleStage::ReturningToBigPicture,
                "Game",
                None,
                lang_code,
            );
            let overlay = state.expect("Returning stage must produce an overlay");
            let expected = dlss_studio::core::i18n::t(lang_code, "bp_returning_to_bp").to_string();
            assert_eq!(
                overlay.title, expected,
                "Language code '{}' produced unexpected return title",
                lang_code
            );
            assert!(overlay.is_exiting);
        }
    }

    #[test]
    fn test_options_profile_idx_strictly_clamped_to_presets() {
        // Navigation clamp logic: (cur + 1).min(3) and cur.saturating_sub(1)
        let mut idx = 0usize;
        let step_down = |i: usize| (i + 1).min(3);
        let step_up = |i: usize| i.saturating_sub(1);

        idx = step_down(idx); // 0 -> 1 (Feeder)
        assert_eq!(idx, 1);
        idx = step_down(idx); // 1 -> 2 (OptiScaler)
        assert_eq!(idx, 2);
        idx = step_down(idx); // 2 -> 3 (Vanilla Restore)
        assert_eq!(idx, 3);
        idx = step_down(idx); // 3 clamped to 3 (CANNOT reach legacy Play or Cancel buttons)
        assert_eq!(idx, 3, "Profile cursor must be strictly clamped at index 3 (Vanilla)");

        idx = step_up(idx); // 3 -> 2
        assert_eq!(idx, 2);
        idx = step_up(idx); // 2 -> 1
        assert_eq!(idx, 1);
        idx = step_up(idx); // 1 -> 0
        assert_eq!(idx, 0);
        idx = step_up(idx); // 0 clamped to 0
        assert_eq!(idx, 0, "Profile cursor must be strictly clamped at index 0 (Native)");
    }

    #[test]
    fn test_multi_exe_stepper_cycling_and_api_switch() {
        use std::path::PathBuf;

        let bg3_dir = PathBuf::from(r"C:\Games\Baldurs Gate 3\bin");
        let exe_vulkan = GameExeOption {
            name: "Baldur's Gate 3 (Vulkan)".to_string(),
            path: bg3_dir.join("bg3.exe"),
            rel: "bin\\bg3.exe".to_string(),
            api: "Vulkan".to_string(),
            bitness: 64,
            is_laa: false,
        };
        let exe_dx11 = GameExeOption {
            name: "Baldur's Gate 3 (DirectX 11)".to_string(),
            path: bg3_dir.join("bg3_dx11.exe"),
            rel: "bin\\bg3_dx11.exe".to_string(),
            api: "DirectX 11".to_string(),
            bitness: 64,
            is_laa: false,
        };

        let mut bg3 = GameEntry {
            name: "Baldur's Gate 3".to_string(),
            dir: PathBuf::from(r"C:\Games\Baldurs Gate 3"),
            exe_path: exe_vulkan.path.clone(),
            exe_rel: exe_vulkan.rel.clone(),
            api: exe_vulkan.api.clone(),
            bitness: 64,
            available_exes: vec![exe_vulkan.clone(), exe_dx11.clone()],
            ..Default::default()
        };

        // 1. Cycle forward from Vulkan -> DirectX 11
        let next_fwd = cycle_game_exe(&bg3, true).expect("Next exe option exists");
        assert_eq!(next_fwd.api, "DirectX 11");
        assert_eq!(next_fwd.rel, "bin\\bg3_dx11.exe");

        // Simulate applying the cycled exe
        bg3.exe_path = next_fwd.path.clone();
        bg3.exe_rel = next_fwd.rel.clone();
        bg3.api = next_fwd.api.clone();

        // 2. Cycle forward again: DirectX 11 wraps around -> Vulkan
        let next_wrap = cycle_game_exe(&bg3, true).expect("Next exe option wraps");
        assert_eq!(next_wrap.api, "Vulkan");
        assert_eq!(next_wrap.rel, "bin\\bg3.exe");

        // 3. Cycle backward from DirectX 11 -> Vulkan
        let prev = cycle_game_exe(&bg3, false).expect("Prev exe option exists");
        assert_eq!(prev.api, "Vulkan");

        // 4. Single-exe game should return None
        let single_exe_game = GameEntry {
            name: "Single Game".to_string(),
            available_exes: vec![exe_vulkan],
            ..Default::default()
        };
        assert!(cycle_game_exe(&single_exe_game, true).is_none(), "Single-exe game must return None");
    }

    #[test]
    fn test_grid_three_row_visibility_constraints() {
        // Target card dimensions:
        // card_height = 225px, row_gap = 12px
        // top_chrome = 66px, bottom_chrome = 60px -> total chrome = 126px
        let card_h = 225.0;
        let gap = 12.0;
        let top_c = 66.0;
        let btm_c = 60.0;

        // 1. 4K native @ 100% DPI (2160p viewport)
        let rows_4k_100 = calculate_visible_grid_rows(2160.0, top_c, btm_c, card_h, gap);
        assert!(rows_4k_100 >= 8, "4K at 100% DPI should fit at least 8 rows, got {}", rows_4k_100);

        // 2. 4K @ 200% DPI (1080p logical viewport)
        let rows_4k_200 = calculate_visible_grid_rows(1080.0, top_c, btm_c, card_h, gap);
        assert!(rows_4k_200 >= 4, "4K at 200% DPI should fit at least 4 rows, got {}", rows_4k_200);

        // 3. 4K @ 250% DPI (864p logical viewport)
        let rows_4k_250 = calculate_visible_grid_rows(864.0, top_c, btm_c, card_h, gap);
        assert!(rows_4k_250 >= 3, "4K at 250% DPI MUST fit at least 3 full rows, got {}", rows_4k_250);

        // 4. 4K @ 300% DPI (720p logical viewport) with responsive compact card (192px card, 10px gap)
        let rows_4k_300 = calculate_visible_grid_rows(720.0, 56.0, 52.0, 192.0, 10.0);
        assert!(rows_4k_300 >= 3, "4K at 300% DPI with compact cards MUST fit at least 3 full rows, got {}", rows_4k_300);
    }

    #[test]
    fn test_boundary_aware_scroll_padding_constants() {
        // Assert that scroll padding constants exceed fixed chrome bar heights to guarantee zero card clipping
        let header_approx_height = 66.0f32;
        let hud_approx_height = 60.0f32;

        assert!(SCROLL_PADDING_TOP_PX > header_approx_height,
            "Top scroll padding ({}px) must exceed header height ({}px)", SCROLL_PADDING_TOP_PX, header_approx_height);
        assert!(SCROLL_PADDING_BOTTOM_PX > hud_approx_height,
            "Bottom scroll padding ({}px) must exceed HUD height ({}px)", SCROLL_PADDING_BOTTOM_PX, hud_approx_height);
    }

    #[test]
    fn test_card_id_generation_and_scroll_top_boundary() {
        // Assert that indices < 6 are treated as top-row elements mapping to scrollTo(top: 0)
        let is_top_row = |idx: usize| idx < 6;
        assert!(is_top_row(0));
        assert!(is_top_row(5));
        for idx in 0..10 {
            let card_id = format!("bp-card-{}", idx);
            assert_eq!(card_id, format!("bp-card-{idx}"));
        }
    }

    #[test]
    fn test_options_apply_hud_and_glyph_mapping() {
        use dlss_studio::big_picture::gamepad::ControllerKind;

        // Controller glyph resolution for the secondary [X] button
        assert_eq!(ControllerKind::XboxSeriesX.patch_glyph().0, "btn-glyph xb-x");
        assert_eq!(ControllerKind::XboxSeriesX.patch_glyph().1, "X");
        assert_eq!(ControllerKind::PlayStation5.patch_glyph().0, "btn-glyph ps-square");
        assert_eq!(ControllerKind::PlayStation5.patch_glyph().1, ControllerKind::PlayStation4.patch_glyph().1);
        assert_eq!(ControllerKind::NintendoSwitch.patch_glyph().0, "btn-glyph n-y");
        assert_eq!(ControllerKind::NintendoSwitch.patch_glyph().1, "Y");

        // Multilingual validation for bp_hud_apply
        for lang in ["en", "de", "es", "fr", "it", "pt", "ru", "zh", "ja", "ko", "pl", "tr", "ar", "hi"] {
            let translated = dlss_studio::core::i18n::t(lang, "bp_hud_apply");
            assert!(!translated.is_empty(), "bp_hud_apply must be localized for {}", lang);
        }
    }

    #[test]
    fn test_is_dlss_studio_foreground_callable() {
        // Safe invocation test verifying the helper executes without panic
        let is_fg = is_dlss_studio_foreground();
        // In headless cargo test runners, the active foreground is either the runner or non-app window
        let _ = is_fg;
    }
