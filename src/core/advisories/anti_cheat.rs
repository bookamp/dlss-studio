use crate::core::scan::GameEntry;
use super::{AdvisorySeverity, RouteAdvisory};

/// Evaluates whether the game uses anti-cheat mechanisms that may flag DLL injection or ReShade hooks.
pub fn get_anti_cheat_advisory(game: &GameEntry) -> Option<RouteAdvisory> {
    if !game.has_anti_cheat {
        return None;
    }

    Some(RouteAdvisory {
        title: format!("Anti-Cheat Detected on {}", game.name),
        reasons: vec![
            format!(
                "Anti-Cheat Protection: {} is protected by anti-cheat middleware. Modifying game binaries or injecting proxy DLLs may result in startup crashes, integrity errors, or account suspension.",
                game.name
            ),
        ],
        recommendation: "Proceed with caution. Disable multiplayer components or run in offline/singleplayer mode when deploying mods.".to_string(),
        severity: AdvisorySeverity::Warning,
    })
}
