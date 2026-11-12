//! Icônes dessinées à la main.
//!
//! Les polices intégrées à egui n'ont pas la plupart des symboles (⚙, ✏, 🗑, ✓, ⟳…) :
//! selon la machine, ils venaient d'une police système de taille et de ligne de base
//! différentes, ou ne s'affichaient pas du tout. Ici, chaque icône est un dessin vectoriel,
//! centré dans un carré : même rendu partout, et toujours aligné sur le texte voisin.

use egui::{pos2, vec2, Atom, Color32, CornerRadius, Painter, Pos2, Rect, Response, RichText, Sense, Shape, Stroke, TextStyle, Ui, WidgetText};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Icon {
    /// ■ arrêt
    Stop,
    /// ● pastille (modifications non enregistrées, état)
    Dot,
    /// ▶ lecture, cue en attente
    Play,
    /// ⏸ pause
    Pause,
    /// ⏮ précédent
    Prev,
    /// ⏭ suivant
    Next,
    /// ⚙ préférences
    Gear,
    /// ✏ mode édition
    Pencil,
    /// 🗑 supprimer, vider
    Trash,
    /// ✓ terminé
    Check,
    /// ✕ effacer, fermer
    Close,
    /// ＋ ajouter
    Plus,
    /// ⟳ relire, rafraîchir (sens horaire)
    Refresh,
    /// ⟲ flèche circulaire en sens antihoraire
    RefreshBack,
    /// ⚠ alerte
    Warning,
    /// ⓘ information
    Info,
    /// 🔍 rechercher
    Search,
    /// ★ par défaut
    Star,
    /// ♪ morceau
    Note,
    /// ▾ menu déroulant
    Caret,
    /// télécommande (boîtier à touches)
    Remote,
    /// livre ouvert (aide)
    Book,
}

impl Icon {
    /// L'icône qui remplace ce caractère dans un texte (aide, bandeaux), s'il y en a une.
    pub fn from_char(c: char) -> Option<Icon> {
        Some(match c {
            '■' => Icon::Stop,
            '●' => Icon::Dot,
            '▶' => Icon::Play,
            '⏸' => Icon::Pause,
            '⏮' => Icon::Prev,
            '⏭' => Icon::Next,
            '⚙' => Icon::Gear,
            '✏' | '✎' => Icon::Pencil,
            '🗑' => Icon::Trash,
            '✓' | '✔' => Icon::Check,
            '✕' | '✖' => Icon::Close,
            '＋' => Icon::Plus,
            '⟳' => Icon::Refresh,
            '⟲' => Icon::RefreshBack,
            '⚠' => Icon::Warning,
            'ⓘ' => Icon::Info,
            '🔍' => Icon::Search,
            '★' => Icon::Star,
            '♪' => Icon::Note,
            '▾' => Icon::Caret,
            _ => return None,
        })
    }
}

/// Côté d'une icône dans un bouton ordinaire, et dans un petit bouton.
pub const SIZE: f32 = 12.0;
pub const SIZE_SMALL: f32 = 10.0;

/// Icône à mettre dans un bouton, avant ou après le texte : `ui.button((atom(Icon::Gear), "Préférences"))`.
/// Elle prend la couleur du texte du bouton (grisée quand il est désactivé).
pub fn atom<'a>(icon: Icon) -> Atom<'a> {
    atom_with(icon, SIZE, None)
}

/// Comme [`atom`], pour un `small_button`.
pub fn atom_small<'a>(icon: Icon) -> Atom<'a> {
    atom_with(icon, SIZE_SMALL, None)
}

/// Comme [`atom`], dans une couleur imposée.
pub fn atom_colored<'a>(icon: Icon, color: Color32) -> Atom<'a> {
    atom_with(icon, SIZE, Some(color))
}

pub fn atom_with<'a>(icon: Icon, size: f32, color: Option<Color32>) -> Atom<'a> {
    Atom::paint(vec2(size, size), move |ui, args| {
        let col = color.unwrap_or(args.fallback_text_color);
        paint(ui.painter(), Rect::from_center_size(args.rect.center(), vec2(size, size)), icon, col);
    })
}

