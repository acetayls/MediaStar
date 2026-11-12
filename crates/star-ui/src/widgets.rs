//! Widgets dessinés à la main, communs aux applis Star (repris de JingleStar) : formats de
//! temps et de dB, saisie d'un volume, vumètre, commandes de transport, éditeur de forme
//! d'onde, logo.

use std::ops::RangeInclusive;

use egui::{
    pos2, vec2, Align2, Color32, CornerRadius, FontId, Id, Key, Pos2, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Ui,
};
use star_core::i18n::tr;

use crate::theme::*;

/// Durée en m:ss (ou h:mm:ss), arrondie à la seconde ; « --:-- » si elle est inconnue.
pub fn fmt_time(secs: f64) -> String {
    if !secs.is_finite() {
        return "--:--".into();
    }
    let s = secs.max(0.0).round() as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// Temps au dixième de seconde, pour les jingles : 0:03.4
pub fn fmt_tenths(secs: f64) -> String {
    let t = (secs.max(0.0) * 10.0).floor() / 10.0;
    format!("{}:{:04.1}", (t / 60.0) as u64, t % 60.0)
}

/// Volume en dB pour l'affichage : « +1.5 dB », « -∞ dB » sous le plancher.
pub fn fmt_db(db: f32, floor: f32) -> String {
    if db <= floor {
        "-∞ dB".into()
    } else if db.abs() < 0.05 {
        "0.0 dB".into()
    } else {
        format!("{db:+.1} dB")
    }
}

pub fn gain_to_db(g: f32) -> f32 {
    if g <= 1e-6 {
        -120.0
    } else {
        20.0 * g.log10()
    }
}

/// Lit une valeur en dB tapée au clavier : « -6 », « +3,5 », « -4.5 dB », « -inf ».
/// Sans signe, la valeur est négative (« 6 » donne -6 dB) : on baisse bien plus souvent
/// qu'on ne monte ; « +3 » pour monter. `-inf` donne le bas de la plage ; la valeur est
/// ramenée dans la plage.
pub fn parse_db(text: &str, range: &RangeInclusive<f32>) -> Option<f32> {
    let t = text.trim().to_lowercase().replace(',', ".").replace(' ', "");
    let t = t.strip_suffix("db").unwrap_or(&t);
    let v = match t {
        "-inf" | "-∞" | "inf" | "∞" => *range.start(),
        _ => {
            let v = t.parse::<f32>().ok().filter(|v| v.is_finite())?;
            if t.starts_with('+') || t.starts_with('-') { v } else { -v.abs() }
        }
    };
    Some(v.clamp(*range.start(), *range.end()))
}

/// Valeur en dB affichée en texte. Un clic ouvre un champ de saisie :
/// Entrée valide, Échap (ou un clic ailleurs) annule. Renvoie la valeur validée.
#[allow(clippy::too_many_arguments)]
pub fn db_value(
    ui: &mut Ui,
    id: Id,
    anchor: Pos2,
    align: Align2,
    db: f32,
    floor: f32,
    range: RangeInclusive<f32>,
    font: FontId,
    color: Color32,
) -> Option<f32> {
    db_value_shown(ui, id, anchor, align, db, floor, range, font, color, fmt_db(db, floor), None)
}

/// Comme `db_value`, avec un texte affiché au choix (« … », « ⚠ ») et une bulle d'aide en plus.
#[allow(clippy::too_many_arguments)]
pub fn db_value_shown(
    ui: &mut Ui,
    id: Id,
    anchor: Pos2,
    align: Align2,
    db: f32,
    floor: f32,
    range: RangeInclusive<f32>,
    font: FontId,
    color: Color32,
    text: String,
    tip: Option<String>,
) -> Option<f32> {
    let editing: Option<String> = ui.data(|d| d.get_temp(id));
    if let Some(mut buf) = editing {
        let w = ui.painter().layout_no_wrap("-60.0 dB".into(), font.clone(), color).size().x + 10.0;
        let rect = align.anchor_size(anchor, vec2(w, font.size + 6.0));
        let edit_id = id.with("edit");
        // `place` : le champ se pose sur la valeur sans décaler la mise en page.
        let resp = ui.place(
            rect,
            egui::TextEdit::singleline(&mut buf)
                .id(edit_id)
                .font(font)
                .align(Align2::CENTER_CENTER)
                .margin(vec2(2.0, 1.0)),
        );
        let first = !ui.data(|d| d.get_temp::<bool>(id.with("focused")).unwrap_or(false));
        if first {
            if let Some(mut st) = egui::text_edit::TextEditState::load(ui.ctx(), edit_id) {
                st.cursor.set_char_range(Some(egui::text::CCursorRange::select_all_str(&buf)));
                st.store(ui.ctx(), edit_id);
            }
            ui.data_mut(|d| d.insert_temp(id.with("focused"), true));
        }
        if !ui.memory(|m| m.has_focus(edit_id)) && !resp.lost_focus() {
            resp.request_focus();
        }
        // Entrée valide ; Échap, la perte du focus ou un clic hors du champ annulent.
        let (enter, escape, clicked_out) = ui.input(|i| {
            let out = i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !rect.contains(p));
            (i.key_pressed(Key::Enter), i.key_pressed(Key::Escape), out)
        });
        if enter || escape || clicked_out || (!first && resp.lost_focus()) {
            ui.data_mut(|d| {
                d.remove::<String>(id);
                d.remove::<bool>(id.with("focused"));
            });
            if resp.has_focus() || ui.memory(|m| m.has_focus(edit_id)) {
                ui.memory_mut(|m| m.surrender_focus(edit_id));
            }
            return if enter { parse_db(&buf, &range) } else { None };
        }
        ui.data_mut(|d| d.insert_temp(id, buf));
        return None;
    }
    let galley = ui.painter().layout_no_wrap(text.clone(), font.clone(), color);
    let rect = align.anchor_size(anchor, galley.size()).expand2(vec2(3.0, 1.0));
    let resp = ui.interact(rect, id, Sense::click());
    ui.painter().galley(rect.min + vec2(3.0, 1.0), galley, color);
    if resp.hovered() {
        ui.painter().rect_stroke(rect, CornerRadius::same(2), Stroke::new(1.0, Color32::from_white_alpha(40)), StrokeKind::Inside);
    }
    let hint = tr(
        "Clic pour saisir une valeur (sans signe = négatif : 6 donne -6 dB, +3 pour monter)",
        "Click to type a value (no sign = negative: 6 gives -6 dB, +3 to go up)",
    );
    let resp = resp.on_hover_text(match tip {
        Some(t) => format!("{t}\n\n{hint}"),
        None => hint.to_string(),
    });
    if resp.clicked() || resp.double_clicked() {
        let start = if db <= floor { "-inf".to_string() } else { format!("{db:.1}") };
        ui.data_mut(|d| d.insert_temp(id, start));
    }
    None
}

/// Repères de niveau des faders et curseurs de volume.
pub const DB_MARKS: [f32; 3] = [3.0, 0.0, -6.0];
/// Graduations sans valeur affichée, entre les repères chiffrés.
pub const DB_MINOR_MARKS: [f32; 10] = [10.0, 6.0, -3.0, -10.0, -15.0, -20.0, -30.0, -40.0, -50.0, -60.0];

/// Position d'une valeur le long d'un curseur egui de `rect` (même marge que egui : un rayon de poignée).
pub fn slider_value_pos(rect: Rect, vertical: bool, range: &RangeInclusive<f32>, v: f32) -> f32 {
    let t = ((v - range.start()) / (range.end() - range.start())).clamp(0.0, 1.0);
    if vertical {
        let r = rect.width() / 2.5;
        egui::lerp((rect.bottom() - r)..=(rect.top() + r), t)
    } else {
        let r = rect.height() / 2.5;
        egui::lerp((rect.left() + r)..=(rect.right() - r), t)
    }
}

