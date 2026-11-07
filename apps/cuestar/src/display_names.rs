//! Nom lisible des écrans (« DELL U2720Q ») pour le choix de l'écran de sortie, au lieu du nom
//! système (`\\.\DISPLAY2` sous Windows, `Monitor #41007` sous macOS, `HDMI-1` sous Linux).
//! Windows : QueryDisplayConfig ; macOS : `system_profiler` ; Linux X11 : EDID lu par `xrandr`.
//! Les noms sont lus une fois, en arrière-plan : en attendant, le nom système reste affiché.

use std::collections::HashMap;
use std::sync::OnceLock;

static NAMES: OnceLock<HashMap<String, String>> = OnceLock::new();
static STARTED: std::sync::Once = std::sync::Once::new();

/// Nom lisible de l'écran, avec son repère système : « DELL U2720Q (écran 2) ».
pub fn label(system_name: &str) -> String {
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            let _ = NAMES.set(read_names());
        });
    });
    let short = short_name(system_name);
    match NAMES.get().and_then(|m| m.get(system_name)).filter(|n| !n.is_empty()) {
        Some(n) if short.is_empty() || *n == short => n.clone(),
        Some(n) => format!("{n} ({short})"),
        None if short.is_empty() => system_name.to_string(),
        None => short,
    }
}

/// Repère court tiré du nom système : « écran 2 » pour `\\.\DISPLAY2`, le connecteur sous Linux,
/// rien sous macOS (`Monitor #…` n'apprend rien).
fn short_name(system_name: &str) -> String {
    if let Some(n) = system_name.strip_prefix(r"\\.\DISPLAY") {
        return format!("écran {n}");
    }
    if system_name.starts_with("Monitor #") {
        return String::new();
    }
    system_name.to_string()
}

#[cfg(windows)]
fn read_names() -> HashMap<String, String> {
    use windows::Win32::Devices::Display::*;
    use windows::Win32::Foundation::ERROR_SUCCESS;

    fn wide(s: &[u16]) -> String {
        let end = s.iter().position(|c| *c == 0).unwrap_or(s.len());
        String::from_utf16_lossy(&s[..end]).trim().to_string()
    }

    let mut out = HashMap::new();
    unsafe {
        let (mut np, mut nm) = (0u32, 0u32);
        if GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut np, &mut nm) != ERROR_SUCCESS {
            return out;
        }
        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); np as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); nm as usize];
        if QueryDisplayConfig(QDC_ONLY_ACTIVE_PATHS, &mut np, paths.as_mut_ptr(), &mut nm, modes.as_mut_ptr(), None) != ERROR_SUCCESS {
            return out;
        }
        paths.truncate(np as usize);
        for p in &paths {
            let mut src = DISPLAYCONFIG_SOURCE_DEVICE_NAME::default();
            src.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME;
            src.header.size = std::mem::size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32;
            src.header.adapterId = p.sourceInfo.adapterId;
            src.header.id = p.sourceInfo.id;
            if DisplayConfigGetDeviceInfo(&mut src.header) != 0 {
                continue;
            }
            let mut tgt = DISPLAYCONFIG_TARGET_DEVICE_NAME::default();
            tgt.header.r#type = DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME;
            tgt.header.size = std::mem::size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32;
            tgt.header.adapterId = p.targetInfo.adapterId;
            tgt.header.id = p.targetInfo.id;
            if DisplayConfigGetDeviceInfo(&mut tgt.header) != 0 {
                continue;
            }
            let mut name = wide(&tgt.monitorFriendlyDeviceName);
            // Dalle d'un portable : souvent sans nom.
            if name.is_empty() && tgt.outputTechnology == DISPLAYCONFIG_OUTPUT_TECHNOLOGY_INTERNAL {
                name = "Écran intégré".into();
            }
            if !name.is_empty() {
                out.entry(wide(&src.viewGdiDeviceName)).or_insert(name);
            }
        }
    }
    out
}

