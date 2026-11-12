//! État partagé entre la console, l'OSC, la capture et les clients web.
//!
//! Chaque changement devient un événement JSON numéroté. Les clients WebSocket et ceux en
//! long-polling attendent sur la même condition et reçoivent les événements qu'ils n'ont pas vus.
//! Les niveaux des vu-mètres ne passent pas par la file : seul le dernier relevé compte.

use serde_json::{json, Value};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Condvar, Mutex, RwLock};
use std::time::{Duration, Instant};

use crate::config::Config;

/// Événements gardés pour les clients en retard ; au-delà, ils rechargent tout l'état.
const KEEP: usize = 4000;

#[derive(Default)]
pub struct Meters {
    /// 0 à 100 par voie d'entrée et par mix.
    pub inch: Vec<u8>,
    pub mix: Vec<u8>,
    /// Augmente à chaque relevé.
    pub seq: u64,
}

#[derive(Default, Clone)]
pub struct YamahaStatus {
    pub connected: bool,
    pub model: String,
    pub error: String,
    pub last_error_line: String,
}

#[derive(Default)]
struct State {
    seq: u64,
    events: VecDeque<(u64, Value)>,
    /// Dernière valeur connue de chaque paramètre de la console : (adresse, x, y) → (valeur, texte).
    params: HashMap<(String, u32, u32), (Value, String)>,
    yamaha: YamahaStatus,
    /// Retour d'état OSC : (cible, adresse) → valeur.
    osc_values: HashMap<(usize, String), Value>,
    osc_alive: Vec<bool>,
    meters: Meters,
}

pub struct Hub {
    pub config: RwLock<Config>,
    pub config_path: PathBuf,
    state: Mutex<State>,
    changed: Condvar,
    /// Lignes à envoyer à la console (le client Yamaha les lit).
    pub yamaha_tx: Mutex<Option<Sender<crate::yamaha::Out>>>,
    /// Augmente à chaque rechargement de la config : la console se reconnecte.
    pub generation: AtomicU64,
    pub ws_clients: AtomicUsize,
    poll_clients: Mutex<HashMap<String, Instant>>,
    pub started: Instant,
}

impl Hub {
    pub fn new(config: Config, config_path: PathBuf) -> Self {
        Self {
            config: RwLock::new(config),
            config_path,
            state: Mutex::new(State::default()),
            changed: Condvar::new(),
            yamaha_tx: Mutex::new(None),
            generation: AtomicU64::new(0),
            ws_clients: AtomicUsize::new(0),
            poll_clients: Mutex::new(HashMap::new()),
            started: Instant::now(),
        }
    }

    pub fn cfg(&self) -> Config {
        self.config.read().map(|c| c.clone()).unwrap_or_default()
    }

    fn push_locked(&self, s: &mut State, ev: Value) {
        s.seq += 1;
        let seq = s.seq;
        s.events.push_back((seq, ev));
        while s.events.len() > KEEP {
            s.events.pop_front();
        }
    }

    pub fn push(&self, ev: Value) {
        if let Ok(mut s) = self.state.lock() {
            self.push_locked(&mut s, ev);
        }
        self.changed.notify_all();
    }

    /// Valeur d'un paramètre de la console ; n'émet un événement que si elle change.
    pub fn set_param(&self, addr: &str, x: u32, y: u32, v: Value, text: Option<String>) {
        if let Ok(mut s) = self.state.lock() {
            let key = (addr.to_string(), x, y);
            let old = s.params.get(&key).cloned();
            let text = text.or_else(|| old.as_ref().filter(|o| o.0 == v).map(|o| o.1.clone())).unwrap_or_default();
            if old.as_ref() == Some(&(v.clone(), text.clone())) {
                return;
            }
            s.params.insert(key, (v.clone(), text.clone()));
            self.push_locked(&mut s, json!({ "e": "p", "a": addr, "x": x, "y": y, "v": v, "s": text }));
        }
        self.changed.notify_all();
    }

    pub fn has_param(&self, addr: &str, x: u32, y: u32) -> bool {
        self.state.lock().map(|s| s.params.contains_key(&(addr.to_string(), x, y))).unwrap_or(false)
    }