/// Petits traits aux repères de niveau (0 dB plus marqué), le long d'un curseur.
pub fn db_marks(ui: &Ui, rect: Rect, vertical: bool, range: &RangeInclusive<f32>, labels: bool) {
    let p = ui.painter();
    // Graduations intermédiaires, sans valeur : traits courts et discrets.
    if vertical {
        let rail = ui.spacing().slider_rail_height / 2.0 + 2.0;
        let c = rect.center().x;
        for m in DB_MINOR_MARKS.into_iter().filter(|m| range.contains(m)) {
            let at = slider_value_pos(rect, vertical, range, m);
            p.line_segment([pos2(c + rail, at), pos2(c + rail + 4.0, at)], Stroke::new(1.0, Color32::from_white_alpha(75)));
        }
    }
    for m in DB_MARKS {
        if !range.contains(&m) {
            continue;
        }
        let zero = m == 0.0;
        let col = if zero { Color32::from_white_alpha(150) } else { Color32::from_white_alpha(80) };
        let len = if zero { 7.0 } else { 5.0 };
        let at = slider_value_pos(rect, vertical, range, m);
        let txt = if zero { "0".to_string() } else { format!("{m:+}") };
        if vertical {
            // Trait à droite du rail, valeur à gauche (le rail est au centre).
            let rail = ui.spacing().slider_rail_height / 2.0 + 2.0;
            let c = rect.center().x;
            p.line_segment([pos2(c + rail, at), pos2(c + rail + len, at)], Stroke::new(1.0, col));
            if !labels {
                p.line_segment([pos2(c - rail - len, at), pos2(c - rail, at)], Stroke::new(1.0, col));
            } else {
                // Valeurs lisibles : plus grandes et plus claires, 0 dB en blanc.
                let txt_col = if zero { Color32::from_white_alpha(235) } else { Color32::from_white_alpha(185) };
                p.text(pos2(c - rail, at), Align2::RIGHT_CENTER, txt, FontId::proportional(10.5), txt_col);
            }
        } else {
            p.line_segment([pos2(at, rect.bottom() - len), pos2(at, rect.bottom())], Stroke::new(1.0, col));
            if labels {
                p.text(pos2(at, rect.top()), Align2::CENTER_BOTTOM, txt, FontId::monospace(8.0), col);
            }
        }
    }
}

/// Vumètre stéréo vertical, échelle -60..0 dB.
pub fn meter(ui: &Ui, rect: Rect, db: [f32; 2]) {
    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(2), Color32::from_rgb(0x12, 0x12, 0x14));
    let w = (rect.width() - 3.0) / 2.0;
    let frac = |d: f32| ((d + 60.0) / 60.0).clamp(0.0, 1.0);
    for (c, d) in db.iter().enumerate() {
        let x0 = rect.left() + 1.0 + c as f32 * (w + 1.0);
        let bar = Rect::from_min_max(pos2(x0, rect.top() + 1.0), pos2(x0 + w, rect.bottom() - 1.0));
        let h = bar.height();
        let level = frac(*d);
        // Segments vert / jaune / rouge.
        for (lo, hi, col) in [(0.0, frac(-12.0), METER_LOW), (frac(-12.0), frac(-3.0), METER_MID), (frac(-3.0), 1.0, METER_HIGH)] {
            let top = level.min(hi);
            if top > lo {
                let r = Rect::from_min_max(
                    pos2(bar.left(), bar.bottom() - top * h),
                    pos2(bar.right(), bar.bottom() - lo * h),
                );
                p.rect_filled(r, CornerRadius::ZERO, col);
            }
        }
    }
    // Graduations.
    for d in [-6.0, -12.0, -24.0, -40.0] {
        let y = rect.bottom() - frac(d) * rect.height();
        p.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(1.0, Color32::from_black_alpha(140)));
    }
}

// ------------------------------------------------------- commandes de playlist

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportIcon {
    Prev,
    Play,
    Pause,
    Stop,
    Next,
}

/// Symbole de transport dessiné à la main, dans un carré de côté `s` centré sur `c`.
fn paint_transport_icon(p: &egui::Painter, c: Pos2, s: f32, icon: TransportIcon, col: Color32) {
    let h = s / 2.0;
    let tri = |pts: Vec<Pos2>| Shape::convex_polygon(pts, col, Stroke::NONE);
    match icon {
        TransportIcon::Play => {
            p.add(tri(vec![pos2(c.x - h * 0.7, c.y - h), pos2(c.x + h * 0.9, c.y), pos2(c.x - h * 0.7, c.y + h)]));
        }
        TransportIcon::Pause => {
            let bar = |x0: f32| Rect::from_min_max(pos2(x0, c.y - h * 0.85), pos2(x0 + h * 0.6, c.y + h * 0.85));
            p.rect_filled(bar(c.x - h * 0.75), CornerRadius::same(1), col);
            p.rect_filled(bar(c.x + h * 0.15), CornerRadius::same(1), col);
        }
        TransportIcon::Stop => {
            p.rect_filled(Rect::from_center_size(c, vec2(s * 0.8, s * 0.8)), CornerRadius::same(1), col);
        }
        TransportIcon::Prev => {
            p.rect_filled(Rect::from_min_max(pos2(c.x - h, c.y - h * 0.8), pos2(c.x - h * 0.65, c.y + h * 0.8)), CornerRadius::ZERO, col);
            p.add(tri(vec![pos2(c.x + h * 0.8, c.y - h * 0.8), pos2(c.x + h * 0.8, c.y + h * 0.8), pos2(c.x - h * 0.5, c.y)]));
        }
        TransportIcon::Next => {
            p.rect_filled(Rect::from_min_max(pos2(c.x + h * 0.65, c.y - h * 0.8), pos2(c.x + h, c.y + h * 0.8)), CornerRadius::ZERO, col);
            p.add(tri(vec![pos2(c.x - h * 0.8, c.y - h * 0.8), pos2(c.x + h * 0.5, c.y), pos2(c.x - h * 0.8, c.y + h * 0.8)]));
        }
    }
}

/// Bouton de la barre de transport : fond plein, symbole et texte facultatif.
pub fn transport_button(ui: &mut Ui, rect: Rect, id: impl std::hash::Hash + std::fmt::Debug, icon: TransportIcon, label: Option<&str>, fill: Color32, fg: Color32) -> Response {
    let resp = ui.interact(rect, ui.make_persistent_id(id), Sense::click());
    let p = ui.painter_at(rect.expand(1.0));
    let r = CornerRadius::same(4);
    let fill = if resp.is_pointer_button_down_on() {
        fill.gamma_multiply(0.8)
    } else if resp.hovered() {
        fill.gamma_multiply(1.2)
    } else {
        fill
    };
    p.rect_filled(rect, r, fill);
    p.rect_stroke(rect, r, Stroke::new(1.0, Color32::from_black_alpha(110)), StrokeKind::Inside);
    let s = 14.0;
    match label {
        Some(text) => {
            let galley = p.layout_no_wrap(text.to_owned(), FontId::proportional(14.0), fg);
            let total = s + 7.0 + galley.size().x;
            let x0 = rect.center().x - total / 2.0;
            paint_transport_icon(&p, pos2(x0 + s / 2.0, rect.center().y), s, icon, fg);
            p.galley(pos2(x0 + s + 7.0, rect.center().y - galley.size().y / 2.0), galley, fg);
        }
        None => paint_transport_icon(&p, rect.center(), s, icon, fg),
    }
    resp
}