/// Icône suivie d'un texte, comme un `ui.label` : l'icône est centrée sur la ligne du texte
/// et prend la taille de sa police. Sans couleur dans `text`, le texte garde la couleur normale.
pub fn label(ui: &mut Ui, icon: Icon, color: Color32, text: impl Into<WidgetText>) -> Response {
    let galley = text.into().into_galley(ui, Some(egui::TextWrapMode::Extend), f32::INFINITY, TextStyle::Body);
    let row = galley.size().y.max(8.0);
    let s = (row * 0.8).round();
    let gap = (s * 0.4).round().max(3.0);
    let (rect, resp) = ui.allocate_exact_size(vec2(s + gap + galley.size().x, row), Sense::hover());
    if ui.is_rect_visible(rect) {
        let icon_rect = Rect::from_center_size(pos2(rect.left() + s / 2.0, rect.center().y), vec2(s, s));
        paint(ui.painter(), icon_rect, icon, color);
        let text_pos = pos2(rect.left() + s + gap, rect.center().y - galley.size().y / 2.0);
        ui.painter().galley(text_pos, galley, ui.visuals().text_color());
    }
    resp
}

/// Icône seule, de la hauteur d'une ligne de texte.
pub fn icon_only(ui: &mut Ui, icon: Icon, color: Color32) -> Response {
    let row = ui.text_style_height(&TextStyle::Body);
    let s = (row * 0.8).round();
    let (rect, resp) = ui.allocate_exact_size(vec2(s + 2.0, row), Sense::hover());
    if ui.is_rect_visible(rect) {
        paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(s, s)), icon, color);
    }
    resp
}

/// Texte dont certains caractères (⚠, ✏, 🗑, ⏸…) sont remplacés par l'icône dessinée.
/// À appeler dans une rangée (`horizontal` ou `horizontal_wrapped`) ; `style` met en forme
/// chaque morceau de texte, `icon_color` colore les icônes.
pub fn text_with_icons(ui: &mut Ui, text: &str, style: impl Fn(&str) -> RichText, icon_color: Color32) {
    if !text.chars().any(|c| Icon::from_char(c).is_some()) {
        ui.label(style(text));
        return;
    }
    let spacing = ui.spacing().item_spacing.x;
    ui.spacing_mut().item_spacing.x = 0.0;
    let mut run = String::new();
    for c in text.chars() {
        match Icon::from_char(c) {
            Some(icon) => {
                if !run.is_empty() {
                    ui.label(style(&run));
                    run.clear();
                }
                icon_only(ui, icon, icon_color);
            }
            // Variante emoji (U+FE0F) qui suit parfois un symbole : sans objet ici.
            None if c == '\u{FE0F}' => {}
            None => run.push(c),
        }
    }
    if !run.is_empty() {
        ui.label(style(&run));
    }
    ui.spacing_mut().item_spacing.x = spacing;
}

