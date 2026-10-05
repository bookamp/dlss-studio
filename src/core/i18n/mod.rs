pub mod langs;

#[allow(dead_code)]
pub struct LangInfo {
    pub code: &'static str,
    pub label: &'static str,
    pub native: &'static str,
    pub dir: &'static str,
}

pub const SUPPORTED_LANGS: &[LangInfo] = &[
    LangInfo { code: "en", label: "English", native: "English", dir: "ltr" },
    LangInfo { code: "de", label: "German", native: "Deutsch", dir: "ltr" },
    LangInfo { code: "es", label: "Spanish", native: "Español", dir: "ltr" },
    LangInfo { code: "fr", label: "French", native: "Français", dir: "ltr" },
    LangInfo { code: "it", label: "Italian", native: "Italiano", dir: "ltr" },
    LangInfo { code: "pt", label: "Portuguese", native: "Português", dir: "ltr" },
    LangInfo { code: "ru", label: "Russian", native: "Русский", dir: "ltr" },
    LangInfo { code: "zh", label: "Chinese", native: "简体中文", dir: "ltr" },
    LangInfo { code: "ja", label: "Japanese", native: "日本語", dir: "ltr" },
    LangInfo { code: "ko", label: "Korean", native: "한국어", dir: "ltr" },
    LangInfo { code: "pl", label: "Polish", native: "Polski", dir: "ltr" },
    LangInfo { code: "tr", label: "Turkish", native: "Türkçe", dir: "ltr" },
    LangInfo { code: "ar", label: "Arabic", native: "العربية", dir: "rtl" },
    LangInfo { code: "hi", label: "Hindi", native: "हिन्दी", dir: "ltr" },
];

#[allow(dead_code)]
extern "system" {
    fn GetUserDefaultUILanguage() -> u16;
    fn GetUserDefaultLocaleName(lpLocaleName: *mut u16, cchLocaleName: i32) -> i32;
    fn GetUserPreferredUILanguages(
        dwFlags: u32,
        pulNumLanguages: *mut u32,
        pwszLanguagesBuffer: *mut u16,
        pcchLanguagesBuffer: *mut u32,
    ) -> i32;
}

#[allow(dead_code)]
pub fn detect_system_language() -> String {
    const MUI_LANGUAGE_NAME: u32 = 0x8;
    let mut num_langs: u32 = 0;
    let mut buf_len: u32 = 0;
    unsafe {
        // 1. Try GetUserPreferredUILanguages (priority list of user's display languages)
        if GetUserPreferredUILanguages(
            MUI_LANGUAGE_NAME,
            &mut num_langs,
            std::ptr::null_mut(),
            &mut buf_len,
        ) != 0 && buf_len > 1 {
            let mut buf = vec![0u16; buf_len as usize];
            if GetUserPreferredUILanguages(
                MUI_LANGUAGE_NAME,
                &mut num_langs,
                buf.as_mut_ptr(),
                &mut buf_len,
            ) != 0 {
                let s = String::from_utf16_lossy(&buf);
                for tag in s.split('\0') {
                    if tag.is_empty() {
                        continue;
                    }
                    let prefix = tag.split('-').next().unwrap_or("").to_lowercase();
                    if SUPPORTED_LANGS.iter().any(|l| l.code == prefix) {
                        return prefix;
                    }
                }
            }
        }

        // 2. Try GetUserDefaultUILanguage() primary language ID
        let uilang = GetUserDefaultUILanguage();
        let primary = uilang & 0x3FF;
        let code = match primary {
            0x09 => "en",
            0x07 => "de",
            0x0a => "es",
            0x0c => "fr",
            0x10 => "it",
            0x16 => "pt",
            0x19 => "ru",
            0x04 => "zh",
            0x11 => "ja",
            0x12 => "ko",
            0x15 => "pl",
            0x1f => "tr",
            0x01 => "ar",
            0x39 => "hi",
            _ => "",
        };
        if !code.is_empty() && SUPPORTED_LANGS.iter().any(|l| l.code == code) {
            return code.to_string();
        }

        // 3. Try GetUserDefaultLocaleName
        let mut locale_buf = [0u16; 85];
        let len = GetUserDefaultLocaleName(locale_buf.as_mut_ptr(), locale_buf.len() as i32);
        if len > 1 {
            let locale = String::from_utf16_lossy(&locale_buf[..(len as usize - 1)]).to_lowercase();
            let prefix = locale.split('-').next().unwrap_or("en");
            if SUPPORTED_LANGS.iter().any(|l| l.code == prefix) {
                return prefix.to_string();
            }
        }
    }

    "en".to_string()
}

#[allow(dead_code)]
pub fn format_log_entry(lang: &str, msg: &str) -> String {
    if msg.eq_ignore_ascii_case("ready") {
        return t(lang, "status_ready").to_string();
    }
    if let Some(start) = msg.find("@{") {
        if let Some(end) = msg[start..].find('}') {
            let token = &msg[start + 2..start + end];
            let parts: Vec<&str> = token.split('|').collect();
            if let Some(key) = parts.first() {
                let template = t(lang, key);
                let mut rendered = template.to_string();
                for (i, arg) in parts.iter().skip(1).enumerate() {
                    let resolved_arg = if arg.starts_with('@') {
                        t(lang, &arg[1..])
                    } else {
                        *arg
                    };
                    rendered = rendered.replace(&format!("{{{}}}", i), resolved_arg);
                }
                let prefix = &msg[..start];
                let suffix = &msg[start + end + 1..];
                return format!("{}{}{}", prefix, rendered, format_log_entry(lang, suffix));
            }
        }
    }
    msg.to_string()
}

