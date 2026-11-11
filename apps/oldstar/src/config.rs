//! Réglages d'OldStar : un fichier JSON lisible et modifiable à la main (oldstar.json, dans le
//! dossier de réglages de la suite, ou celui passé par `--config`). Tous les champs ont une
//! valeur par défaut : un fichier partiel est complété.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Port HTTP de la page iPad (et du WebSocket, même port).
    pub http_port: u16,
    /// Console pilotée : « yamaha » (CL/QL) ou « m32 » (Midas M32 / Behringer X32), une seule à la fois.
    pub console: ConsoleKind,
    pub yamaha: YamahaConfig,
    pub m32: M32Config,
    pub osc: Vec<OscTarget>,
    pub windows: Vec<WindowConfig>,
    /// La page peut afficher n'importe quelle fenêtre ouverte du PC (en plus de `windows`).
    pub free_windows: bool,
    pub drag: DragConfig,
}

/// Glisser dans une fenêtre capturée (déplacer un module dans Lake…).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct DragConfig {
    /// Les glissers passent toujours par de vrais mouvements de souris (premier plan), même
    /// en mode arrière-plan : les messages envoyés à la fenêtre ne savent pas faire glisser.
    pub always_real: bool,
    pub before_down_ms: u64,
    pub hold_ms: u64,
    pub step_px: i32,
    pub step_ms: u64,
    pub before_up_ms: u64,
    pub after_up_ms: u64,
}

impl Default for DragConfig {
    fn default() -> Self {
        let t = crate::capture::DragTiming::default();
        Self {
            always_real: true,
            before_down_ms: t.before_down_ms,
            hold_ms: t.hold_ms,
            step_px: t.step_px,
            step_ms: t.step_ms,
            before_up_ms: t.before_up_ms,
            after_up_ms: t.after_up_ms,
        }
    }
}

impl DragConfig {
    pub fn timing(&self) -> crate::capture::DragTiming {
        crate::capture::DragTiming {
            before_down_ms: self.before_down_ms.min(2000),
            hold_ms: self.hold_ms.min(3000),
            step_px: self.step_px.clamp(1, 200),
            step_ms: self.step_ms.min(200),
            before_up_ms: self.before_up_ms.min(2000),
            after_up_ms: self.after_up_ms.min(3000),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            http_port: 8080,
            console: ConsoleKind::Yamaha,
            yamaha: YamahaConfig::default(),
            m32: M32Config::default(),
            osc: default_targets(),
            windows: default_windows(),
            free_windows: true,
            drag: DragConfig::default(),
        }
    }
}

impl Config {
    /// Vu-mètres de la console choisie : (actifs, images par seconde vers la page).
    pub fn meters(&self) -> (bool, u32) {
        match self.console {
            ConsoleKind::Yamaha => (self.yamaha.meters.mode != MeterMode::Off, self.yamaha.meters.fps.clamp(1, 30)),
            ConsoleKind::M32 => (self.m32.meters, self.m32.meter_fps.clamp(1, 30)),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleKind {
    #[default]
    Yamaha,
    M32,
}

impl std::str::FromStr for ConsoleKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        match s.to_ascii_lowercase().as_str() {
            "yamaha" | "cl" | "ql" => Ok(Self::Yamaha),
            "m32" | "x32" | "midas" | "behringer" => Ok(Self::M32),
            other => Err(format!("console inconnue : {other} (yamaha ou m32)")),
        }
    }
}

/// Midas M32 / Behringer X32 : OSC sur UDP 10023.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct M32Config {
    pub host: String,
    pub port: u16,
    /// Voies d'entrée affichées (M32 : 32).
    pub channels: u32,
    /// Bus de mixage (M32 : 16).
    pub buses: u32,
    pub meters: bool,
    /// Images par seconde des vu-mètres envoyées à la page.
    pub meter_fps: u32,
}

