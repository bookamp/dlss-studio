#![allow(dead_code)]

/// Retrieves a value from an INI formatted string by section and key (case-insensitive).
pub fn get_ini(content: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    let target_section = section.trim().to_lowercase();
    let target_key = key.trim().to_lowercase();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(';') || trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let sec_name = &trimmed[1..trimmed.len() - 1].trim().to_lowercase();
            in_section = sec_name == &target_section;
            continue;
        }

        if in_section {
            if let Some((k, v)) = line.split_once('=') {
                if k.trim().to_lowercase() == target_key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

/// Sets or updates a key-value pair within a specified section of an INI string.
/// If the section or key does not exist, it is created. Preserves other sections and comments.
pub fn set_ini(content: &str, section: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let target_section = section.trim().to_lowercase();
    let target_key = key.trim().to_lowercase();

    let mut section_start: Option<usize> = None;
    let mut section_end: Option<usize> = None;
    let mut key_index: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let sec_name = &trimmed[1..trimmed.len() - 1].trim().to_lowercase();
            if sec_name == &target_section {
                section_start = Some(i);
            } else if section_start.is_some() && section_end.is_none() {
                section_end = Some(i);
            }
        } else if section_start.is_some() && section_end.is_none() {
            if let Some((k, _)) = line.split_once('=') {
                if k.trim().to_lowercase() == target_key {
                    key_index = Some(i);
                }
            }
        }
    }

    if let Some(k_idx) = key_index {
        lines[k_idx] = format!("{}={}", key.trim(), value.trim());
    } else if let Some(_s_idx) = section_start {
        let insert_idx = section_end.unwrap_or(lines.len());
        lines.insert(insert_idx, format!("{}={}", key.trim(), value.trim()));
    } else {
        if !lines.is_empty() && !lines.last().unwrap().is_empty() {
            lines.push(String::new());
        }
        lines.push(format!("[{}]", section.trim()));
        lines.push(format!("{}={}", key.trim(), value.trim()));
    }

    lines.join("\r\n")
}

/// Replaces or adds an entire section with the specified key-value entries.
pub fn configure_section(content: &str, section: &str, entries: &[(&str, &str)]) -> String {
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let target_section = section.trim().to_lowercase();

    let mut section_start: Option<usize> = None;
    let mut section_end: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let sec_name = &trimmed[1..trimmed.len() - 1].trim().to_lowercase();
            if sec_name == &target_section {
                section_start = Some(i);
            } else if section_start.is_some() && section_end.is_none() {
                section_end = Some(i);
            }
        }
    }

    if let Some(start) = section_start {
        let end = section_end.unwrap_or(lines.len());
        lines.drain(start..end);
    }

    if !lines.is_empty() && !lines.last().unwrap().is_empty() {
        lines.push(String::new());
    }
    lines.push(format!("[{}]", section.trim()));
    for (k, v) in entries {
        lines.push(format!("{}={}", k.trim(), v.trim()));
    }

    lines.join("\r\n")
}

/// Removes a section and all its keys from an INI string.
pub fn remove_section(content: &str, section: &str) -> String {
    let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
    let target_section = section.trim().to_lowercase();

    let mut section_start: Option<usize> = None;
    let mut section_end: Option<usize> = None;

    for (i, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let sec_name = &trimmed[1..trimmed.len() - 1].trim().to_lowercase();
            if sec_name == &target_section {
                section_start = Some(i);
            } else if section_start.is_some() && section_end.is_none() {
                section_end = Some(i);
            }
        }
    }

    if let Some(start) = section_start {
        let end = section_end.unwrap_or(lines.len());
        lines.drain(start..end);
    }

    // Clean up trailing double empty lines
    while lines.len() > 1 && lines.last().unwrap().is_empty() && lines[lines.len() - 2].is_empty() {
        lines.pop();
    }

    lines.join("\r\n")
}

/// Formats the [RenoDX.MFGUnlock] section within an INI configuration.
pub fn configure_mfg_unlock_ini(existing: &str, multiplier: Option<u32>) -> String {
    let mult = multiplier.unwrap_or(4);
    let force_multiplier = match mult {
        2 => "2",
        3 => "3",
        4 => "4",
        _ => "0", // 1x or 0 = Auto / game menu controlled
    };

    let entries = [
        ("Enabled", "1"),
        ("LogLevel", "0"),
        ("Method", "0"),
        ("ForceMultiplier", force_multiplier),
        ("MaxCount", "4"),
        ("TemporalFix", "1"),
    ];

    configure_section(existing, "RenoDX.MFGUnlock", &entries)
}
