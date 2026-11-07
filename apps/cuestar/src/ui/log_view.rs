//! Onglet « Log » des Préférences : le journal `cuestar.log` (régie et sortie), relu quand il
//! change. Les messages de la barre d'état y sont aussi inscrits, pour les relire en entier.

use egui::{RichText, TextStyle};
use star_core::log::Level;
use star_ui::theme::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Fin du journal affichée : au-delà, la fenêtre deviendrait lente à faire défiler.
const MAX_SHOWN: usize = 200_000;
/// Intervalle entre deux coups d'œil au fichier, tant que l'onglet est affiché.
const POLL: Duration = Duration::from_millis(500);

pub struct LogView {
    text: String,
    /// Lignes affichées : le journal filtré selon les niveaux cochés.
    shown: String,
    /// Niveaux affichés (Debug, Info, Alerte, Critique).
    levels: [bool; 4],
    /// Taille et date du fichier à la dernière lecture : on ne relit que s'il a changé.
    seen: Option<(u64, Option<SystemTime>)>,
    last_poll: Option<Instant>,
    follow: bool,
    error: Option<String>,
}

impl Default for LogView {
    fn default() -> Self {
        Self { text: String::new(), shown: String::new(), levels: [true; 4], seen: None, last_poll: None, follow: true, error: None }
    }
}

impl LogView {
    fn refresh(&mut self, path: &Path, force: bool) {
        if !force && self.last_poll.is_some_and(|t| t.elapsed() < POLL) {
            return;
        }
        self.last_poll = Some(Instant::now());
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(_) => {
                self.text.clear();
                self.seen = None;
                self.error = None;
                return;
            }
        };
        let stamp = (meta.len(), meta.modified().ok());
        if !force && self.seen == Some(stamp) {
            return;
        }
        match std::fs::read(path) {
            Ok(bytes) => {
                let all = String::from_utf8_lossy(&bytes);
                self.text = tail(&all, MAX_SHOWN).to_string();
                self.shown = filter(&self.text, self.levels);
                self.seen = Some(stamp);
                self.error = None;
            }
            Err(e) => self.error = Some(format!("Lecture du journal impossible : {e}")),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui) {
        let Some(path) = star_core::log::log_path() else {
            ui.label(RichText::new("Pas de journal pour cette session.").color(TEXT_DIM));
            return;
        };
        self.refresh(&path, false);
        ui.ctx().request_repaint_after(POLL);

        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(path.display().to_string()).color(TEXT_DIM));
            ui.separator();
            ui.checkbox(&mut self.follow, "Suivre la fin").on_hover_text("Descendre automatiquement aux dernières lignes");
            if ui.button("Copier").on_hover_text("Copier les lignes affichées").clicked() {
                ui.ctx().copy_text(self.shown.clone());
            }
            if ui.button("Ouvrir le dossier").on_hover_text("Dossier du journal (cuestar.log, cuestar.old.log)").clicked() {
                if let Some(dir) = path.parent() {
                    if let Err(e) = open_path(dir) {
                        self.error = Some(e);
                    }
                }
            }
            if ui.button("Actualiser").clicked() {
                self.refresh(&path, true);
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Niveaux").color(TEXT_DIM));
            let mut changed = false;
            for (i, l) in Level::ALL.into_iter().enumerate() {
                let (name, color, tip) = match l {
                    Level::Debug => ("Debug", TEXT_DIM, "Détails de mise au point"),
                    Level::Info => ("Info", TEXT, "Déroulement normal : démarrage, projet ouvert, enregistré…"),
                    Level::Warning => ("Alerte", METER_MID, "Problème dont CueStar se remet : fichier illisible, erreur de sortie…"),
                    Level::Critical => ("Critique", STOP, "Panne grave : plantage, arrêt sur erreur"),
                };
                let on = self.levels[i];
                let b = egui::Button::new(RichText::new(name).color(if on { color } else { TEXT_DIM })).fill(if on { PANEL_HI } else { BG }).selected(on);
                if ui.add(b).on_hover_text(tip).clicked() {
                    self.levels[i] = !on;
                    changed = true;
                }
            }
            if changed {
                self.shown = filter(&self.text, self.levels);
            }
        });
        if let Some(e) = &self.error {
            ui.label(RichText::new(e).color(STOP));
        }
        ui.separator();

        if self.text.is_empty() {
            ui.label(RichText::new("Journal vide.").color(TEXT_DIM));
            return;
        }
        if self.shown.is_empty() {
            ui.label(RichText::new("Aucune ligne à ces niveaux.").color(TEXT_DIM));
            return;
        }
        egui::ScrollArea::both().auto_shrink(false).stick_to_bottom(self.follow).show(ui, |ui| {
            // Lecture seule, mais sélectionnable pour copier un passage.
            let mut shown = self.shown.as_str();
            ui.add(egui::TextEdit::multiline(&mut shown).font(TextStyle::Monospace).desired_width(f32::INFINITY).code_editor());
        });
    }
}

