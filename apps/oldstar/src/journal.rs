//! Journal d'OldStar : tous les messages du serveur (démarrage, adresses, appareils, console,
//! erreurs), horodatés. Ils s'affichent dans le panneau Journal de la fenêtre, vont dans le
//! fichier oldstar.log (dossier de réglages) et, en mode `--sans-fenetre`, dans la console.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
    /// Détail (touchers reçus…) : panneau et console, pas le fichier.
    Detail,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub time: String,
    pub level: Level,
    pub text: String,
}

/// Lignes gardées pour le panneau.
const KEEP: usize = 3000;

static LINES: Mutex<VecDeque<Line>> = Mutex::new(VecDeque::new());
static CONSOLE: AtomicBool = AtomicBool::new(true);

/// Recopie aussi les lignes dans la console (mode sans fenêtre).
pub fn set_console(on: bool) {
    CONSOLE.store(on, Ordering::Relaxed);
}

/// Heure locale HH:MM:SS sous Windows ; ailleurs, heure UTC (sans dépendance de fuseau).
#[cfg(windows)]
fn clock() -> String {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:02}:{:02}:{:02}", t.wHour, t.wMinute, t.wSecond)
}

#[cfg(not(windows))]
fn clock() -> String {
    let full = star_core::time::now(false);
    full.split_whitespace().nth(1).unwrap_or("").to_string() + " UTC"
}

pub fn push(level: Level, text: &str) {
    match level {
        Level::Info => star_core::log::write(text),
        Level::Warn => star_core::log::warn(text),
        Level::Error => star_core::log::critical(text),
        Level::Detail => {}
    }
    if CONSOLE.load(Ordering::Relaxed) {
        if level == Level::Error || level == Level::Warn {
            eprintln!("{text}");
        } else {
            println!("{text}");
        }
    }
    if let Ok(mut l) = LINES.lock() {
        l.push_back(Line { time: clock(), level, text: text.to_string() });
        while l.len() > KEEP {
            l.pop_front();
        }
    }
}

pub fn info(text: &str) {
    push(Level::Info, text);
}

pub fn warn(text: &str) {
    push(Level::Warn, text);
}

pub fn error(text: &str) {
    push(Level::Error, text);
}

pub fn detail(text: &str) {
    push(Level::Detail, text);
}

pub fn lines() -> Vec<Line> {
    LINES.lock().map(|l| l.iter().cloned().collect()).unwrap_or_default()
}

pub fn clear() {
    if let Ok(mut l) = LINES.lock() {
        l.clear();
    }
}

/// Tout le journal en texte, pour le presse-papiers.
pub fn text() -> String {
    lines().iter().map(|l| format!("{} {}", l.time, l.text)).collect::<Vec<_>>().join("\n")
}
