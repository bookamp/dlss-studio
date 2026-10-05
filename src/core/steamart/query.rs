//! Game title query normalization, sanitization, and Steam store candidate scoring.

use regex::Regex;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct StoreSearchResult {
    pub items: Option<Vec<StoreItem>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StoreItem {
    pub id: u64,
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub item_type: Option<String>,
}

/// Normalizes and cleans game folder names by removing scene tags, repacks, and edition markers.
pub fn clean_name(name: &str) -> String {
    let re_brackets = Regex::new(r"\[[^\]]*\]|\([^)]*\)").unwrap();
    let re_tags = Regex::new(r"(?i)\b(repack|fitgirl|dodi|elamigos|codex|rune|empress|plaza|skidrow|multi\d*)\b").unwrap();
    let re_punct = Regex::new(r"[_\-—–:.]+").unwrap();
    let re_spaces = Regex::new(r"\s+").unwrap();

    let s = re_brackets.replace_all(name, " ");
    let s = re_tags.replace_all(&s, " ");
    let s = re_punct.replace_all(&s, " ");
    let s = re_spaces.replace_all(&s, " ");
    s.trim().to_string()
}

#[doc(hidden)]
pub fn norm_title(s: &str) -> String {
    s.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Scores candidate Steam search items against query string.
pub fn score_item(item: &StoreItem, query: &str) -> i32 {
    if let Some(ref t) = item.item_type {
        if t != "app" {
            return -100;
        }
    }
    let q = norm_title(query);
    let q_words: std::collections::HashSet<&str> = q.split_whitespace().collect();
    let n = norm_title(item.name.as_deref().unwrap_or_default());
    let words: Vec<&str> = n.split_whitespace().collect();
    let shared = words.iter().filter(|w| q_words.contains(*w)).count() as i32;

    let mut score: i32 = 0;
    if n == q {
        score += 120;
    } else if n.starts_with(&q) || q.starts_with(&n) {
        score += 70;
    }
    if !q_words.is_empty() {
        score += (shared * 40) / (q_words.len() as i32);
    }

    let edition_words: std::collections::HashSet<&str> = [
        "edition", "ultimate", "deluxe", "definitive", "enhanced", "complete",
        "remastered", "goty", "year", "standard", "director", "directors", "cut"
    ].iter().cloned().collect();

    for w in &words {
        if !q_words.contains(w) {
            if edition_words.contains(w) {
                score += 5;
            } else {
                score -= 8;
            }
        }
    }

    score
}

/// Evaluates candidate Steam search rows and picks the highest scoring match.
#[doc(hidden)]
pub fn pick_best(items: &[StoreItem], query: &str) -> Option<StoreItem> {
    let mut best: Option<StoreItem> = None;
    let mut best_score: i32 = -1;

    for item in items {
        let score = score_item(item, query);
        if score > best_score {
            best_score = score;
            best = Some(item.clone());
        }
    }

    if best_score > 10 {
        best
    } else {
        None
    }
}

pub fn url_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.' || b == b'~' {
            out.push(b as char);
        } else if b == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{:02X}", b));
        }
    }
    out
}

pub fn url_decode(s: &str) -> String {
    let mut out = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(val) = u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16) {
                out.push(val);
                i += 3;
                continue;
            }
        } else if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

pub fn split_camel_or_numbers(s: &str) -> String {
    let mut out = String::new();
    let mut prev_char: Option<char> = None;
    for c in s.chars() {
        if let Some(p) = prev_char {
            let is_lower_to_upper = p.is_lowercase() && c.is_uppercase();
            let is_letter_to_digit = p.is_alphabetic() && c.is_numeric();
            let is_digit_to_letter = p.is_numeric() && c.is_alphabetic();
            if is_lower_to_upper || is_letter_to_digit || is_digit_to_letter {
                out.push(' ');
            }
        }
        out.push(c);
        prev_char = Some(c);
    }
    out
}