/// Dessine l'icône dans le carré `rect` (centrée, à la taille du plus petit côté).
pub fn paint(p: &Painter, rect: Rect, icon: Icon, col: Color32) {
    let c = rect.center();
    let s = rect.width().min(rect.height());
    let h = s / 2.0;
    let pt = |x: f32, y: f32| -> Pos2 { c + vec2(x, y) * h };
    // Trait de base : assez épais pour rester net en petit.
    let w = (s * 0.12).max(1.3);
    let stroke = |k: f32| Stroke::new(w * k, col);
    let poly = |pts: Vec<Pos2>| Shape::convex_polygon(pts, col, Stroke::NONE);
    let line = |pts: Vec<Pos2>, k: f32| Shape::line(pts, stroke(k));

    match icon {
        Icon::Stop => {
            p.rect_filled(Rect::from_center_size(c, vec2(1.4 * h, 1.4 * h)), CornerRadius::same(1), col);
        }
        Icon::Dot => {
            p.circle_filled(c, 0.5 * h, col);
        }
        Icon::Play => {
            p.add(poly(vec![pt(-0.6, -0.85), pt(0.85, 0.0), pt(-0.6, 0.85)]));
        }
        Icon::Pause => {
            let bar = |x0: f32| Rect::from_min_max(pt(x0, -0.8), pt(x0 + 0.5, 0.8));
            p.rect_filled(bar(-0.7), CornerRadius::same(1), col);
            p.rect_filled(bar(0.2), CornerRadius::same(1), col);
        }
        Icon::Prev => {
            p.rect_filled(Rect::from_min_max(pt(-0.85, -0.75), pt(-0.55, 0.75)), CornerRadius::ZERO, col);
            p.add(poly(vec![pt(0.8, -0.75), pt(0.8, 0.75), pt(-0.45, 0.0)]));
        }
        Icon::Next => {
            p.rect_filled(Rect::from_min_max(pt(0.55, -0.75), pt(0.85, 0.75)), CornerRadius::ZERO, col);
            p.add(poly(vec![pt(-0.8, -0.75), pt(0.45, 0.0), pt(-0.8, 0.75)]));
        }
        Icon::Gear => {
            for i in 0..8 {
                let a = i as f32 * std::f32::consts::FRAC_PI_4;
                let (sin, cos) = a.sin_cos();
                p.line_segment([pt(cos * 0.6, sin * 0.6), pt(cos * 0.92, sin * 0.92)], Stroke::new(0.34 * h, col));
            }
            p.circle_stroke(c, 0.5 * h, Stroke::new(0.36 * h, col));
        }
        Icon::Pencil => {
            // Crayon en diagonale, pointe en bas à gauche.
            let d = vec2(1.0, -1.0) / std::f32::consts::SQRT_2;
            let n = vec2(1.0, 1.0) / std::f32::consts::SQRT_2;
            let k = 0.3 * h;
            let tip = pt(-0.85, 0.85);
            let b0 = tip + d * (0.55 * h);
            let s0 = b0 + d * (0.18 * h);
            let b1 = pt(0.85, -0.85);
            p.add(poly(vec![tip, b0 + n * k, b0 - n * k]));
            p.add(poly(vec![s0 + n * k, b1 + n * k, b1 - n * k, s0 - n * k]));
        }
        Icon::Trash => {
            p.line_segment([pt(-0.85, -0.55), pt(0.85, -0.55)], stroke(1.0));
            p.add(line(vec![pt(-0.3, -0.55), pt(-0.3, -0.85), pt(0.3, -0.85), pt(0.3, -0.55)], 0.9));
            p.add(line(vec![pt(-0.6, -0.3), pt(-0.48, 0.88), pt(0.48, 0.88), pt(0.6, -0.3)], 1.0));
            p.line_segment([pt(-0.18, -0.05), pt(-0.18, 0.6)], stroke(0.8));
            p.line_segment([pt(0.18, -0.05), pt(0.18, 0.6)], stroke(0.8));
        }
        Icon::Check => {
            p.add(line(vec![pt(-0.8, 0.0), pt(-0.25, 0.55), pt(0.8, -0.6)], 1.5));
        }
        Icon::Close => {
            p.line_segment([pt(-0.65, -0.65), pt(0.65, 0.65)], stroke(1.4));
            p.line_segment([pt(-0.65, 0.65), pt(0.65, -0.65)], stroke(1.4));
        }
        Icon::Plus => {
            p.line_segment([pt(-0.8, 0.0), pt(0.8, 0.0)], stroke(1.8));
            p.line_segment([pt(0.0, -0.8), pt(0.0, 0.8)], stroke(1.8));
        }
        Icon::Refresh | Icon::RefreshBack => {
            // Arc de 280° terminé par une pointe ; `m` retourne le dessin pour le sens antihoraire.
            let m = if icon == Icon::Refresh { 1.0 } else { -1.0 };
            let r = 0.66;
            let a0 = (-60f32).to_radians();
            let a1 = a0 + 280f32.to_radians();
            let arc: Vec<Pos2> = (0..=24)
                .map(|i| {
                    let a = a0 + (a1 - a0) * i as f32 / 24.0;
                    pt(m * a.cos() * r, a.sin() * r)
                })
                .collect();
            p.add(Shape::line(arc, stroke(1.1)));
            let (sin, cos) = a1.sin_cos();
            let (px, py) = (cos * r, sin * r);
            let (tx, ty) = (-sin, cos);
            p.add(poly(vec![
                pt(m * (px + tx * 0.5), py + ty * 0.5),
                pt(m * (px + cos * 0.38 - tx * 0.05), py + sin * 0.38 - ty * 0.05),
                pt(m * (px - cos * 0.38 - tx * 0.05), py - sin * 0.38 - ty * 0.05),
            ]));
        }
        Icon::Warning => {
            p.add(Shape::closed_line(vec![pt(0.0, -0.88), pt(0.95, 0.8), pt(-0.95, 0.8)], stroke(1.0)));
            p.line_segment([pt(0.0, -0.32), pt(0.0, 0.22)], stroke(1.1));
            p.circle_filled(pt(0.0, 0.52), w * 0.6, col);
        }
        Icon::Info => {
            p.circle_stroke(c, 0.88 * h, stroke(1.0));
            p.circle_filled(pt(0.0, -0.45), w * 0.65, col);
            p.line_segment([pt(0.0, -0.1), pt(0.0, 0.5)], stroke(1.15));
        }
        Icon::Search => {
            p.circle_stroke(pt(-0.2, -0.2), 0.55 * h, stroke(1.1));
            p.line_segment([pt(0.22, 0.22), pt(0.85, 0.85)], stroke(1.7));
        }
        Icon::Star => {
            // Étoile à cinq branches : pentagone central et cinq pointes (formes convexes).
            let at = |i: usize, r: f32| {
                let a = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::PI / 5.0;
                pt(a.cos() * r, a.sin() * r + 0.08)
            };
            let inner: Vec<Pos2> = (0..5).map(|i| at(2 * i + 1, 0.42)).collect();
            p.add(poly(inner.clone()));
            for i in 0..5 {
                p.add(poly(vec![inner[(i + 4) % 5], at(2 * i, 0.98), inner[i]]));
            }
        }
        Icon::Caret => {
            p.add(poly(vec![pt(-0.6, -0.3), pt(0.6, -0.3), pt(0.0, 0.4)]));
        }
        Icon::Remote => {
            // Boîtier arrondi, gros bouton rond en haut, quatre petites touches.
            let r = Rect::from_min_max(pt(-0.5, -0.95), pt(0.5, 0.95));
            p.rect_stroke(r, CornerRadius::same((0.25 * h) as u8), stroke(0.9), egui::StrokeKind::Middle);
            p.circle_filled(pt(0.0, -0.5), 0.2 * h, col);
            for (x, y) in [(-0.2, 0.05), (0.2, 0.05), (-0.2, 0.45), (0.2, 0.45)] {
                p.circle_filled(pt(x, y), 0.1 * h, col);
            }
        }
        Icon::Book => {
            // Deux pages pleines, séparées par la reliure.
            p.add(poly(vec![pt(-0.95, -0.65), pt(-0.08, -0.5), pt(-0.08, 0.8), pt(-0.95, 0.62)]));
            p.add(poly(vec![pt(0.08, -0.5), pt(0.95, -0.65), pt(0.95, 0.62), pt(0.08, 0.8)]));
        }
        Icon::Note => {
            p.circle_filled(pt(-0.3, 0.5), 0.36 * h, col);
            p.line_segment([pt(0.05, 0.5), pt(0.05, -0.85)], stroke(1.0));
            p.line_segment([pt(0.05, -0.85), pt(0.6, -0.45)], stroke(1.3));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_map_to_icons() {
        for c in "■●▶⏸⏮⏭⚙✏🗑✓✕＋⟳⟲⚠ⓘ🔍★♪▾".chars() {
            assert!(Icon::from_char(c).is_some(), "{c}");
        }
        assert_eq!(Icon::from_char('a'), None);
        assert_eq!(Icon::from_char('→'), None);
    }
}