/// Interrupteur de mode avec voyant : ambre et lumineux quand il est actif.
pub fn mode_chip(ui: &mut Ui, rect: Rect, id: impl std::hash::Hash + std::fmt::Debug, on: bool, label: &str) -> Response {
    let resp = ui.interact(rect, ui.make_persistent_id(id), Sense::click());
    let p = ui.painter_at(rect.expand(1.0));
    let r = CornerRadius::same(4);
    let (fill, stroke, text) = if on {
        (ACCENT.gamma_multiply(0.14), ACCENT.gamma_multiply(0.6), TEXT)
    } else {
        (PANEL_HI, BORDER, TEXT_DIM)
    };
    p.rect_filled(rect, r, if resp.hovered() { fill.gamma_multiply(1.3) } else { fill });
    p.rect_stroke(rect, r, Stroke::new(1.0, stroke), StrokeKind::Inside);
    let galley = p.layout_no_wrap(label.to_owned(), FontId::proportional(13.0), text);
    let total = 7.0 + 6.0 + galley.size().x;
    let x0 = rect.center().x - total / 2.0;
    let led = pos2(x0 + 3.5, rect.center().y);
    if on {
        p.circle_filled(led, 6.0, ACCENT.gamma_multiply(0.18));
        p.circle_filled(led, 3.5, ACCENT);
    } else {
        p.circle_filled(led, 3.5, Color32::from_rgb(0x4a, 0x4a, 0x54));
    }
    p.galley(pos2(x0 + 13.0, rect.center().y - galley.size().y / 2.0), galley, text);
    resp
}

/// Contrôle segmenté : une rangée d'options exclusives. Renvoie l'option cliquée.
pub fn segmented(ui: &mut Ui, rect: Rect, id: impl std::hash::Hash + std::fmt::Debug + Copy, labels: &[&str], selected: usize) -> Option<usize> {
    let n = labels.len() as f32;
    let w = rect.width() / n;
    let p = ui.painter_at(rect.expand(1.0));
    let mut clicked = None;
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + i as f32 * w, rect.top()), vec2(w, rect.height()));
        let resp = ui.interact(r, ui.make_persistent_id((id, i)), Sense::click());
        let corner = CornerRadius {
            nw: if i == 0 { 4 } else { 0 },
            sw: if i == 0 { 4 } else { 0 },
            ne: if i + 1 == labels.len() { 4 } else { 0 },
            se: if i + 1 == labels.len() { 4 } else { 0 },
        };
        let on = i == selected;
        let fill = if on { ACCENT.gamma_multiply(0.55) } else { PANEL_HI };
        p.rect_filled(r, corner, if resp.hovered() && !on { fill.gamma_multiply(1.3) } else { fill });
        if i > 0 {
            p.line_segment([r.left_top(), r.left_bottom()], Stroke::new(1.0, BORDER));
        }
        let col = if on { TEXT } else { TEXT_DIM };
        p.text(r.center(), Align2::CENTER_CENTER, *label, FontId::proportional(12.5), col);
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    p.rect_stroke(rect, CornerRadius::same(4), Stroke::new(1.0, BORDER), StrokeKind::Inside);
    clicked
}

/// Vrai à l'image où le bouton gauche s'enfonce sur ce widget : GO, arrêt et jingles partent
/// Symbole de boucle dessiné (deux flèches qui se suivent sur un anneau arrondi),
/// plus net à petite taille qu'un emoji.
pub fn loop_icon(p: &egui::Painter, r: Rect, color: Color32) {
    let stroke = Stroke::new(1.6, color);
    let rad = r.height() / 2.0;
    let (cl, cr) = (pos2(r.left() + rad, r.center().y), pos2(r.right() - rad, r.center().y));
    // Demi-cercles aux extrémités, puis les deux côtés droits interrompus par les flèches.
    let arc = |c: Pos2, from: f32| -> Vec<Pos2> {
        (0..=8).map(|i| {
            let a = from + std::f32::consts::PI * i as f32 / 8.0;
            c + vec2(a.cos(), a.sin()) * rad
        }).collect()
    };
    p.add(egui::Shape::line(arc(cr, -std::f32::consts::FRAC_PI_2), stroke));
    p.add(egui::Shape::line(arc(cl, std::f32::consts::FRAC_PI_2), stroke));
    let (top, bottom) = (r.top(), r.bottom());
    p.line_segment([pos2(cl.x, top), pos2(cr.x - 1.0, top)], stroke);
    p.line_segment([pos2(cr.x, bottom), pos2(cl.x + 1.0, bottom)], stroke);
    let head = 3.0;
    // Flèche vers la droite en haut, vers la gauche en bas.
    let tip = pos2(r.center().x + 2.0, top);
    p.add(egui::Shape::convex_polygon(vec![tip + vec2(head, 0.0), tip + vec2(-head * 0.6, -head), tip + vec2(-head * 0.6, head)], color, Stroke::NONE));
    let tip = pos2(r.center().x - 2.0, bottom);
    p.add(egui::Shape::convex_polygon(vec![tip + vec2(-head, 0.0), tip + vec2(head * 0.6, head), tip + vec2(head * 0.6, -head)], color, Stroke::NONE));
}

/// Avancement d'un cue qui passe, derrière le nom dans sa ligne de conduite : fond teinté
/// jusqu'à la position. `frac` : de 0 (début) à 1 (fin).
pub fn cue_progress_back(p: &egui::Painter, row: Rect, frac: f32) {
    let frac = frac.clamp(0.0, 1.0);
    p.rect_filled(Rect::from_min_size(row.min, vec2(row.width() * frac, row.height())), CornerRadius::same(3), GO.gamma_multiply(0.28));
}

/// Avancement d'un cue qui passe, par-dessus la ligne (à dessiner après le nom et son champ de
/// saisie, qui ne le cachent donc jamais) : barre pleine en bas de la ligne et tête de lecture.
pub fn cue_progress_front(p: &egui::Painter, row: Rect, frac: f32) {
    let frac = frac.clamp(0.0, 1.0);
    let x = row.left() + row.width() * frac;
    let bar = Rect::from_min_max(pos2(row.left(), row.bottom() - 3.0), row.max);
    p.rect_filled(bar, CornerRadius::same(1), GO.gamma_multiply(0.25));
    p.rect_filled(Rect::from_min_max(bar.min, pos2(x, bar.bottom())), CornerRadius::same(1), GO);
    p.rect_filled(Rect::from_min_max(pos2(x - 1.0, row.top()), pos2(x + 1.0, row.bottom())), CornerRadius::ZERO, GO);
}

/// Bouton « Ajouter » : un « + » orange épais dessiné à gauche du libellé, cadre orange.
pub fn add_button(ui: &mut Ui, label: &str, enabled: bool) -> Response {
    let col = if enabled { ACCENT } else { TEXT_DIM };
    // Le « + » est un élément du bouton (et non des espaces devant le texte) : toujours aligné.
    let plus = crate::icons::atom_with(crate::icons::Icon::Plus, 12.0, Some(col));
    let text = egui::RichText::new(label).size(14.0);
    ui.add_enabled(enabled, egui::Button::new((plus, text)).stroke(Stroke::new(1.0, col.gamma_multiply(0.6))))
}

/// Vrai à l'image où le bouton gauche s'enfonce sur ce widget : GO, arrêt et jingles partent
/// à l'appui, sans attendre le relâché (un clic dure couramment 80 à 150 ms).

pub fn pressed(ui: &Ui, resp: &Response) -> bool {
    resp.hovered() && ui.input(|i| i.pointer.primary_pressed())
}

/// Ligne de la liste des morceaux : numéro et nom à gauche (tronqué), place réservée au gain à droite
/// (dessiné ensuite par l'appelant, à la position renvoyée).
pub fn track_row(ui: &mut Ui, label: &str, color: Color32, selected: bool, gain: Option<&str>) -> (Response, Pos2) {
    track_row_styled(ui, label, color, selected.then(|| ACCENT.gamma_multiply(0.18)), None, gain)
}

