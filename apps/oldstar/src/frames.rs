//! Images JPEG des fenêtres capturées, et touchers de l'iPad ramenés aux pixels de la fenêtre.
//!
//! L'iPad demande l'image suivante quand il a fini d'afficher la précédente : le débit s'adapte
//! tout seul au Wi-Fi et à la lenteur de l'iPad 2. Chaque fenêtre est capturée au plus `fps`
//! fois par seconde, quel que soit le nombre d'iPad qui la regardent.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use image::codecs::jpeg::JpegEncoder;
use image::{ImageBuffer, Rgb};

use crate::capture;
use crate::config::{WindowConfig, Zone};
use crate::hub::Hub;

/// Fenêtre à afficher : une de la config (`Config::windows`), ou une fenêtre ouverte choisie
/// librement dans la page (identifiant donné par `capture::list`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Target {
    Config(usize),
    Free(isize),
}

#[derive(Default)]
pub struct Frames {
    /// Fenêtre retrouvée pour chaque entrée de la config : (critères cherchés, poignée, quand).
    handles: Mutex<HashMap<usize, (String, isize, Instant)>>,
    /// Dernière image encodée par (fenêtre, zone).
    jpegs: Mutex<HashMap<(Target, Option<usize>), (Instant, Arc<Vec<u8>>)>>,
}

fn criteria(w: &WindowConfig) -> String {
    format!("{}\u{1}{}\u{1}{}", w.title, w.exe.join("|"), w.path)
}

impl Frames {
    fn handle(&self, idx: usize, w: &WindowConfig) -> Result<isize, String> {
        let mut map = self.handles.lock().map_err(|_| "verrou")?;
        let key = criteria(w);
        if let Some((k, h, at)) = map.get(&idx) {
            if *k == key && capture::is_window(*h) && at.elapsed() < Duration::from_secs(5) {
                return Ok(*h);
            }
        }
        if !capture::AVAILABLE {
            return Err("capture des fenêtres non disponible sur ce système".into());
        }
        let m = capture::Match { title: &w.title, exe: &w.exe, path: &w.path };
        let h = capture::find(&m).ok_or_else(|| format!("fenêtre « {} » introuvable", w.name))?;
        map.insert(idx, (key, h, Instant::now()));
        Ok(h)
    }

    /// Réglages et poignée de la fenêtre visée.
    fn resolve(&self, hub: &Hub, target: Target) -> Result<(WindowConfig, isize), String> {
        let cfg = hub.cfg();
        match target {
            Target::Config(idx) => {
                let w = cfg.windows.get(idx).ok_or("fenêtre inconnue")?.clone();
                let h = self.handle(idx, &w)?;
                Ok((w, h))
            }
            Target::Free(h) => {
                if !cfg.free_windows {
                    return Err("choix libre des fenêtres désactivé dans les réglages".into());
                }
                if !capture::AVAILABLE {
                    return Err("capture des fenêtres non disponible sur ce système".into());
                }
                // Seulement une fenêtre que la liste montre (visible, titrée, pas OldStar).
                let info = capture::list().into_iter().find(|w| w.id == h).ok_or("fenêtre fermée")?;
                Ok((WindowConfig { name: info.title, ..Default::default() }, h))
            }
        }
    }

    pub fn jpeg(&self, hub: &Hub, target: Target, zone: Option<usize>) -> Result<Arc<Vec<u8>>, String> {
        let (w, h) = self.resolve(hub, target)?;
        let zone_cfg = match zone {
            Some(z) => Some(w.zones.get(z).ok_or("zone inconnue")?.clone()),
            None => None,
        };
        let period = Duration::from_millis(1000 / w.fps.clamp(1, 30) as u64);
        if let Some((at, img)) = self.jpegs.lock().map_err(|_| "verrou")?.get(&(target, zone)) {
            if at.elapsed() < period {
                return Ok(img.clone());
            }
        }
        let frame = capture::capture(h)?;
        let img = Arc::new(encode(&frame, zone_cfg.as_ref(), w.max_width, w.jpeg_quality)?);
        self.jpegs.lock().map_err(|_| "verrou")?.insert((target, zone), (Instant::now(), img.clone()));
        Ok(img)
    }