#[allow(dead_code)]
pub fn t_param(lang: &str, key: &str, param: &str) -> String {
    t(lang, key).replace("{0}", param)
}

#[allow(dead_code)]
pub fn t_params(lang: &str, key: &str, params: &[&str]) -> String {
    let mut s = t(lang, key).to_string();
    for (i, p) in params.iter().enumerate() {
        s = s.replace(&format!("{{{}}}", i), p);
    }
    s
}

pub fn t<'a>(lang: &str, key: &'a str) -> &'a str {
    let resolved_key = match key {
        "setup_reinstall_detected" => "setup_existing_detected",
        "setup_reinstall_desc" => "setup_existing_help",
        "setup_btn_browse" => "setup_browse",
        "setup_pref_shortcut" => "setup_pref_desktop",
        "setup_storage_desc" => "setup_storage_help",
        "setup_btn_install" => "setup_btn_install_now",
        k => k,
    };
    let l = lang.to_lowercase();
    let l_prefix = l.split('-').next().unwrap_or("en");

    let val = match l_prefix {
        "de" => langs::de::lookup(resolved_key),
        "es" => langs::es::lookup(resolved_key),
        "fr" => langs::fr::lookup(resolved_key),
        "it" => langs::it::lookup(resolved_key),
        "pt" => langs::pt::lookup(resolved_key),
        "ru" => langs::ru::lookup(resolved_key),
        "zh" => langs::zh::lookup(resolved_key),
        "ja" => langs::ja::lookup(resolved_key),
        "ko" => langs::ko::lookup(resolved_key),
        "pl" => langs::pl::lookup(resolved_key),
        "tr" => langs::tr::lookup(resolved_key),
        "ar" => langs::ar::lookup(resolved_key),
        "hi" => langs::hi::lookup(resolved_key),
        _ => None,
    };

    if let Some(trans) = val {
        return trans;
    }

    // Fall back to English
    if let Some(trans) = langs::en::lookup(resolved_key) {
        return trans;
    }

    // Fall back to key itself
    key
}

#[allow(dead_code)]
pub fn translate_advisory_reason(lang: &str, reason: &str) -> String {
    if reason.starts_with("32-bit Architecture") {
        t(lang, "advisory_reason_32bit").to_string()
    } else if reason.starts_with("DirectX 11 Interop Notice") {
        t(lang, "advisory_reason_dx11_interop").to_string()
    } else if reason.starts_with("Non-DirectX 12 / Missing Native Upscaler") {
        t(lang, "advisory_reason_no_native_upscaler").to_string()
    } else if reason.starts_with("Mod-Deployed DLSS Detected") {
        t(lang, "advisory_reason_mod_dlss").to_string()
    } else if reason.starts_with("Legacy / Non-DirectX API") {
        t(lang, "advisory_reason_legacy_api").to_string()
    } else if reason.starts_with("Missing Native DLSS:") {
        t(lang, "advisory_reason_missing_native_dlss").to_string()
    } else if reason.starts_with("Non-DirectX 12 API:") || reason.starts_with("DirectX 12 Required:") {
        let api = reason.rsplit("under ").next().map(|s| s.trim_end_matches('.')).unwrap_or("Non-DX12");
        t_param(lang, "advisory_reason_non_dx12_native", api)
    } else if reason.starts_with("DirectX 11 Limitation:") {
        t(lang, "advisory_reason_dx11_mfg").to_string()
    } else if reason.starts_with("Missing Native DLSS-G:") {
        t(lang, "advisory_reason_missing_dlssg").to_string()
    } else if reason.starts_with("Hardware Requirement:") {
        t(lang, "advisory_reason_hardware_rtx40").to_string()
    } else {
        reason.to_string()
    }
}

#[allow(dead_code)]
pub fn translate_advisory_recommendation(lang: &str, rec: &str) -> String {
    if rec.contains("DLSS 5 Feeder") && rec.contains("enable DLSS 5 Neural Rendering") {
        let api = rec.rsplit("on ").next().map(|s| s.trim_end_matches('.')).unwrap_or("");
        t_param(lang, "advisory_rec_feeder_api", api)
    } else if rec.contains("DLSS 5 Feeder route provides generic frame interception") {
        t(lang, "advisory_rec_feeder_generic").to_string()
    } else if rec.contains("For titles without native DLSS-G") {
        t(lang, "advisory_rec_feeder_sr_dlaa").to_string()
    } else if rec.contains("bridge DirectX 11 DLSS calls to D3D12") {
        t(lang, "advisory_rec_opti_dx11").to_string()
    } else if rec.contains("Switch executable to Vulkan") {
        t(lang, "advisory_rec_vulkan_fg_alt").to_string()
    } else {
        rec.to_string()
    }
}