/// Ligne de playlist avec un fond (`fill`) et un cadre (`outline`) au choix, pour distinguer
/// l'élément en cours de l'élément sélectionné.
pub fn track_row_styled(ui: &mut Ui, label: &str, color: Color32, fill: Option<Color32>, outline: Option<Color32>, gain: Option<&str>) -> (Response, Pos2) {
    // Glissable : un morceau peut être déposé sur un bouton de jingle.
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.max_rect().width().min(ui.available_width()), 20.0), Sense::click_and_drag());
    let p = ui.painter_at(rect);
    if let Some(f) = fill {
        p.rect_filled(rect, CornerRadius::same(3), f);
    } else if resp.hovered() {
        p.rect_filled(rect, CornerRadius::same(3), PANEL_HI);
    }
    if let Some(c) = outline {
        p.rect_stroke(rect.shrink(0.5), CornerRadius::same(3), egui::Stroke::new(1.0, c), egui::StrokeKind::Inside);
    }
    let font = FontId::monospace(12.0);
    let anchor = pos2(rect.right() - 6.0, rect.center().y);
    let mut right = anchor.x;
    if let Some(txt) = gain {
        right -= p.layout_no_wrap(txt.to_owned(), FontId::monospace(11.0), TEXT).size().x + 8.0;
    }
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width((right - rect.left() - 6.0).max(10.0));
    let g = p.layout_job(job);
    p.galley(pos2(rect.left() + 6.0, rect.center().y - g.size().y / 2.0), g, color);
    (resp, anchor)
}

/// Pastille d'état : un rond de couleur et un texte.
pub fn status_dot(ui: &mut egui::Ui, color: Color32, text: impl Into<String>) -> egui::Response {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 5.0, color);
        ui.label(egui::RichText::new(text.into()).color(TEXT));
    })
    .response
}

/// Marge horizontale de la forme d'onde, pour saisir les poignées aux extrémités.
const HANDLE_PAD: f32 = 14.0;

/// Points d'édition d'un jingle, en secondes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Markers {
    pub start: f64,
    pub loop_start: f64,
    pub loop_end: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Handle {
    Start,
    LoopStart,
    LoopEnd,
    End,
    FadeIn,
    FadeOut,
}