    /// Toucher en coordonnées relatives (0 à 1) de l'image affichée → clic dans la fenêtre.
    /// `mode` : « postmessage » (arrière-plan, la fenêtre ne passe pas devant) ou
    /// « sendinput » (premier plan, vrais clics) ; sinon celui des réglages de la fenêtre.
    #[allow(clippy::too_many_arguments)]
    pub fn input(&self, hub: &Hub, target: Target, zone: Option<usize>, mode: Option<&str>, kind: &str, nx: f64, ny: f64, wheel: i32) -> Result<(), String> {
        let (w, h) = self.resolve(hub, target)?;
        let (cw, ch) = capture::client_size(h).ok_or("taille de la fenêtre introuvable")?;
        let rect = zone.and_then(|z| w.zones.get(z)).map(|z| clamp_zone(z, cw as u32, ch as u32)).unwrap_or((0, 0, cw as u32, ch as u32));
        let (x, y) = map_point(rect, nx, ny);
        let cfg = hub.cfg();
        let mode = match mode {
            Some(m @ ("postmessage" | "sendinput")) => m,
            _ => w.input.as_str(),
        };
        // Un glisser ne marche qu'avec de vrais mouvements de souris.
        let mode = if cfg.drag.always_real && matches!(kind, "down" | "move" | "up") { "sendinput" } else { mode };
        capture::set_drag_timing(cfg.drag.timing());
        capture::input(h, mode, kind, x, y, wheel)
    }
}

/// Zone ramenée dans la fenêtre : (x, y, largeur, hauteur).
pub fn clamp_zone(z: &Zone, w: u32, h: u32) -> (u32, u32, u32, u32) {
    let x = z.x.min(w.saturating_sub(1));
    let y = z.y.min(h.saturating_sub(1));
    (x, y, z.w.max(1).min(w - x), z.h.max(1).min(h - y))
}

/// Point relatif (0 à 1) d'un rectangle → pixel.
pub fn map_point(rect: (u32, u32, u32, u32), nx: f64, ny: f64) -> (i32, i32) {
    let (x, y, w, h) = rect;
    let px = x as f64 + nx.clamp(0.0, 1.0) * (w.saturating_sub(1)) as f64;
    let py = y as f64 + ny.clamp(0.0, 1.0) * (h.saturating_sub(1)) as f64;
    (px.round() as i32, py.round() as i32)
}

fn encode(f: &capture::Frame, zone: Option<&Zone>, max_width: u32, quality: u8) -> Result<Vec<u8>, String> {
    let (x0, y0, w, h) = zone.map(|z| clamp_zone(z, f.w, f.h)).unwrap_or((0, 0, f.w, f.h));
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for y in y0..y0 + h {
        let row = &f.bgra[((y * f.w + x0) * 4) as usize..((y * f.w + x0 + w) * 4) as usize];
        for px in row.chunks_exact(4) {
            rgb.extend_from_slice(&[px[2], px[1], px[0]]);
        }
    }
    let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_raw(w, h, rgb).ok_or("image invalide")?;
    let max_width = max_width.max(64);
    if w > max_width {
        let nh = ((h as u64 * max_width as u64) / w as u64).max(1) as u32;
        img = image::imageops::thumbnail(&img, max_width, nh);
    }
    let mut out = Vec::new();
    JpegEncoder::new_with_quality(&mut out, quality.clamp(20, 95)).encode_image(&img).map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapping() {
        assert_eq!(map_point((0, 0, 101, 51), 0.5, 1.0), (50, 50));
        let z = Zone { name: "z".into(), x: 100, y: 50, w: 200, h: 100 };
        let r = clamp_zone(&z, 1000, 800);
        assert_eq!(r, (100, 50, 200, 100));
        assert_eq!(map_point(r, 0.0, 0.0), (100, 50));
        // Zone qui déborde : coupée au bord de la fenêtre.
        assert_eq!(clamp_zone(&z, 250, 120), (100, 50, 150, 70));
    }

    #[test]
    fn encodes_zone() {
        let f = capture::Frame { w: 64, h: 32, bgra: vec![128; 64 * 32 * 4] };
        let z = Zone { name: "z".into(), x: 10, y: 10, w: 20, h: 10 };
        let jpg = encode(&f, Some(&z), 1024, 60).unwrap();
        assert_eq!(&jpg[..2], &[0xff, 0xd8]);
    }
}