impl Default for M32Config {
    fn default() -> Self {
        Self { host: "127.0.0.1".into(), port: 10023, channels: 32, buses: 16, meters: true, meter_fps: 12 }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct YamahaConfig {
    pub enabled: bool,
    /// Adresse IP de la console.
    pub host: String,
    pub port: u16,
    /// Nombre de voies d'entrée affichées (QL1 : 40, CL5 : 80, voies stéréo comprises).
    pub channels: u32,
    /// Nombre de mix (bus Aux / Mix).
    pub mixes: u32,
    /// Échelles des valeurs brutes de l'EQ : valeur brute = valeur × échelle.
    /// Gain en dB × 100 ; fréquence en Hz × freq_scale ; Q × q_scale. À vérifier sur la vraie console.
    pub freq_scale: f64,
    pub q_scale: f64,
    pub meters: MeterConfig,
}

impl Default for YamahaConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            host: "127.0.0.1".into(),
            port: 49280,
            channels: 40,
            mixes: 16,
            freq_scale: 10.0,
            q_scale: 1000.0,
            meters: MeterConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MeterMode {
    /// Commandes d'abonnement envoyées à la connexion ; la console pousse des lignes « NOTIFY mtr ».
    Subscribe,
    /// « get » périodique de l'adresse de niveau, voie par voie.
    Poll,
    Off,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct MeterConfig {
    pub mode: MeterMode,
    /// Commandes envoyées à la connexion en mode Subscribe.
    pub subscribe: Vec<String>,
    /// Adresses interrogées en mode Poll (x = voie ou mix, y = 0).
    pub poll_inch: String,
    pub poll_mix: String,
    pub poll_ms: u64,
    /// Plage des valeurs brutes reçues (min = silence, max = crête).
    pub raw_min: f64,
    pub raw_max: f64,
    /// Images par seconde envoyées à l'iPad (10 à 15).
    pub fps: u32,
}

impl Default for MeterConfig {
    fn default() -> Self {
        Self {
            mode: MeterMode::Subscribe,
            subscribe: vec!["mtrstart MIXER:Current/Meter/InCh 100".into(), "mtrstart MIXER:Current/Meter/Mix 100".into()],
            poll_inch: "MIXER:Current/Meter/InCh/PostOn".into(),
            poll_mix: "MIXER:Current/Meter/Mix/PostOn".into(),
            poll_ms: 100,
            raw_min: 0.0,
            raw_max: 127.0,
            fps: 12,
        }
    }
}

/// Une appli pilotée en OSC (UDP).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct OscTarget {
    pub name: String,
    pub host: String,
    pub port: u16,
    /// Préfixe des adresses (« /cuestar »).
    pub prefix: String,
    /// Abonnement au retour d'état (« <préfixe>/subscribe <port> », renouvelé toutes les 3 s).
    pub subscribe: bool,
    /// Valeurs du retour d'état affichées sur l'iPad (adresse sans le préfixe → libellé).
    pub feedback: Vec<Feedback>,
    pub buttons: Vec<OscButton>,
}

impl Default for OscTarget {
    fn default() -> Self {
        Self { name: "Appli".into(), host: "127.0.0.1".into(), port: 9000, prefix: String::new(), subscribe: false, feedback: vec![], buttons: vec![] }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Feedback {
    pub address: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct OscButton {
    pub label: String,
    /// Adresse sans le préfixe de la cible (« cuelist/go »).
    pub address: String,
    /// Arguments : nombres (entiers ou flottants), booléens ou textes.
    pub args: Vec<serde_json::Value>,
    /// go, stop, accent ou vide.
    pub style: String,
    /// Bouton large (deux colonnes).
    pub wide: bool,
}

impl Default for OscButton {
    fn default() -> Self {
        Self { label: String::new(), address: String::new(), args: vec![], style: String::new(), wide: false }
    }
}

fn b(label: &str, address: &str, style: &str, wide: bool) -> OscButton {
    OscButton { label: label.into(), address: address.into(), args: vec![], style: style.into(), wide }
}

fn f(address: &str, label: &str) -> Feedback {
    Feedback { address: address.into(), label: label.into() }
}

/// Adresses de CueStar (apps/cuestar/src/osc.rs) et de JingleStar (apps/jinglestar/src/osc.rs),
/// ports par défaut de leurs réglages OSC.
fn default_targets() -> Vec<OscTarget> {
    vec![
        OscTarget {
            name: "CueStar".into(),
            host: "127.0.0.1".into(),
            port: 9000,
            prefix: "/cuestar".into(),
            subscribe: true,
            feedback: vec![f("cuelist/standby_name", "En attente"), f("media", "À l'écran"), f("remaining", "Reste (s)")],
            buttons: vec![
                b("GO", "cuelist/go", "go", true),
                b("Stop", "cuelist/stop", "stop", false),
                b("Pause", "cuelist/toggle", "", false),
                b("◀ Cue préc.", "cuelist/prev", "", false),
                b("Cue suiv. ▶", "cuelist/next", "", false),
                b("Noir", "noir", "accent", true),
            ],
        },
        OscTarget {
            name: "JingleStar".into(),
            host: "127.0.0.1".into(),
            port: 8000,
            prefix: "/jinglestar".into(),
            // Le retour d'état de JingleStar ne contient que les niveaux.
            subscribe: false,
            feedback: vec![],
            buttons: vec![
                b("GO", "go", "go", true),
                b("Tout arrêter", "stop_all", "stop", true),
                b("◀ Cue préc.", "module/1/cue/previous", "", false),
                b("Cue suiv. ▶", "module/1/cue/next", "", false),
                b("Pause cues", "module/1/cue/pause", "", false),
                b("Playlist ▶/❚❚", "module/2/playlist/pause", "", false),
            ],
        },
    ]
}

/// Une fenêtre capturée, retrouvée par un bout de son titre.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    /// Nom affiché sur l'iPad.
    pub name: String,
    /// Texte cherché dans le titre, sans tenir compte des majuscules ; plusieurs possibles
    /// séparés par « | ». Vide : le titre n'est pas regardé.
    pub title: String,
    /// Noms possibles du programme, avec * et ? (« lake*.exe ») ; vide : pas regardé.
    pub exe: Vec<String>,
    /// Bout du chemin du programme (« \Lake\ ») ; vide : pas regardé.
    /// Tous les critères remplis doivent correspondre ; si plusieurs fenêtres conviennent,
    /// la plus grande non réduite est prise.
    pub path: String,
    /// Images par seconde au plus.
    pub fps: u32,
    /// Largeur maximale de l'image envoyée (pixels), pour soulager le Wi-Fi et l'iPad 2.
    pub max_width: u32,
    pub jpeg_quality: u8,
    /// sendinput : clics réels (la fenêtre passe au premier plan) ;
    /// postmessage : messages envoyés à la fenêtre, qui peut rester derrière (selon l'appli).
    pub input: String,
    /// Recadrages affichés en grand (pixels de la zone cliente de la fenêtre).
    pub zones: Vec<Zone>,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self { name: "Fenêtre".into(), title: String::new(), exe: vec![], path: String::new(), fps: 4, max_width: 1024, jpeg_quality: 60, input: "sendinput".into(), zones: vec![] }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Zone {
    pub name: String,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

fn default_windows() -> Vec<WindowConfig> {
    vec![
        // Lake Controller : relevé en 8.1.7 (« Lake Controller.exe », titre « Lake Controller 8.1.7 »).
        // Le motif couvre les autres versions tant que l'exe commence par « Lake ».
        WindowConfig { name: "Lake".into(), exe: vec!["lake*.exe".into()], ..Default::default() },
        // Titres supposés des éditeurs Yamaha (à vérifier sur un PC où ils sont installés).
        WindowConfig { name: "Yamaha Editor".into(), title: "CL Editor|QL Editor|CL/QL Editor".into(), ..Default::default() },
        WindowConfig {
            name: "Bloc-notes".into(),
            title: "Bloc-notes|Notepad".into(),
            zones: vec![Zone { name: "Haut gauche".into(), x: 0, y: 0, w: 400, h: 300 }],
            ..Default::default()
        },
    ]
}

pub fn default_path() -> Option<PathBuf> {
    crate::APP.config_file("oldstar.json")
}

impl Config {
    /// Lit le fichier ; s'il manque, l'écrit avec les valeurs par défaut pour qu'on puisse le modifier.
    pub fn load_or_create(path: &Path) -> Result<Self, String> {
        match std::fs::read_to_string(path) {
            Ok(text) => serde_json::from_str(&text).map_err(|e| format!("{} : {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let c = Self::default();
                c.save(path)?;
                Ok(c)
            }
            Err(e) => Err(format!("{} : {e}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(path, text).map_err(|e| format!("{} : {e}", path.display()))
    }
}