/// Éditeur façon QLab : poignées de début et de fin, traits de début et de fin de boucle,
/// rampes de fondu avec leurs poignées en haut. Renvoie `true` si un point ou un fondu a été déplacé.
/// Le fondu de sortie n'a de poignée qu'en lecture unique (en boucle, il ne sert qu'à l'arrêt).
#[allow(clippy::too_many_arguments)]
pub fn waveform_editor(
    ui: &mut Ui,
    background: impl FnOnce(&WaveView),
    duration: f64,
    m: &mut Markers,
    looping: bool,
    fade_in: &mut f64,
    fade_out: &mut f64,
    playhead: Option<f64>,
    seek: &mut Option<f64>,
) -> bool {
    let size = vec2(ui.available_width(), 150.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let wave = rect.shrink2(vec2(HANDLE_PAD, 14.0));
    let p = ui.painter_at(rect);
    p.rect_filled(rect, CornerRadius::same(4), Color32::from_rgb(0x12, 0x12, 0x14));
    if duration <= 0.0 {
        return false;
    }

    // Zoom : partie visible (v0..v1 s), gardée d'une image à l'autre pour ce son.
    let view_id = resp.id.with("view");
    let (mut v0, mut v1) = ui
        .data(|d| d.get_temp::<(f64, f64, f64)>(view_id))
        .filter(|v| v.2 == duration)
        .map(|v| (v.0, v.1))
        .unwrap_or((0.0, duration));
    let min_span = (duration / 2000.0).max(0.002).min(duration);
    if let Some(hover) = resp.hover_pos() {
        // Molette (ou Ctrl + molette, pincement) : zoom autour du pointeur. Défilement horizontal : déplacement.
        let (scroll, zoom) = ui.input_mut(|i| {
            let d = (i.smooth_scroll_delta, i.zoom_delta());
            i.smooth_scroll_delta = egui::Vec2::ZERO;
            d
        });
        let span = v1 - v0;
        let factor = (-(scroll.y as f64) * 0.004).exp() / zoom as f64;
        let new_span = (span * factor).clamp(min_span, duration);
        let anchor = v0 + ((hover.x - wave.left()) / wave.width()).clamp(0.0, 1.0) as f64 * span;
        v0 = anchor - (anchor - v0) * new_span / span - scroll.x as f64 / wave.width() as f64 * new_span;
        v1 = v0 + new_span;
    }
    let clamp_view = |v0: f64, v1: f64| {
        let span = (v1 - v0).clamp(min_span, duration);
        let v0 = v0.clamp(0.0, duration - span);
        (v0, v0 + span)
    };
    (v0, v1) = clamp_view(v0, v1);
    let zoomed = v1 - v0 < duration * 0.999;
    let x_of = |t: f64| wave.left() + ((t - v0) / (v1 - v0)) as f32 * wave.width();
    let t_of = |x: f32| (v0 + ((x - wave.left()) / wave.width()) as f64 * (v1 - v0)).clamp(0.0, duration);
    let clip_x = |x: f32| x.clamp(rect.left(), rect.right());

    // Zone de boucle.
    if looping {
        let lr = Rect::from_x_y_ranges(clip_x(x_of(m.loop_start))..=clip_x(x_of(m.loop_end)), wave.y_range());
        p.rect_filled(lr, CornerRadius::ZERO, ACCENT.gamma_multiply(0.12));
    }

    // Fond : la forme d'onde (ou des vignettes), dessinée par l'appelant sur v0..v1.
    // Les fondus s'y voient : l'amplitude jouée est dessinée avec le gain du fondu
    // (linéaire, comme le mixeur), le son d'origine reste en filigrane derrière.
    let fade_gain = |t: f64| {
        let mut g = 1.0;
        if *fade_in > 0.0 && t < m.start + *fade_in {
            g *= ((t - m.start) / *fade_in).clamp(0.0, 1.0);
        }
        if !looping && *fade_out > 0.0 && t > m.end - *fade_out {
            g *= ((m.end - t) / *fade_out).clamp(0.0, 1.0);
        }
        g as f32
    };
    background(&WaveView { painter: &p, rect: wave, v0, v1, duration, markers: *m, fade_gain: &fade_gain });

    // Parties exclues assombries.
    let shade = Color32::from_black_alpha(140);
    p.rect_filled(Rect::from_x_y_ranges(rect.left()..=clip_x(x_of(m.start)), rect.y_range()), CornerRadius::ZERO, shade);
    p.rect_filled(Rect::from_x_y_ranges(clip_x(x_of(m.end))..=rect.right(), rect.y_range()), CornerRadius::ZERO, shade);

    // Rampes de fondu (linéaires, comme dans le mixeur) : partie atténuée voilée, poignée carrée en haut.
    let span = (m.end - m.start).max(0.0);
    *fade_in = fade_in.clamp(0.0, span);
    *fade_out = fade_out.clamp(0.0, span);
    let fi_x = x_of(m.start + *fade_in);
    let fo_x = x_of(m.end - *fade_out);
    let ramp = Stroke::new(1.5, Color32::from_rgb(0xe8, 0xe8, 0xe8));
    let veil = Color32::from_black_alpha(110);
    if *fade_in > 0.0 {
        let (a, b) = (pos2(x_of(m.start), wave.bottom()), pos2(fi_x, wave.top()));
        p.add(Shape::convex_polygon(vec![pos2(a.x, wave.top()), b, a], veil, Stroke::NONE));
        p.line_segment([a, b], ramp);
    }
    if *fade_out > 0.0 && !looping {
        let (a, b) = (pos2(fo_x, wave.top()), pos2(x_of(m.end), wave.bottom()));
        p.add(Shape::convex_polygon(vec![a, pos2(b.x, wave.top()), b], veil, Stroke::NONE));
        p.line_segment([a, b], ramp);
    }
    let mut fade_handles = vec![(Handle::FadeIn, fi_x, *fade_in)];
    if !looping {
        fade_handles.push((Handle::FadeOut, fo_x, *fade_out));
    }
    for &(_, x, secs) in &fade_handles {
        let sq = Rect::from_center_size(pos2(x, rect.top() + 7.0), vec2(9.0, 9.0));
        p.rect_filled(sq, CornerRadius::same(1), Color32::from_rgb(0xe8, 0xe8, 0xe8));
        p.rect_stroke(sq, CornerRadius::same(1), Stroke::new(1.0, Color32::from_black_alpha(160)), StrokeKind::Inside);
        if secs > 0.0 {
            p.text(pos2(x, wave.top() + 3.0), Align2::CENTER_TOP, format!("{secs:.2} s"), FontId::monospace(9.0), Color32::from_rgb(0xe8, 0xe8, 0xe8));
        }
    }

    // Traits de boucle.
    if looping {
        // Boucle étroite à l'écran : les étiquettes passent à l'extérieur pour ne pas se chevaucher.
        let narrow = x_of(m.loop_end) - x_of(m.loop_start) < 110.0;
        // « ⟲ début » et « fin ⟳ » : flèches dessinées, centrées sur la ligne du texte.
        for (t, label, start) in [(m.loop_start, tr("début", "start"), true), (m.loop_end, tr("fin", "end"), false)] {
            let x = x_of(t);
            p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(2.0, ACCENT));
            let inside_left = start != narrow;
            let g = p.layout_no_wrap(label.to_owned(), FontId::proportional(10.0), ACCENT);
            let (s, gap) = (9.0, 2.0);
            let total = s + gap + g.size().x;
            let left = if inside_left { x + 3.0 } else { x - 3.0 - total };
            let top = rect.top() + 1.0;
            let (icon_x, text_x) = if start { (left, left + s + gap) } else { (left + g.size().x + gap, left) };
            let icon = if start { crate::icons::Icon::RefreshBack } else { crate::icons::Icon::Refresh };
            crate::icons::paint(&p, Rect::from_min_size(pos2(icon_x, top + (g.size().y - s) / 2.0), vec2(s, s)), icon, ACCENT);
            p.galley(pos2(text_x, top), g, ACCENT);
        }
    }

    // Poignées de début (verte) et de fin (rouge).
    for (t, col, left) in [(m.start, GO, true), (m.end, STOP, false)] {
        let x = x_of(t);
        p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.5, col));
        let dir = if left { 1.0 } else { -1.0 };
        let tri = vec![pos2(x, rect.bottom()), pos2(x + 10.0 * dir, rect.bottom()), pos2(x, rect.bottom() - 12.0)];
        p.add(Shape::convex_polygon(tri, col, Stroke::NONE));
    }

    if let Some(t) = playhead {
        let x = x_of(t);
        p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], Stroke::new(1.0, Color32::WHITE));
    }

    // Glisser les poignées. Les points de boucle se saisissent dans la moitié haute,
    // le début et la fin dans la moitié basse : ils restent séparables même superposés.
    // Les poignées de fondu se saisissent dans la bande au-dessus de la forme d'onde.
    let nearest = |pos: Pos2| {
        if pos.y < wave.top() {
            let fade = fade_handles
                .iter()
                .map(|&(h, x, _)| (h, (x - pos.x).abs()))
                .filter(|(_, d)| *d < 10.0)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(h, _)| h);
            if fade.is_some() {
                return fade;
            }
        }
        let top = pos.y < rect.center().y;
        let mut handles = vec![(Handle::Start, m.start), (Handle::End, m.end)];
        if looping {
            let lp = [(Handle::LoopStart, m.loop_start), (Handle::LoopEnd, m.loop_end)];
            if top {
                handles.splice(0..0, lp);
            } else {
                handles.extend(lp);
            }
        }
        handles
            .into_iter()
            .map(|(h, t)| (h, (x_of(t) - pos.x).abs()))
            .filter(|(_, d)| *d < 14.0)
            // À distance égale, le premier de la liste (priorité selon la zone) l'emporte.
            .fold(None::<(Handle, f32)>, |best, (h, d)| match best {
                Some((_, bd)) if bd <= d => best,
                _ => Some((h, d)),
            })
            .map(|(h, _)| h)
    };
    let drag_id = resp.id.with("handle");
    if let Some(pos) = resp.hover_pos() {
        if nearest(pos).is_some() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
    }
    if resp.drag_started() {
        // On choisit la poignée là où le bouton a été enfoncé, pas après le seuil de glisser.
        let origin = ui.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos());
        let h = origin.and_then(nearest);
        ui.data_mut(|d| d.insert_temp(drag_id, h));
    }
    let dbl_empty = resp.double_clicked() && resp.interact_pointer_pos().and_then(nearest).is_none();
    // Pendant la lecture, un clic hors des poignées déplace la lecture à cet endroit.
    if playhead.is_some() && resp.clicked() {
        if let Some(pos) = resp.interact_pointer_pos().filter(|p| nearest(*p).is_none()) {
            *seek = Some(t_of(pos.x));
        }
    }
    let mut changed = false;
    let mut view = (v0, v1);
    if resp.dragged() {
        let h: Option<Handle> = ui.data(|d| d.get_temp(drag_id)).flatten();
        if h.is_none() && zoomed {
            // Glisser hors des poignées : déplace la partie visible.
            let dt = -(resp.drag_delta().x / wave.width()) as f64 * (v1 - v0);
            view = clamp_view(v0 + dt, v1 + dt);
        }
        if let (Some(h), Some(pos)) = (h, resp.interact_pointer_pos()) {
            let t = t_of(pos.x);
            let before = (*m, *fade_in, *fade_out);
            match h {
                Handle::FadeIn => *fade_in = (t - m.start).clamp(0.0, m.end - m.start),
                Handle::FadeOut => *fade_out = (m.end - t).clamp(0.0, m.end - m.start),
                Handle::Start => {
                    m.start = t.min(m.end);
                    m.loop_start = m.loop_start.max(m.start);
                    m.loop_end = m.loop_end.max(m.loop_start);
                }
                Handle::End => {
                    m.end = t.max(m.start);
                    m.loop_end = m.loop_end.min(m.end);
                    m.loop_start = m.loop_start.min(m.loop_end);
                }
                Handle::LoopStart => m.loop_start = t.clamp(m.start, m.loop_end),
                Handle::LoopEnd => m.loop_end = t.clamp(m.loop_start, m.end),
            }
            changed = (*m, *fade_in, *fade_out) != before;
        }
    }

    // Zoom : bouton pour tout afficher (ou double-clic) et repère de la partie visible.
    if zoomed {
        let btn = Rect::from_min_size(rect.right_top() + vec2(-86.0, 16.0), vec2(80.0, 18.0));
        if ui.place(btn, egui::Button::new(egui::RichText::new(tr("Tout afficher", "Show all")).small())).clicked() {
            view = (0.0, duration);
        }
    }
    if dbl_empty {
        view = (0.0, duration);
    }
    // Barre de défilement sous la forme d'onde : la partie visible se glisse le long du son,
    // un clic à côté y amène la vue.
    let (sb, sb_resp) = ui.allocate_exact_size(vec2(rect.width(), 10.0), Sense::click_and_drag());
    let span = view.1 - view.0;
    let thumb = Rect::from_x_y_ranges(
        sb.left() + (view.0 / duration) as f32 * sb.width()..=sb.left() + (view.1 / duration) as f32 * sb.width(),
        sb.y_range(),
    );
    let sp = ui.painter();
    sp.rect_filled(sb, CornerRadius::same(3), Color32::from_white_alpha(14));
    let hot = sb_resp.hovered() || sb_resp.dragged();
    sp.rect_filled(thumb, CornerRadius::same(3), Color32::from_white_alpha(if !zoomed { 40 } else if hot { 170 } else { 110 }));
    if zoomed {
        if sb_resp.dragged() {
            let dt = (sb_resp.drag_delta().x / sb.width()) as f64 * duration;
            view = clamp_view(view.0 + dt, view.1 + dt);
        } else if let Some(p) = sb_resp.interact_pointer_pos().filter(|_| sb_resp.clicked()) {
            let c = ((p.x - sb.left()) / sb.width()) as f64 * duration;
            view = clamp_view(c - span / 2.0, c + span / 2.0);
        }
    }
    ui.data_mut(|d| d.insert_temp(view_id, (view.0, view.1, duration)));
    resp.on_hover_text(tr(
        "Glisser les poignées : début et fin en bas (vert, rouge), boucle en haut (orange), fondus sur les carrés blancs.\nMolette : zoom, glisser ailleurs : se déplacer, double-clic : tout afficher.",
        "Drag the handles: start and end at the bottom (green, red), loop at the top (orange), fades on the white squares.\nWheel: zoom, drag elsewhere: scroll, double-click: show all.",
    ));
    changed
}

