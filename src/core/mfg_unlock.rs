#![allow(dead_code)]

pub const RELEASE_VERSION: &str = "0.9";
pub const RELEASE_URL: &str = "https://github.com/mavismmg/MFGAdaUnlock-RenoDx/releases/download/0.9/renodx-mfgunlock.addon64";
pub const RELEASE_SHA256: &str = "64184bb370f223c3cabb359010a9a64e114cdae6b62d8b014a731a602af0a0da";

pub fn configure_mfg_unlock_ini(existing: &str, multiplier: Option<u32>) -> String {
    let mult = multiplier.unwrap_or(4);
    let force_multiplier = match mult {
        2 => "2",
        3 => "3",
        4 => "4",
        _ => "0", // 1x or 0 = Auto / game menu controlled
    };

    let mut lines: Vec<String> = existing.lines().map(|s| s.to_string()).collect();
    let mut section_start = None;
    let mut section_end = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.eq_ignore_ascii_case("[RenoDX.MFGUnlock]") {
            section_start = Some(i);
        } else if section_start.is_some() && trimmed.starts_with('[') && trimmed.ends_with(']') {
            section_end = Some(i);
            break;
        }
    }

    let entries = vec![
        ("Enabled", "1".to_string()),
        ("LogLevel", "0".to_string()),
        ("Method", "0".to_string()),
        ("ForceMultiplier", force_multiplier.to_string()),
        ("MaxCount", "4".to_string()),
        ("TemporalFix", "1".to_string()),
    ];

    if let Some(start) = section_start {
        let end = section_end.unwrap_or(lines.len());
        lines.drain(start..end);
    }

    if !lines.is_empty() && !lines.last().unwrap().is_empty() {
        lines.push("".to_string());
    }
    lines.push("[RenoDX.MFGUnlock]".to_string());
    for (k, v) in entries {
        lines.push(format!("{}={}", k, v));
    }
    lines.push("".to_string());

    lines.join("\r\n")
}