    pub fn yamaha_status(&self) -> YamahaStatus {
        self.state.lock().map(|s| s.yamaha.clone()).unwrap_or_default()
    }

    pub fn set_yamaha_status(&self, f: impl FnOnce(&mut YamahaStatus)) {
        if let Ok(mut s) = self.state.lock() {
            let before = s.yamaha.clone();
            f(&mut s.yamaha);
            let y = s.yamaha.clone();
            if before.connected != y.connected || before.model != y.model || before.error != y.error {
                self.push_locked(&mut s, json!({ "e": "ys", "connected": y.connected, "model": y.model, "error": y.error }));
            } else {
                return;
            }
        }
        self.changed.notify_all();
    }

    pub fn set_osc_value(&self, target: usize, addr: &str, v: Value) {
        if let Ok(mut s) = self.state.lock() {
            let key = (target, addr.to_string());
            if s.osc_values.get(&key) == Some(&v) {
                return;
            }
            s.osc_values.insert(key, v.clone());
            self.push_locked(&mut s, json!({ "e": "of", "t": target, "a": addr, "v": v }));
        }
        self.changed.notify_all();
    }

    pub fn set_osc_alive(&self, alive: Vec<bool>) {
        if let Ok(mut s) = self.state.lock() {
            if s.osc_alive == alive {
                return;
            }
            s.osc_alive = alive.clone();
            self.push_locked(&mut s, json!({ "e": "os", "alive": alive }));
        }
        self.changed.notify_all();
    }

    pub fn osc_alive(&self) -> Vec<bool> {
        self.state.lock().map(|s| s.osc_alive.clone()).unwrap_or_default()
    }

    pub fn set_meters(&self, inch: Option<(usize, Vec<u8>)>, mix: Option<(usize, Vec<u8>)>) {
        if let Ok(mut s) = self.state.lock() {
            let m = &mut s.meters;
            for (dst, src) in [(&mut m.inch, inch), (&mut m.mix, mix)] {
                if let Some((start, vals)) = src {
                    if dst.len() < start + vals.len() {
                        dst.resize(start + vals.len(), 0);
                    }
                    dst[start..start + vals.len()].copy_from_slice(&vals);
                }
            }
            m.seq += 1;
        }
        self.changed.notify_all();
    }

    /// Relevé des vu-mètres s'il est plus récent que `seen`.
    pub fn meters_since(&self, seen: u64) -> Option<(u64, Value)> {
        let s = self.state.lock().ok()?;
        (s.meters.seq > seen).then(|| (s.meters.seq, json!({ "i": s.meters.inch, "m": s.meters.mix })))
    }

    pub fn seq(&self) -> u64 {
        self.state.lock().map(|s| s.seq).unwrap_or(0)
    }

    /// Événements après `since` ; `None` si une partie a été oubliée (le client doit tout recharger).
    pub fn events_since(&self, since: u64) -> Option<(u64, Vec<Value>)> {
        let s = self.state.lock().ok()?;
        if since > s.seq {
            return None;
        }
        if let Some((first, _)) = s.events.front() {
            if since + 1 < *first && since < s.seq {
                return None;
            }
        }
        let evs = s.events.iter().filter(|(n, _)| *n > since).map(|(_, e)| e.clone()).collect();
        Some((s.seq, evs))
    }

    /// Attend un nouvel événement (ou un relevé des vu-mètres si `meters` est donné).
    pub fn wait(&self, since: u64, meters: Option<u64>, timeout: Duration) {
        let Ok(guard) = self.state.lock() else { return };
        let _ = self.changed.wait_timeout_while(guard, timeout, |s| s.seq <= since && meters.is_none_or(|m| s.meters.seq <= m));
    }

