use std::fs;
use std::io::{Read, Seek};
use std::path::Path;

pub fn extract_zip<R: Read + Seek>(reader: R, out_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|e| format!("Failed to create {}: {}", out_dir.display(), e))?;
    let mut archive = zip::ZipArchive::new(reader).map_err(|e| format!("Invalid zip archive: {}", e))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| format!("Corrupt zip entry: {}", e))?;
        let outpath = match file.enclosed_name() {
            Some(path) => out_dir.join(path),
            None => continue,
        };

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath).map_err(|e| format!("Failed to create {}: {}", outpath.display(), e))?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p).map_err(|e| format!("Failed to create {}: {}", p.display(), e))?;
                }
            }
            let mut outfile = fs::File::create(&outpath).map_err(|e| format!("Failed to create {}: {}", outpath.display(), e))?;
            std::io::copy(&mut file, &mut outfile).map_err(|e| format!("Failed to extract to {}: {}", outpath.display(), e))?;
        }
    }
    Ok(())
}

/// Helper to construct a tar.exe command configured with CREATE_NO_WINDOW on Windows
/// to prevent console window flashing during background component extractions.
pub fn silent_tar_command() -> std::process::Command {
    let mut cmd = std::process::Command::new("tar.exe");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}
