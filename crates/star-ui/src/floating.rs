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

// ---------------------------------------------------------------------------------------------
// Place des fenêtres flottantes : chacune se rouvre là où elle a été fermée (même écran, même
// taille), d'une ouverture à l'autre et, si l'appli a donné un fichier avec `remember_in`,
// d'un lancement à l'autre.
//
// La position est retenue en pixels de l'écran : avec deux écrans à des échelles différentes
// (portable à 150 %, projecteur à 100 %), des points ne désignent pas le même endroit selon la
// fenêtre qui les convertit. À l'ouverture, la position est donc réappliquée depuis la fenêtre
// elle-même, avec sa propre échelle.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Place retenue : coin haut-gauche extérieur en pixels de l'écran, taille intérieure en points.
type Place = [f32; 4];

struct Opened {
    /// Place appliquée à cette ouverture (None : première ouverture, place choisie par le système).
    place: Option<Place>,
    /// Dernière image de la fenêtre principale où la fenêtre était affichée.
    seen: u64,
    /// Images de la fenêtre flottante depuis son ouverture.
    frames: u32,
}

#[derive(Default)]
struct Places {
    path: Option<PathBuf>,
    saved: HashMap<String, Place>,
    opened: HashMap<String, Opened>,
    dirty: bool,
    written: Option<Instant>,
}

static PLACES: Mutex<Option<Places>> = Mutex::new(None);

fn with<R>(f: impl FnOnce(&mut Places) -> R) -> R {
    let mut g = PLACES.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Places::default))
}

impl Places {
    fn write(&mut self) {
        self.dirty = false;
        self.written = Some(Instant::now());
        if let Some(p) = &self.path {
            let _ = star_core::prefs::save(p, &self.saved);
        }
    }
}

/// Retient la place des fenêtres flottantes dans `path` (JSON), relu tout de suite.
pub fn remember_in(path: PathBuf) {
    let saved = star_core::prefs::load(&path).unwrap_or_default();
    with(|p| {
        p.saved = saved;
        p.path = Some(path);
    });
}

/// Écrit les places modifiées (à appeler en quittant l'appli).
pub fn save_places() {
    with(|p| {
        if p.dirty {
            p.write();
        }
    });
}

/// Builder d'une fenêtre flottante `key`, complété de sa place retenue. À appeler à chaque
/// image avec le contexte de la fenêtre principale, avant `show_viewport_immediate`.
pub fn placed(ctx: &egui::Context, key: &str, vp: egui::ViewportBuilder) -> egui::ViewportBuilder {
    let frame = ctx.cumulative_frame_nr_for(egui::ViewportId::ROOT);
    let ppp = ctx.pixels_per_point();
    with(|p| {
        // Pas affichée à l'image précédente : c'est une nouvelle ouverture.
        let reopened = p.opened.get(key).is_none_or(|o| o.seen + 1 < frame);
        if reopened {
            let place = p.saved.get(key).copied();
            p.opened.insert(key.to_owned(), Opened { place, seen: frame, frames: 0 });
        }
        let o = p.opened.get_mut(key).expect("ouverture enregistrée");
        o.seen = frame;
        // Toujours la même place pendant toute l'ouverture : egui déplacerait la fenêtre si le
        // builder changeait d'une image à l'autre.
        match o.place {
            Some([x, y, w, h]) => vp.with_position([x / ppp, y / ppp]).with_inner_size([w, h]),
            None => vp,
        }
    })
}

/// À appeler à chaque image dans la fenêtre flottante `key` (contexte de cette fenêtre) :
/// la remet à sa place à l'ouverture, puis retient où elle est.
pub fn track(ctx: &egui::Context, key: &str) {
    let ppp = ctx.pixels_per_point();
    let (outer, inner, minimized) = ctx.input(|i| {
        let v = i.viewport();
        (v.outer_rect, v.inner_rect, v.minimized.unwrap_or(false))
    });
    with(|p| {
        let Some(o) = p.opened.get_mut(key) else { return };
        o.frames = o.frames.saturating_add(1);
        if o.frames == 1 {
            // Position en pixels convertie avec l'échelle de cette fenêtre-ci.
            if let Some([x, y, ..]) = o.place {
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x / ppp, y / ppp)));
            }
            return;
        }
        // Laisser le temps au déplacement de l'ouverture d'arriver avant de relever la place.
        if o.frames < 4 || minimized {
            return;
        }
        let (Some(outer), Some(inner)) = (outer, inner) else { return };
        let place = [(outer.min.x * ppp).round(), (outer.min.y * ppp).round(), inner.width().round(), inner.height().round()];
        if place[2] < 1.0 || place[3] < 1.0 || p.saved.get(key) == Some(&place) {
            return;
        }
        p.saved.insert(key.to_owned(), place);
        p.dirty = true;
        // Enregistrée au plus une fois par seconde pendant un déplacement.
        if p.written.is_none_or(|t| t.elapsed() >= Duration::from_secs(1)) {
            p.write();
        }
    });
}
