//! Barre du haut commune aux applis Star : même hauteur, même logo, même nom, mêmes onglets
//! segmentés et mêmes boutons à icône partout (la version est dans l'aide). Chaque appli y ajoute ses propres boutons.
//!
//! ```ignore
//! app_bar::show(ui, |ui| {
//!     ui.horizontal_centered(|ui| {
//!         app_bar::identity(ui, Logo::Cue, "CueStar", false);
//!         app_bar::tabs(ui, &mut self.tab, &[(Tab::A, "A"), (Tab::B, "B")], app_bar::TAB_WIDTH);
//!     });
//! });
//! ```

use egui::text::{LayoutJob, TextFormat};
use egui::{vec2, Align, Button, FontId, Response, Sense, Ui};

use crate::theme::*;
use crate::icons::{self, Icon};
use crate::widgets;

/// Hauteur de la barre.
pub const HEIGHT: f32 = 48.0;
/// Côté du logo.
pub const LOGO_SIZE: f32 = 26.0;
/// Taille du nom de l'appli (police du logo).
pub const NAME_SIZE: f32 = 22.0;
/// Hauteur des onglets.
pub const TAB_HEIGHT: f32 = 28.0;
/// Largeur d'un onglet.
pub const TAB_WIDTH: f32 = 110.0;
/// Largeur d'un onglet quand la fenêtre est étroite.
pub const TAB_WIDTH_COMPACT: f32 = 85.0;

/// Logo de l'appli : la petite variante de son icône.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Logo {
    /// JingleStar : casque.
    Jingle,
    /// CueStar : lecture.
    Cue,
    /// OldStar : fader et signal sans fil.
    Tablet,
    /// SweepStar : réponse impulsionnelle.
    Impulse,
    /// MediaStar : les quatre modules et l'étoile.
    Media,
}

/// Le panneau du haut, fond `PANEL`, marges 12 × 6. `add` y place le contenu.
pub fn show<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Panel::top("toolbar")
        .exact_size(HEIGHT)
        .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(12, 6)))
        .show(ui, add)
        .inner
}

/// Dessine le logo seul, à la taille donnée.
pub fn logo(ui: &mut Ui, logo: Logo, size: f32) -> Response {
    match logo {
        Logo::Jingle => widgets::logo_mark(ui, size),
        Logo::Cue => widgets::logo_mark_play(ui, size),
        Logo::Tablet => widgets::logo_mark_fader(ui, size),
        Logo::Impulse => widgets::logo_mark_impulse(ui, size),
        Logo::Media => widgets::logo_mark_media(ui, size),
    }
}

/// Logo et nom (« Star » en jaune), à gauche de la barre. La version n'est pas affichée ici
/// mais dans l'aide. `compact` : le logo seul, le nom dans l'infobulle (fenêtre étroite).
pub fn identity(ui: &mut Ui, which: Logo, name: &str, compact: bool) -> Response {
    let resp = logo(ui, which, LOGO_SIZE).on_hover_text(name);
    if compact {
        return resp;
    }
    ui.add_space(6.0);
    let (head, tail) = match name.strip_suffix("Star") {
        Some(h) => (h, "Star"),
        None => (name, ""),
    };
    let font = FontId::new(NAME_SIZE, logo_family());
    let mut job = LayoutJob::default();
    job.append(head, 0.0, TextFormat { font_id: font.clone(), color: TEXT, valign: Align::Center, ..Default::default() });
    job.append(tail, 0.0, TextFormat { font_id: font, color: ACCENT, valign: Align::Center, ..Default::default() });
    ui.add(egui::Label::new(job).wrap_mode(egui::TextWrapMode::Extend).selectable(false));
    resp
}

/// Une rangée d'onglets segmentés (`width` par onglet) ; vrai si l'onglet choisi a changé.
pub fn tabs<T: PartialEq + Copy>(ui: &mut Ui, current: &mut T, items: &[(T, &str)], width: f32) -> bool {
    let labels: Vec<&str> = items.iter().map(|(_, l)| *l).collect();
    let selected = items.iter().position(|(v, _)| v == current).unwrap_or(usize::MAX);
    let (rect, _) = ui.allocate_exact_size(vec2(width * items.len() as f32, TAB_HEIGHT), Sense::hover());
    match widgets::segmented(ui, rect, ("app-bar-tabs", labels.first().copied().unwrap_or("")), &labels, selected) {
        Some(i) if items[i].0 != *current => {
            *current = items[i].0;
            true
        }
        _ => false,
    }
}

/// Un onglet seul, à bascule (même allure que [`tabs`]) ; renvoie sa réponse pour l'infobulle.
pub fn toggle_tab(ui: &mut Ui, on: &mut bool, label: &str, width: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(width, TAB_HEIGHT), Sense::hover());
    if widgets::segmented(ui, rect, ("app-bar-toggle", label), &[label], if *on { 0 } else { 1 }).is_some() {
        *on = !*on;
    }
    resp
}

/// Bouton à icône seule (Préférences, Télécommandes, Aide…), allumé quand sa fenêtre est ouverte.
pub fn icon_button(ui: &mut Ui, icon: Icon, open: bool, tip: &str) -> Response {
    ui.add(Button::selectable(open, icons::atom(icon)).min_size(vec2(32.0, TAB_HEIGHT))).on_hover_text(tip)
}
