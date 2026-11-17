//! Choix d'une transition : menu déroulant du type, puis ses réglages (durée et, selon le
//! type, sens du mouvement ou couleur). Partagé par la Cuelist, le Diaporama et la Présentation.

use crate::protocol::{Direction, Transition, TransitionKind};
use std::hash::Hash;

const FALLBACK: &str = "Sans l'affichage composé (affichage de secours, un seul lecteur), les transitions autres que Coupe et Fondu au noir passent par le noir.";

/// Menu et réglages d'une transition, sur une ligne. Vrai si elle a changé.
pub fn picker(ui: &mut egui::Ui, id: impl Hash + Copy + std::fmt::Debug, t: &mut Transition, max_secs: f64) -> bool {
    let before = *t;
    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt((id, "transition-kind"))
            .selected_text(t.kind.label())
            .width(150.0)
            .show_ui(ui, |ui| {
                for k in TransitionKind::ALL {
                    ui.selectable_value(&mut t.kind, k, k.label()).on_hover_text(k.hint());
                }
            })
            .response
            .on_hover_text(format!("{}\n\n{FALLBACK}", t.kind.hint()));
        if t.kind != TransitionKind::Cut {
            if t.secs < 0.1 {
                t.secs = 1.0_f64.min(max_secs);
            }
            ui.add(egui::DragValue::new(&mut t.secs).range(0.1..=max_secs).speed(0.05).suffix(" s").max_decimals(2)).on_hover_text("Durée de la transition");
        }
        if t.kind.has_direction() {
            egui::ComboBox::from_id_salt((id, "transition-dir"))
                .selected_text(format!("{} {}", t.direction.arrow(), t.direction.label()))
                .width(130.0)
                .show_ui(ui, |ui| {
                    for d in Direction::ALL {
                        ui.selectable_value(&mut t.direction, d, format!("{} {}", d.arrow(), d.label()));
                    }
                })
                .response
                .on_hover_text("Sens du mouvement de la nouvelle image");
        }
        if t.kind == TransitionKind::Dip {
            ui.color_edit_button_srgb(&mut t.color).on_hover_text("Couleur traversée");
        }
    });
    *t != before
}
