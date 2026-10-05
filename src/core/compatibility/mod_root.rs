use std::path::{Path, PathBuf};

pub fn managed_mod_root(game_dir: &Path, exe_path: Option<&Path>) -> Option<PathBuf> {
    let mut current = exe_path
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| game_dir.to_path_buf());

    for _ in 0..5 {
        let has_mo2 = current.join("ModOrganizer.exe").exists();
        let has_stock = current.join("Stock Game").exists();
        if has_mo2 && has_stock {
            return Some(current);
        }
        if let Some(parent) = current.parent() {
            if parent == current {
                break;
            }
            current = parent.to_path_buf();
        } else {
            break;
        }
    }
    None
}
