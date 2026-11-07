//! Onglet « Log » des Préférences : le journal `cuestar.log` (régie et sortie), relu quand il
//! change. Les messages de la barre d'état y sont aussi inscrits, pour les relire en entier.

use egui::{RichText, TextStyle};
use star_ui::theme::*;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

/// Fin du journal affichée : au-delà, la fenêtre deviendrait lente à faire défiler.
const MAX_SHOWN: usize = 200_000;
/// Intervalle entre deux coups d'œil au fichier, tant que l'onglet est affiché.
const POLL: Duration = Duration::from_millis(500);

pub struct LogView {
    text: String,
    /// Taille et date du fichier à la dernière lecture : on ne relit que s'il a changé.
    seen: Option<(u64, Option<SystemTime>)>,
    last_poll: Option<Instant>,
    follow: bool,
    error: Option<String>,
}

impl Default for LogView {
    fn default() -> Self {
        Self { text: String::new(), seen: None, last_poll: None, follow: true, error: None }
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
            if ui.button("Copier").on_hover_text("Copier tout le journal affiché").clicked() {
                ui.ctx().copy_text(self.text.clone());
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
        if let Some(e) = &self.error {
            ui.label(RichText::new(e).color(STOP));
        }
        ui.separator();

        if self.text.is_empty() {
            ui.label(RichText::new("Journal vide.").color(TEXT_DIM));
            return;
        }
        egui::ScrollArea::both().auto_shrink(false).stick_to_bottom(self.follow).show(ui, |ui| {
            // Lecture seule, mais sélectionnable pour copier un passage.
            let mut shown = self.text.as_str();
            ui.add(egui::TextEdit::multiline(&mut shown).font(TextStyle::Monospace).desired_width(f32::INFINITY).code_editor());
        });
    }
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
    use super::tail;

    #[test]
    fn tail_starts_on_a_whole_line() {
        assert_eq!(tail("abc", 10), "abc");
        assert_eq!(tail("ligne 1\nligne 2\nligne 3\n", 10), "ligne 3\n");
        // Coupure au milieu d'un caractère accentué : pas de panique.
        assert_eq!(tail("ééé\nfin", 6), "fin");
    }
}
