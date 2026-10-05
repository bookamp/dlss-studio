//! OptiScaler candidate scoring heuristics.

/// Computes a priority score for OptiScaler directory candidates.
/// Higher version numbers and RTX40 MFG variants receive higher scores.
pub fn score_optiscaler_dir(name: &str) -> u64 {
    let mut score: u64 = 0;
    let lower = name.to_lowercase();

    if lower.contains("mfg") || lower.contains("rtx40") {
        score += 100_000_000;
    }
    if lower.contains("dlssnr") {
        score += 50_000_000;
    }

    // Extract semver components if present (e.g. 0.8.5)
    let parts: Vec<&str> = lower.split(|c: char| !c.is_ascii_digit() && c != '.').collect();
    for p in parts {
        let sub: Vec<&str> = p.split('.').collect();
        if sub.len() >= 2 {
            let major = sub[0].parse::<u64>().unwrap_or(0);
            let minor = sub[1].parse::<u64>().unwrap_or(0);
            let patch = sub.get(2).and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
            score += major * 1_000_000 + minor * 10_000 + patch * 100;
            break;
        }
    }

    score
}