/// Ce que voit le fond de l'éditeur de forme d'onde : la zone de dessin, la partie visible du
/// son (`v0..v1` secondes), les points d'édition et le gain des fondus à un instant donné.
pub struct WaveView<'a> {
    pub painter: &'a egui::Painter,
    pub rect: Rect,
    pub v0: f64,
    pub v1: f64,
    pub duration: f64,
    pub markers: Markers,
    pub fade_gain: &'a dyn Fn(f64) -> f32,
}

impl WaveView<'_> {
    /// Abscisse de l'instant `t`.
    pub fn x_of(&self, t: f64) -> f32 {
        self.rect.left() + ((t - self.v0) / (self.v1 - self.v0)) as f32 * self.rect.width()
    }
    /// Instant sous l'abscisse `x`.
    pub fn t_of(&self, x: f32) -> f64 {
        (self.v0 + ((x - self.rect.left()) / self.rect.width()) as f64 * (self.v1 - self.v0)).clamp(0.0, self.duration)
    }
}

/// Fond de l'éditeur dessiné à partir des crêtes du son : `peaks(t0, t1, n)` renvoie `n` crêtes
/// (0 à 1) réparties sur `t0..t1`. La partie jouée est en bleu clair, avec les fondus appliqués.
pub fn peaks_background(peaks: impl Fn(f64, f64, usize) -> Vec<f32>) -> impl FnOnce(&WaveView) {
    move |w: &WaveView| {
        let (p, wave, m) = (w.painter, w.rect, w.markers);
        let mid = wave.center().y;
        let cols = wave.width().max(1.0) as usize;
        for (c, peak) in peaks(w.v0, w.v1, cols).into_iter().enumerate() {
            let x = wave.left() + c as f32 + 0.5;
            let t = w.t_of(x);
            let inside = t >= m.start && t <= m.end;
            let col = if inside { Color32::from_rgb(0x7f, 0xb8, 0xe8) } else { Color32::from_rgb(0x3a, 0x48, 0x55) };
            let h = peak.min(1.0) * wave.height() / 2.0;
            if inside {
                let g = (w.fade_gain)(t);
                if g < 1.0 {
                    p.line_segment([pos2(x, mid - h), pos2(x, mid + h)], Stroke::new(1.0, col.gamma_multiply(0.22)));
                }
                let h = h * g;
                p.line_segment([pos2(x, mid - h), pos2(x, mid + h)], Stroke::new(1.0, col));
            } else {
                p.line_segment([pos2(x, mid - h), pos2(x, mid + h)], Stroke::new(1.0, col));
            }
        }
    }
}

/// Logo de JingleStar dans la barre du haut : la petite variante de l'icône
/// (`jinglestar-icone-petite.svg`), grille de pads avec un casque sur le pad central.
pub fn logo_mark(ui: &mut Ui, height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(height, height), Sense::hover());
    logo_small(ui.painter(), rect, Symbol::Headphones);
    resp
}

/// Logo de CueStar dans la barre du haut : la petite variante de l'icône
/// (`cuestar-icone-petite.svg`), grille de pads avec un triangle « lecture » sur le pad central.
pub fn logo_mark_play(ui: &mut Ui, height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(height, height), Sense::hover());
    logo_small(ui.painter(), rect, Symbol::Play);
    resp
}

/// Logo de SweepStar dans la barre du haut : la petite variante de l'icône
/// (`sweepstar-icone-petite.svg`), grille de pads avec une réponse impulsionnelle sur le pad central.
pub fn logo_mark_impulse(ui: &mut Ui, height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(height, height), Sense::hover());
    logo_small(ui.painter(), rect, Symbol::Impulse);
    resp
}

/// Logo d'OldStar dans la barre du haut : la petite variante de l'icône
/// (`oldstar-icone-petite.svg`), grille de pads avec un fader et un signal sans fil sur le pad central.
pub fn logo_mark_fader(ui: &mut Ui, height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(height, height), Sense::hover());
    logo_small(ui.painter(), rect, Symbol::Fader);
    resp
}

/// Logo de MediaStar dans la barre du haut : la petite variante de l'icône
/// (`mediastar-icone-petite.svg`), quatre tuiles aux symboles des modules et l'étoile au centre.
pub fn logo_mark_media(ui: &mut Ui, height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(height, height), Sense::hover());
    logo_media_small(ui.painter(), rect);
    resp
}

/// Symbole du pad central des logos.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Symbol {
    Headphones,
    Play,
    Impulse,
    Fader,
}

/// Réponse impulsionnelle du logo de SweepStar (repère de 100 unités, un point sur trois du SVG).
const IMPULSE: &[(f32, f32)] = &[(8.0, 6.4e+01), (2e+01, 6.4e+01), (2.6e+01, 1e+01), (3.2e+01, 8.6e+01), (3.2e+01, 4.3e+01), (3.4e+01, 4.6e+01), (3.5e+01, 5.2e+01), (3.6e+01, 5.9e+01), (3.7e+01, 6.5e+01), (3.8e+01, 7.1e+01), (3.9e+01, 7.5e+01), (4e+01, 7.7e+01), (4.1e+01, 7.6e+01), (4.3e+01, 7.5e+01), (4.4e+01, 7.2e+01), (4.5e+01, 6.8e+01), (4.6e+01, 6.4e+01), (4.7e+01, 6.1e+01), (4.8e+01, 5.8e+01), (4.9e+01, 5.7e+01), (5e+01, 5.7e+01), (5.2e+01, 5.8e+01), (5.3e+01, 5.9e+01), (5.4e+01, 6.1e+01), (5.5e+01, 6.3e+01), (5.6e+01, 6.5e+01), (5.7e+01, 6.7e+01), (5.8e+01, 6.8e+01), (6e+01, 6.8e+01), (6.1e+01, 6.8e+01), (6.2e+01, 6.7e+01), (6.3e+01, 6.6e+01), (6.4e+01, 6.5e+01), (6.5e+01, 6.4e+01), (6.6e+01, 6.3e+01), (6.7e+01, 6.2e+01), (6.9e+01, 6.2e+01), (7e+01, 6.2e+01), (7.1e+01, 6.2e+01), (7.2e+01, 6.3e+01), (7.3e+01, 6.3e+01), (7.4e+01, 6.4e+01), (7.5e+01, 6.5e+01), (7.7e+01, 6.5e+01), (7.8e+01, 6.5e+01), (7.9e+01, 6.5e+01), (8e+01, 6.5e+01), (8.1e+01, 6.5e+01), (8.2e+01, 6.4e+01), (8.3e+01, 6.4e+01), (8.4e+01, 6.4e+01), (8.6e+01, 6.3e+01), (8.7e+01, 6.3e+01), (8.8e+01, 6.3e+01), (8.9e+01, 6.3e+01), (9e+01, 6.3e+01), (9.1e+01, 6.4e+01), (9.2e+01, 6.4e+01)];