/// Lignes du journal aux niveaux cochés (`levels` dans l'ordre de [`Level::ALL`]). Une ligne sans
/// niveau suit la ligne datée qui la précède (trace d'un plantage) ; avant les niveaux, Info.
fn filter(text: &str, levels: [bool; 4]) -> String {
    if levels.iter().all(|l| *l) {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut current = Level::Info;
    for line in text.split_inclusive('\n') {
        if let Some(l) = Level::of_line(line) {
            current = l;
        } else if line.get(..4).is_some_and(|y| y.bytes().all(|b| b.is_ascii_digit())) {
            // Ligne datée d'avant les niveaux.
            current = Level::Info;
        }
        if levels[Level::ALL.iter().position(|l| *l == current).unwrap_or(1)] {
            out.push_str(line);
        }
    }
    out
}

/// Fin de `s` sur au plus `max` octets, coupée au début d'une ligne.
fn tail(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut start = s.len() - max;
    while !s.is_char_boundary(start) {
        start += 1;
    }
    match s[start..].find('\n') {
        Some(i) => &s[start + i + 1..],
        None => &s[start..],
    }
}

/// Ouvre un dossier dans l'explorateur de fichiers du système.
fn open_path(p: &Path) -> Result<(), String> {
    let p: PathBuf = p.to_path_buf();
    #[cfg(windows)]
    let r = std::process::Command::new("explorer.exe").arg(&p).spawn();
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(&p).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(&p).spawn();
    r.map(|_| ()).map_err(|e| format!("Ouverture du dossier impossible : {e}"))
}

#[cfg(test)]
mod tests {
    use super::{filter, tail};

    #[test]
    fn filtre_par_niveau() {
        let log = "2026-11-07 10:00:00 UTC Démarrage (ancien format)\n\
                   2026-11-07 10:00:01 UTC INFO Projet ouvert\n\
                   2026-11-07 10:00:02 UTC ALERTE Sortie : fichier illisible\n\
                   2026-11-07 10:00:03 UTC CRITIQUE PANIQUE dans le thread « main »\n\
                   \x20  0: trace\n\
                   2026-11-07 10:00:04 UTC DEBUG détail\n";
        let only_warn = filter(log, [false, false, true, true]);
        assert_eq!(only_warn.lines().count(), 3);
        assert!(only_warn.contains("ALERTE") && only_warn.contains("0: trace") && !only_warn.contains("Projet"));
        let info = filter(log, [false, true, false, false]);
        assert_eq!(info.lines().count(), 2);
        assert_eq!(filter(log, [true; 4]), log);
    }

    #[test]
    fn tail_starts_on_a_whole_line() {
        assert_eq!(tail("abc", 10), "abc");
        assert_eq!(tail("ligne 1\nligne 2\nligne 3\n", 10), "ligne 3\n");
        // Coupure au milieu d'un caractère accentué : pas de panique.
        assert_eq!(tail("ééé\nfin", 6), "fin");
    }
}