#[cfg(target_os = "macos")]
fn read_names() -> HashMap<String, String> {
    std::process::Command::new("system_profiler")
        .args(["SPDisplaysDataType", "-json"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| parse_system_profiler(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

#[cfg(all(unix, not(target_os = "macos")))]
fn read_names() -> HashMap<String, String> {
    std::process::Command::new("xrandr")
        .arg("--verbose")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| parse_xrandr(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

#[cfg(not(any(windows, unix)))]
fn read_names() -> HashMap<String, String> {
    HashMap::new()
}

/// macOS : winit nomme l'écran `Monitor #<numéro de modèle>` ; `system_profiler` donne le nom
/// de chaque écran avec ce numéro de modèle en hexadécimal.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_system_profiler(json: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return out };
    let gpus = v.get("SPDisplaysDataType").and_then(|g| g.as_array()).cloned().unwrap_or_default();
    for gpu in gpus {
        for d in gpu.get("spdisplays_ndrvs").and_then(|d| d.as_array()).cloned().unwrap_or_default() {
            let Some(name) = d.get("_name").and_then(|n| n.as_str()) else { continue };
            let product = ["_spdisplays_display-product-id", "spdisplays_display-product-id"]
                .iter()
                .find_map(|k| d.get(*k).and_then(|p| p.as_str()))
                .and_then(|p| u32::from_str_radix(p.trim().trim_start_matches("0x"), 16).ok());
            if let Some(p) = product {
                out.entry(format!("Monitor #{p}")).or_insert_with(|| name.to_string());
            }
        }
    }
    out
}

/// Linux X11 : nom du moniteur dans l'EDID de chaque sortie de `xrandr --verbose`.
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
fn parse_xrandr(text: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut output: Option<String> = None;
    let mut edid: Option<String> = None;
    let mut flush = |output: &Option<String>, edid: &mut Option<String>| {
        if let (Some(o), Some(hex)) = (output, edid.take()) {
            if let Some(n) = edid_name(&hex) {
                out.insert(o.clone(), n);
            }
        }
    };
    for line in text.lines() {
        if !line.starts_with(char::is_whitespace) {
            flush(&output, &mut edid);
            output = line.split_whitespace().next().filter(|_| line.contains(" connected")).map(str::to_string);
            continue;
        }
        let t = line.trim();
        if t == "EDID:" {
            edid = Some(String::new());
        } else if let Some(e) = edid.as_mut() {
            if !t.is_empty() && t.chars().all(|c| c.is_ascii_hexdigit()) {
                e.push_str(t);
            } else {
                flush(&output, &mut edid);
            }
        }
    }
    flush(&output, &mut edid);
    out
}

/// Nom du moniteur (descripteur 0xFC) dans un EDID en hexadécimal.
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
fn edid_name(hex: &str) -> Option<String> {
    let bytes: Vec<u8> = (0..hex.len() / 2).filter_map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).ok()).collect();
    for off in [54, 72, 90, 108] {
        let d = bytes.get(off..off + 18)?;
        if d[0..3] == [0, 0, 0] && d[3] == 0xFC {
            let text: String = d[5..].iter().take_while(|b| **b != 0x0A).map(|b| *b as char).collect();
            let text = text.trim().to_string();
            return (!text.is_empty()).then_some(text);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_courts() {
        assert_eq!(short_name(r"\\.\DISPLAY2"), "écran 2");
        assert_eq!(short_name("Monitor #41007"), "");
        assert_eq!(short_name("HDMI-1"), "HDMI-1");
    }

    #[test]
    fn edid() {
        // EDID réel (en-tête, puis descripteur 0xFC « DELL U2720Q » au 4e emplacement).
        let mut b = vec![0u8; 128];
        b[0..8].copy_from_slice(&[0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0]);
        b[108..113].copy_from_slice(&[0, 0, 0, 0xFC, 0]);
        let name = b"DELL U2720Q\n ";
        b[113..113 + name.len()].copy_from_slice(name);
        let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
        assert_eq!(edid_name(&hex).as_deref(), Some("DELL U2720Q"));

        let mut text = String::from("Screen 0: minimum 8 x 8\nHDMI-1 connected primary 1920x1080+0+0 (0x46) normal\n\tIdentifier: 0x42\n\tEDID: \n");
        for chunk in hex.as_bytes().chunks(32) {
            text += &format!("\t\t{}\n", std::str::from_utf8(chunk).unwrap());
        }
        text += "\tBorderDimensions: 4\nDP-1 disconnected (normal)\n";
        let m = parse_xrandr(&text);
        assert_eq!(m.get("HDMI-1").map(String::as_str), Some("DELL U2720Q"));
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn system_profiler() {
        let json = r#"{"SPDisplaysDataType":[{"_name":"Apple M1","spdisplays_ndrvs":[
            {"_name":"Color LCD","_spdisplays_display-product-id":"a050"},
            {"_name":"LG HDR 4K","_spdisplays_display-product-id":"5b6f"}]}]}"#;
        let m = parse_system_profiler(json);
        assert_eq!(m.get("Monitor #41040").map(String::as_str), Some("Color LCD"));
        assert_eq!(m.get("Monitor #23407").map(String::as_str), Some("LG HDR 4K"));
    }
}