/// Reproduit `*-icone-petite.svg` : l'icône de l'appli (carré de 1024 unités) dont la grille
/// et le pad central sont agrandis de 28 % autour du centre, pour rester lisibles en petit.
fn logo_small(p: &egui::Painter, rect: Rect, symbol: Symbol) {
    const FOND: Color32 = Color32::from_rgb(0x22, 0x22, 0x28);
    const CASE: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x33);
    const LISERE: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x23);
    const PAD: Color32 = Color32::from_rgb(0xed, 0xa8, 0x3a);
    const ENCRE: Color32 = Color32::from_rgb(0x20, 0x20, 0x26);
    const BANDEAUX: [Color32; 8] = [
        Color32::from_rgb(0x4a, 0x8f, 0xdb),
        Color32::from_rgb(0x4f, 0xae, 0x58),
        Color32::from_rgb(0xe2, 0xb2, 0x32),
        Color32::from_rgb(0xe2, 0x7a, 0x30),
        Color32::from_rgb(0x9b, 0x6b, 0xd3),
        Color32::from_rgb(0x3f, 0xb8, 0xaf),
        Color32::from_rgb(0xd9, 0x53, 0x4f),
        Color32::from_rgb(0x8a, 0x8a, 0x90),
    ];
    let u = rect.height() / 1024.0;
    let r = |v: f32| (v * u).round().clamp(0.0, 255.0) as u8;
    // Coordonnées du SVG, agrandies de 28 % autour du centre (512, 512).
    let k = 1.28;
    let at = |x: f32, y: f32| rect.min + vec2((512.0 + (x - 512.0) * k) * u, (512.0 + (y - 512.0) * k) * u);
    let carre = |x: f32, y: f32, w: f32| Rect::from_min_max(at(x, y), at(x + w, y + w));

    p.rect_filled(rect, CornerRadius::same(r(230.0)), FOND);
    let mut b = 0;
    for row in 0..3 {
        for col in 0..3 {
            if row == 1 && col == 1 {
                continue;
            }
            let (x, y) = (180.8 + col as f32 * 230.4, 180.8 + row as f32 * 230.4);
            let rad = r(32.4 * k);
            p.rect_filled(carre(x, y, 201.6), CornerRadius::same(rad), CASE);
            let bandeau = Rect::from_min_max(at(x, y), at(x + 201.6, y + 36.0));
            p.rect_filled(bandeau, CornerRadius { nw: rad, ne: rad, sw: 0, se: 0 }, BANDEAUX[b]);
            b += 1;
        }
    }
    p.rect_filled(carre(315.0, 315.0, 394.0), CornerRadius::same(r(79.5 * k)), LISERE);
    p.rect_filled(carre(337.0, 337.0, 350.0), CornerRadius::same(r(59.5 * k)), PAD);

    // Symbole : repère de 100 unités placé en (372, 372) à l'échelle 2,8.
    draw_symbol(p, symbol, &|x, y| at(372.0 + x * 2.8, 372.0 + y * 2.8), 2.8 * k * u, ENCRE);
}

/// Dessine le symbole d'un module dans son repère de 100 unités : `sym` place un point du
/// repère, `s` est la taille d'une unité en points.
fn draw_symbol(p: &egui::Painter, symbol: Symbol, sym: &dyn Fn(f32, f32) -> Pos2, s: f32, ink: Color32) {
    let r = |v: f32| v.round().clamp(0.0, 255.0) as u8;
    if symbol == Symbol::Fader {
        // Fader : glissière de 8 unités, bouton de 26 × 16 ; signal : point et trois arcs.
        p.line_segment([sym(20.0, 12.0), sym(20.0, 88.0)], Stroke::new(8.0 * s, ink));
        for y in [12.0, 88.0] {
            p.circle_filled(sym(20.0, y), 4.0 * s, ink);
        }
        p.rect_filled(Rect::from_min_max(sym(7.0, 32.0), sym(33.0, 48.0)), CornerRadius::same(r(3.5 * s)), ink);
        p.circle_filled(sym(70.0, 80.0), 6.0 * s, ink);
        for radius in [15.0f32, 27.0, 39.0] {
            let pts: Vec<Pos2> = (0..=12)
                .map(|i| {
                    let a = std::f32::consts::PI * (1.25 + 0.5 * i as f32 / 12.0);
                    sym(70.0 + radius * a.cos(), 80.0 + radius * a.sin())
                })
                .collect();
            for end in [pts[0], pts[pts.len() - 1]] {
                p.circle_filled(end, 5.0 * s, ink);
            }
            p.add(Shape::line(pts, Stroke::new(10.0 * s, ink)));
        }
    } else if symbol == Symbol::Impulse {
        // Tracé de 10 unités de large, extrémités arrondies.
        let pts: Vec<Pos2> = IMPULSE.iter().map(|&(x, y)| sym(x, y)).collect();
        p.add(Shape::line(pts, Stroke::new(10.0 * s, ink)));
    } else if symbol == Symbol::Play {
        // Triangle à coins arrondis par un trait de 12 : on l'approche par un triangle plein un peu agrandi.
        let c = (31.0 + 85.0 + 31.0) / 3.0;
        let grow = |x: f32, y: f32| sym(c + (x - c) * 1.17, 50.0 + (y - 50.0) * 1.17);
        p.add(Shape::convex_polygon(vec![grow(31.0, 19.0), grow(85.0, 50.0), grow(31.0, 81.0)], ink, Stroke::NONE));
    } else {
        // Arceau : demi-cercle de rayon 33,5 centré en (50, 52), prolongé jusqu'à y = 62, trait de 13.
        let mut pts = vec![sym(16.5, 62.0)];
        for i in 0..=24 {
            let a = std::f32::consts::PI * (1.0 + i as f32 / 24.0);
            pts.push(sym(50.0 + 33.5 * a.cos(), 52.0 + 33.5 * a.sin()));
        }
        pts.push(sym(83.5, 62.0));
        p.add(Shape::line(pts, Stroke::new(13.0 * s, ink)));
        // Écouteurs.
        for x in [10.0, 65.0] {
            p.rect_filled(Rect::from_min_max(sym(x, 50.0), sym(x + 25.0, 88.0)), CornerRadius::same(r(9.0 * s)), ink);
        }
    }
}

