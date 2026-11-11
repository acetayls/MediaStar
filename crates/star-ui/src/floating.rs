//! Fenêtres flottantes : de vraies fenêtres du système, à part de la fenêtre principale
//! (elles peuvent aller sur un autre écran, ou passer derrière la régie).

/// Bouton d'une fenêtre flottante dans la fenêtre principale : l'ouvre si elle est fermée,
/// sinon la ramène au premier plan (elle a pu passer derrière la fenêtre principale ou être
/// réduite). Rend `true` si la fenêtre vient d'être ouverte.
pub fn open_or_raise(ctx: &egui::Context, open: &mut bool, id: egui::ViewportId) -> bool {
    if *open {
        raise(ctx, id);
        false
    } else {
        *open = true;
        true
    }
}

/// Ramène une fenêtre flottante déjà ouverte au premier plan (et la rouvre si elle était réduite).
pub fn raise(ctx: &egui::Context, id: egui::ViewportId) {
    ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Minimized(false));
    ctx.send_viewport_cmd_to(id, egui::ViewportCommand::Focus);
}
