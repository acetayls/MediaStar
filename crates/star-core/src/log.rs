//! Journal de l'application, à côté de la session (`jinglestar.log`, `cuestar.log`), et trace
//! des plantages : sans console sous Windows, c'est la seule trace qui reste.
//!
//! [`init`] au lancement choisit le fichier ; avant (et dans les tests), rien n'est écrit.

use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::app::App;
pub use crate::time::now;

/// Au-delà, le journal repart d'un fichier vide (l'ancien est gardé en `.old.log`).
const MAX_BYTES: u64 = 1_000_000;

static PATH: OnceLock<PathBuf> = OnceLock::new();

/// Journal de `app` pour tout le processus. Sans effet après le premier appel.
pub fn init(app: App) {
    if let Some(p) = app.log_path() {
        let _ = PATH.set(p);
    }
}

/// Fichier du journal, une fois [`init`] appelé.
pub fn log_path() -> Option<PathBuf> {
    PATH.get().cloned()
}

/// Niveau d'une ligne du journal, pour filtrer l'affichage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Level {
    Debug,
    Info,
    Warning,
    Critical,
}

impl Level {
    pub const ALL: [Level; 4] = [Level::Debug, Level::Info, Level::Warning, Level::Critical];
    /// Repère écrit après la date dans le journal.
    pub fn tag(self) -> &'static str {
        match self {
            Level::Debug => "DEBUG",
            Level::Info => "INFO",
            Level::Warning => "ALERTE",
            Level::Critical => "CRITIQUE",
        }
    }
    /// Niveau d'une ligne du journal, ou None si elle n'en porte pas (suite d'un message sur
    /// plusieurs lignes, ou journal d'avant les niveaux).
    pub fn of_line(line: &str) -> Option<Level> {
        // « 2026-11-07 23:00:00 UTC ALERTE … » : le repère suit la date.
        let head = line.get(..line.len().min(40))?;
        Level::ALL.into_iter().find(|l| head.contains(&format!(" UTC {} ", l.tag())))
    }
}

/// Ajoute une ligne datée au journal, au niveau Info. Ne bloque jamais l'application si le disque refuse.
pub fn write(msg: &str) {
    write_level(Level::Info, msg);
}

/// Message de mise au point.
pub fn debug(msg: &str) {
    write_level(Level::Debug, msg);
}

/// Problème dont l'application se remet (fichier illisible, sortie perdue…).
pub fn warn(msg: &str) {
    write_level(Level::Warning, msg);
}

/// Panne grave : plantage, arrêt sur erreur.
pub fn critical(msg: &str) {
    write_level(Level::Critical, msg);
}

/// Ajoute une ligne datée au journal, avec son niveau.
pub fn write_level(level: Level, msg: &str) {
    let Some(path) = PATH.get() else { return };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = std::fs::rename(path, path.with_extension("old.log"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{} {} {msg}", now(false), level.tag());
    }
}

/// Inscrit chaque panique au journal (thread, message, emplacement), puis laisse faire
/// le comportement habituel. Les paniques rattrapées sont aussi notées. `started` : la ligne
/// de démarrage, par exemple « Démarrage de JingleStar 0.1.4 ».
pub fn install_panic_hook(started: &str) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        let bt = std::backtrace::Backtrace::force_capture();
        critical(&format!("PANIQUE dans le thread « {thread} » : {info}\n{bt}"));
        previous(info);
    }));
    write(started);
}

#[cfg(test)]
mod tests {
    use super::Level;

    #[test]
    fn niveau_des_lignes() {
        assert_eq!(Level::of_line("2026-11-07 23:00:00 UTC ALERTE Sortie : x"), Some(Level::Warning));
        assert_eq!(Level::of_line("2026-11-07 23:00:00 UTC INFO Projet ouvert"), Some(Level::Info));
        assert_eq!(Level::of_line("2026-11-07 23:00:00 UTC Projet ouvert"), None);
        assert_eq!(Level::of_line("   0: std::backtrace"), None);
        assert_eq!(Level::of_line(""), None);
    }
}