/// Reproduit `mediastar-icone-petite.svg` : quatre tuiles à bandeau de couleur portant le
/// symbole de chaque module, et l'étoile sur le pad orange du centre.
fn logo_media_small(p: &egui::Painter, rect: Rect) {
    const FOND: Color32 = Color32::from_rgb(0x22, 0x22, 0x28);
    const CASE: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x33);
    const LISERE: Color32 = Color32::from_rgb(0x1e, 0x1e, 0x23);
    const PAD: Color32 = Color32::from_rgb(0xed, 0xa8, 0x3a);
    const ENCRE: Color32 = Color32::from_rgb(0x20, 0x20, 0x26);
    let u = rect.height() / 1024.0;
    let r = |v: f32| (v * u).round().clamp(0.0, 255.0) as u8;
    let k = 1.28;
    let at = |x: f32, y: f32| rect.min + vec2((512.0 + (x - 512.0) * k) * u, (512.0 + (y - 512.0) * k) * u);

    p.rect_filled(rect, CornerRadius::same(r(230.0)), FOND);
    let tiles = [
        (180.8, 180.8, Color32::from_rgb(0x4a, 0x8f, 0xdb), Symbol::Headphones),
        (527.2, 180.8, Color32::from_rgb(0x4f, 0xae, 0x58), Symbol::Play),
        (180.8, 527.2, Color32::from_rgb(0x9b, 0x6b, 0xd3), Symbol::Fader),
        (527.2, 527.2, Color32::from_rgb(0x3f, 0xb8, 0xaf), Symbol::Impulse),
    ];
    for (x, y, color, symbol) in tiles {
        let rad = r(50.0 * k);
        p.rect_filled(Rect::from_min_max(at(x, y), at(x + 316.0, y + 316.0)), CornerRadius::same(rad), CASE);
        p.rect_filled(Rect::from_min_max(at(x, y), at(x + 316.0, y + 52.0)), CornerRadius { nw: rad, ne: rad, sw: 0, se: 0 }, color);
        // Symbole dans la couleur du bandeau : repère de 100 unités à l'échelle 1,75.
        let (ox, oy) = (x + 70.5, y + 96.5);
        draw_symbol(p, symbol, &|sx, sy| at(ox + sx * 1.75, oy + sy * 1.75), 1.75 * k * u, color);
    }
    p.rect_filled(Rect::from_min_max(at(392.0, 392.0), at(632.0, 632.0)), CornerRadius::same(r(58.0 * k)), LISERE);
    p.rect_filled(Rect::from_min_max(at(410.0, 410.0), at(614.0, 614.0)), CornerRadius::same(r(44.0 * k)), PAD);
    // Étoile à cinq branches, repère de 100 unités placé en (437, 437) à l'échelle 1,5.
    let star = |x: f32, y: f32| at(437.0 + x * 1.5, 437.0 + y * 1.5);
    let centre = star(50.0, 52.0);
    let pts: Vec<Pos2> = (0..10)
        .map(|i| {
            let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
            let len = if i % 2 == 0 { 41.0 } else { 16.5 } * 1.5 * k * u;
            centre + vec2(a.cos(), a.sin()) * len
        })
        .collect();
    // Étoile non convexe : un triangle par branche autour du centre.
    for i in 0..10 {
        p.add(Shape::convex_polygon(vec![centre, pts[i], pts[(i + 1) % 10]], ENCRE, Stroke::NONE));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Event, Modifiers, PointerButton, RawInput};

    /// Simule un glisser de `from` vers `to` sur l'éditeur placé dans une fenêtre, comme dans l'appli.
    fn drag(from: Pos2, to: Pos2, looping: bool) -> Markers {
        let ctx = egui::Context::default();
        let mut m = Markers { start: 0.0, loop_start: 0.0, loop_end: 10.0, end: 10.0 };
        let overview = |_: f64, _: f64, n: usize| vec![0.5f32; n];
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0));
        let frame = |events: Vec<Event>, m: &mut Markers| {
            let input = RawInput { screen_rect: Some(screen), events, ..Default::default() };
            let mut wave_rect = Rect::NOTHING;
            let mut out = ctx.run_ui(input, |ui| {
                egui::Window::new("t").fixed_pos(pos2(0.0, 0.0)).default_width(620.0).resizable(true).show(ui.ctx(), |ui| {
                    waveform_editor(ui, peaks_background(overview), 10.0, m, looping, &mut 0.0, &mut 0.0, None, &mut None);
                    wave_rect = ui.min_rect();
                });
            });
            out.textures_delta.clear();
            wave_rect
        };
        let r = frame(vec![], &mut m);
        let _ = frame(vec![], &mut m);
        println!("zone {r:?}");
        let ev = |p: Pos2, pressed: Option<bool>| match pressed {
            Some(pr) => Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: pr, modifiers: Modifiers::NONE },
            None => Event::PointerMoved(p),
        };
        frame(vec![ev(from, None)], &mut m);
        frame(vec![ev(from, Some(true))], &mut m);
        for i in 1..=10 {
            let p = from + (to - from) * (i as f32 / 10.0);
            frame(vec![ev(p, None)], &mut m);
        }
        frame(vec![ev(to, Some(false))], &mut m);
        m
    }

    #[test]
    fn double_click_types_a_db_value() {
        let ctx = egui::Context::default();
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(400.0, 300.0));
        let mut got = None;
        let frame = |events: Vec<Event>, got: &mut Option<f32>| {
            let input = RawInput { screen_rect: Some(screen), events, ..Default::default() };
            let mut out = ctx.run_ui(input, |ui| {
                let id = ui.make_persistent_id("db");
                if let Some(v) = db_value(ui, id, pos2(100.0, 100.0), Align2::CENTER_TOP, 0.0, -60.0, -60.0..=10.0, FontId::proportional(11.0), TEXT) {
                    *got = Some(v);
                }
            });
            out.textures_delta.clear();
        };
        let p = pos2(100.0, 106.0);
        let btn = |pressed| Event::PointerButton { pos: p, button: PointerButton::Primary, pressed, modifiers: Modifiers::NONE };
        frame(vec![], &mut got);
        frame(vec![Event::PointerMoved(p)], &mut got);
        frame(vec![btn(true), btn(false), btn(true), btn(false)], &mut got);
        frame(vec![], &mut got);
        frame(vec![], &mut got);
        frame(vec![Event::Key { key: Key::A, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::COMMAND }], &mut got);
        frame(vec![Event::Text("-6,5".into())], &mut got);
        frame(vec![Event::Key { key: Key::Enter, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }], &mut got);
        assert_eq!(got, Some(-6.5));
    }

    #[test]
    fn typed_db_values() {
        let r = -60.0..=10.0;
        assert_eq!(parse_db("-6", &r), Some(-6.0));
        assert_eq!(parse_db(" +3,5 dB", &r), Some(3.5));
        assert_eq!(parse_db("-4.5dB", &r), Some(-4.5));
        assert_eq!(parse_db("-inf", &r), Some(-60.0));
        assert_eq!(parse_db("-∞ dB", &r), Some(-60.0));
        assert_eq!(parse_db("+25", &r), Some(10.0));
        assert_eq!(parse_db("6", &r), Some(-6.0), "sans signe : négatif");
        assert_eq!(parse_db("+6", &r), Some(6.0));
        assert_eq!(parse_db("0", &r), Some(0.0));
        assert_eq!(parse_db("80", &r), Some(-60.0));
        assert_eq!(parse_db("abc", &r), None);
        assert_eq!(parse_db("", &r), None);
    }

    #[test]
    fn handles_are_draggable() {
        // Géométrie : fenêtre en (0,0), marge interne, éditeur pleine largeur.
        let probe = drag(pos2(-50.0, -50.0), pos2(-50.0, -50.0), false);
        assert_eq!(probe.end, 10.0);
        let ctx_rect = {
            let ctx = egui::Context::default();
            let mut r = Rect::NOTHING;
            let overview = |_: f64, _: f64, n: usize| vec![0.5f32; n];
            for _ in 0..2 {
                let input = RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, vec2(1000.0, 700.0))), ..Default::default() };
                let mut out = ctx.run_ui(input, |ui| {
                    egui::Window::new("t").fixed_pos(pos2(0.0, 0.0)).default_width(620.0).resizable(true).show(ui.ctx(), |ui| {
                        let mut m = Markers { start: 0.0, loop_start: 0.0, loop_end: 10.0, end: 10.0 };
                        waveform_editor(ui, peaks_background(overview), 10.0, &mut m, true, &mut 0.0, &mut 0.0, None, &mut None);
                        r = ui.min_rect();
                    });
                });
                out.textures_delta.clear();
            }
            r
        };
        let wave = ctx_rect.shrink2(vec2(HANDLE_PAD, 14.0));
        let y_low = ctx_rect.bottom() - 20.0;
        let y_top = wave.top() + 15.0;
        let mid_x = wave.center().x;

        let m = drag(pos2(wave.right(), y_low), pos2(mid_x, y_low), false);
        assert!((m.end - 5.0).abs() < 0.3, "fin déplacée : {m:?}");

        let m = drag(pos2(wave.left(), y_low), pos2(mid_x, y_low), false);
        assert!((m.start - 5.0).abs() < 0.3, "début déplacé : {m:?}");

        let m = drag(pos2(wave.right(), y_top), pos2(mid_x, y_top), true);
        assert!((m.loop_end - 5.0).abs() < 0.3 && m.end == 10.0, "fin de boucle seule : {m:?}");

        let m = drag(pos2(wave.left(), y_top), pos2(mid_x, y_top), true);
        assert!((m.loop_start - 5.0).abs() < 0.3 && m.start == 0.0, "début de boucle seul : {m:?}");
    }
}