    /// Tout l'état, pour un client qui arrive (ou qui a trop de retard).
    pub fn snapshot(&self) -> Value {
        let cfg = self.cfg();
        let s = match self.state.lock() {
            Ok(s) => s,
            Err(_) => return json!({}),
        };
        let params: Vec<Value> = s.params.iter().map(|((a, x, y), (v, t))| json!([a, x, y, v, t])).collect();
        let osc: Vec<Value> = s.osc_values.iter().map(|((t, a), v)| json!([t, a, v])).collect();
        let targets: Vec<Value> = cfg
            .osc
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "buttons": t.buttons.iter().map(|b| json!({ "label": default_label(&b.label), "style": b.style, "wide": b.wide })).collect::<Vec<_>>(),
                    "feedback": t.feedback.iter().map(|f| json!({ "a": f.address, "label": default_label(&f.label) })).collect::<Vec<_>>(),
                })
            })
            .collect();
        let windows: Vec<Value> = cfg
            .windows
            .iter()
            .map(|w| json!({ "name": default_label(&w.name), "fps": w.fps, "zones": w.zones.iter().map(|z| json!({ "name": default_label(&z.name), "w": z.w, "h": z.h })).collect::<Vec<_>>() }))
            .collect();
        json!({
            "seq": s.seq,
            "lang": if cfg.language == star_core::i18n::Lang::En { "en" } else { "fr" },
            // Bloc « yamaha » : la console choisie, quelle qu'elle soit (la page ne fait pas la
            // différence ; la M32 est traduite en paramètres Yamaha par midas.rs).
            "yamaha": match cfg.console {
                crate::config::ConsoleKind::Yamaha => json!({
                    "kind": "yamaha",
                    "enabled": cfg.yamaha.enabled,
                    "connected": s.yamaha.connected, "model": s.yamaha.model, "error": s.yamaha.error,
                    "channels": cfg.yamaha.channels, "mixes": cfg.yamaha.mixes,
                    "freq_scale": cfg.yamaha.freq_scale, "q_scale": cfg.yamaha.q_scale,
                    "meters": cfg.meters().0,
                    "meter_fps": cfg.meters().1,
                }),
                crate::config::ConsoleKind::None => json!({ "kind": "none", "enabled": false, "connected": false, "model": "", "error": "", "channels": 0, "mixes": 0, "meters": false, "meter_fps": 1 }),
                crate::config::ConsoleKind::M32 => json!({
                    "kind": "m32",
                    "enabled": true,
                    "connected": s.yamaha.connected, "model": s.yamaha.model, "error": s.yamaha.error,
                    "channels": cfg.m32.channels, "mixes": cfg.m32.buses,
                    "freq_scale": crate::midas::FREQ_SCALE, "q_scale": crate::midas::Q_SCALE,
                    "meters": cfg.meters().0,
                    "meter_fps": cfg.meters().1,
                }),
            },
            "params": params,
            "targets": targets,
            "osc": osc,
            "osc_alive": s.osc_alive,
            "windows": windows,
            "free_windows": cfg.free_windows && crate::capture::AVAILABLE,
        })
    }

    /// Note qu'un client en long-polling est passé (pour compter les clients sur le PC).
    pub fn poll_seen(&self, who: String) {
        if let Ok(mut p) = self.poll_clients.lock() {
            p.insert(who, Instant::now());
            p.retain(|_, t| t.elapsed() < Duration::from_secs(30));
        }
    }

    pub fn poll_count(&self) -> usize {
        self.poll_clients.lock().map(|p| p.values().filter(|t| t.elapsed() < Duration::from_secs(30)).count()).unwrap_or(0)
    }

    /// Nouvelle config : prévient les clients et fait reconnecter la console.
    pub fn replace_config(&self, c: Config) {
        if let Ok(mut cur) = self.config.write() {
            *cur = c;
        }
        self.generation.fetch_add(1, Ordering::Relaxed);
        self.push(json!({ "e": "cfg" }));
    }
}

/// Libellés des boutons et retours par défaut (écrits en français dans oldstar.json) : en
/// anglais si l'interface l'est. Un libellé changé à la main reste tel quel.
fn default_label(label: &str) -> &str {
    if !star_core::i18n::is_en() {
        return label;
    }
    match label {
        "En attente" => "Standby",
        "À l'écran" => "On screen",
        "Reste (s)" => "Remaining (s)",
        "◀ Cue préc." => "◀ Prev cue",
        "Cue suiv. ▶" => "Next cue ▶",
        "Noir" => "Black",
        "Tout arrêter" => "Stop all",
        "Bloc-notes" => "Notepad",
        "Haut gauche" => "Top left",
        other => other,
    }
}
