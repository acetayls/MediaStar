//! Traceur de courbes : axe des fréquences logarithmique (20 Hz–20 kHz) ou axe linéaire
//! (temps), plusieurs courbes superposées, lecture au survol, zoom vertical à la molette,
//! zoom horizontal à Ctrl + molette (ou Maj + molette), déplacement au glisser, double-clic
//! pour revenir à la vue par défaut.
//! Même allure que la courbe de l'égaliseur (`star_ui::eq_view`). À déplacer dans star-ui
//! une fois stable.

use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Shape, Stroke, Ui};

use star_ui::theme::*;

/// Axe horizontal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum XAxis {
    /// Fréquences (Hz), échelle logarithmique.
    LogFreq { min: f64, max: f64 },
    /// Échelle linéaire, avec son unité (« ms »).
    Linear { min: f64, max: f64, unit: &'static str },
}

impl XAxis {
    pub const AUDIO: XAxis = XAxis::LogFreq { min: 20.0, max: 20_000.0 };

    fn frac(&self, x: f64) -> f64 {
        match *self {
            XAxis::LogFreq { min, max } => (x.max(1e-9) / min).ln() / (max / min).ln(),
            XAxis::Linear { min, max, .. } => (x - min) / (max - min),
        }
    }

    fn value(&self, frac: f64) -> f64 {
        match *self {
            XAxis::LogFreq { min, max } => min * ((max / min).ln() * frac).exp(),
            XAxis::Linear { min, max, .. } => min + (max - min) * frac,
        }
    }

    fn contains(&self, x: f64) -> bool {
        let (a, b) = self.range();
        x >= a && x <= b
    }

    fn range(&self) -> (f64, f64) {
        match *self {
            XAxis::LogFreq { min, max } | XAxis::Linear { min, max, .. } => (min, max),
        }
    }

    fn with_range(&self, min: f64, max: f64) -> XAxis {
        match *self {
            XAxis::LogFreq { .. } => XAxis::LogFreq { min, max },
            XAxis::Linear { unit, .. } => XAxis::Linear { min, max, unit },
        }
    }
}

/// Vue horizontale, gardée par l'appelant : l'axe affiché (zoomé) et l'axe complet.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XView {
    pub axis: XAxis,
    pub default: XAxis,
}

impl XView {
    pub const fn new(axis: XAxis) -> Self {
        Self { axis, default: axis }
    }

    pub const AUDIO: XView = XView::new(XAxis::AUDIO);

    pub fn reset(&mut self) {
        self.axis = self.default;
    }

    /// Nouvel axe complet (la vue y revient).
    pub fn set_default(&mut self, axis: XAxis) {
        if self.default != axis {
            self.default = axis;
            self.axis = axis;
        }
    }

    /// Montre `[min, max]` (bornée à l'axe complet).
    pub fn show_range(&mut self, min: f64, max: f64) {
        self.axis = self.default.with_range(min, max);
        self.clamp();
    }

    /// Zoom de facteur `k` (< 1 : plus près) autour de la fraction `at` de la largeur.
    fn zoom(&mut self, at: f64, k: f64) {
        let a = self.axis;
        self.axis = a.with_range(a.value(at - at * k), a.value(at + (1.0 - at) * k));
        self.clamp();
    }

    /// Déplacement d'une fraction `d` de la largeur, sans sortir de l'axe complet.
    fn pan(&mut self, d: f64) {
        let a = self.axis;
        let (dmin, dmax) = self.default.range();
        let d = d.clamp(-a.frac(a.range().0) + a.frac(dmin), a.frac(dmax) - 1.0);
        self.axis = a.with_range(a.value(d), a.value(1.0 + d));
        self.clamp();
    }

    fn clamp(&mut self) {
        let (dmin, dmax) = self.default.range();
        let (mut min, mut max) = self.axis.range();
        min = min.max(dmin);
        max = max.min(dmax);
        // Largeur minimale : 1/20 d'octave en fréquence, 1/1000 de l'axe en linéaire.
        match self.default {
            XAxis::LogFreq { .. } => {
                if max / min < 2f64.powf(1.0 / 20.0) {
                    let c = (min * max).sqrt();
                    let h = 2f64.powf(1.0 / 40.0);
                    (min, max) = ((c / h).max(dmin), (c * h).min(dmax));
                }
            }
            XAxis::Linear { .. } => {
                let w = (dmax - dmin) / 1000.0;
                if max - min < w {
                    let c = 0.5 * (min + max);
                    (min, max) = ((c - w / 2.0).max(dmin), (c + w / 2.0).min(dmax));
                }
            }
        }
        self.axis = self.default.with_range(min, max);
    }
}

/// Vue verticale, gardée par l'appelant (elle change au zoom et au déplacement).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct YView {
    pub min: f64,
    pub max: f64,
    /// Vue rendue par un double-clic.
    pub default: (f64, f64),
}

impl YView {
    pub const fn new(min: f64, max: f64) -> Self {
        Self { min, max, default: (min, max) }
    }
}

/// Une courbe : points (x, y) dans l'ordre des x croissants.
pub struct Series<'a> {
    pub points: &'a [[f64; 2]],
    pub color: Color32,
    pub width: f32,
    pub name: &'a str,
    /// Barres pleines : les points vont par paires (bord bas, niveau), (bord haut, niveau).
    pub bars: bool,
}

impl<'a> Series<'a> {
    pub fn line(points: &'a [[f64; 2]], color: Color32, width: f32, name: &'a str) -> Self {
        Self { points, color, width, name, bars: false }
    }
}

/// Format des fréquences sur l'axe et au survol (« 1,5k », « 200 »).
pub fn fmt_freq(f: f64) -> String {
    if f >= 1000.0 {
        let s = format!("{:.1}k", f / 1000.0);
        s.replace(".0k", "k")
    } else if f >= 100.0 {
        format!("{f:.0}")
    } else {
        format!("{f:.1}").replace(".0", "")
    }
}

/// Pas de graduation « ronds » (1, 2, 5 × 10ⁿ) pour environ `n` divisions.
fn nice_step(span: f64, n: f64) -> f64 {
    let raw = (span / n).max(1e-12);
    let p = 10f64.powf(raw.log10().floor());
    [1.0, 2.0, 5.0, 10.0].into_iter().map(|m| m * p).find(|s| *s >= raw).unwrap_or(10.0 * p)
}

/// Trace les courbes dans un cadre de `height` points de haut, toute la largeur disponible.
/// `y_unit` suit les valeurs au survol (« dB », « ° »).
pub fn plot(ui: &mut Ui, height: f32, xv: &mut XView, y: &mut YView, y_unit: &str, series: &[Series]) -> egui::Response {
    let size = vec2(ui.available_width().max(200.0), height);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, CornerRadius::same(4), Color32::from_rgb(0x12, 0x12, 0x14));
    let area = Rect::from_min_max(rect.min + vec2(40.0, 8.0), rect.max - vec2(10.0, 18.0));
    if area.width() < 10.0 || area.height() < 10.0 {
        return resp;
    }

    // Molette : zoom vertical autour du pointeur ; Ctrl + molette (ou Maj + molette, ou le
    // pincement) : zoom horizontal. Glisser : déplacement dans les deux sens.
    if resp.hovered() {
        let (scroll, zoom) = ui.input(|i| (i.smooth_scroll_delta, i.zoom_delta()));
        if let Some(pt) = resp.hover_pos() {
            let fx = (((pt.x - area.left()) / area.width()) as f64).clamp(0.0, 1.0);
            if zoom != 1.0 {
                xv.zoom(fx, 1.0 / zoom as f64);
            } else if scroll.x != 0.0 {
                xv.zoom(fx, (-(scroll.x as f64) / 200.0).exp());
            } else if scroll.y != 0.0 {
                let at = y.max - ((pt.y - area.top()) / area.height()) as f64 * (y.max - y.min);
                let k = (-(scroll.y as f64) / 200.0).exp();
                y.min = at - (at - y.min) * k;
                y.max = at + (y.max - at) * k;
            }
        }
    }
    if resp.dragged() {
        let dy = resp.drag_delta().y as f64 / area.height() as f64 * (y.max - y.min);
        y.min += dy;
        y.max += dy;
        let dx = resp.drag_delta().x as f64 / area.width() as f64;
        if dx != 0.0 && xv.axis != xv.default {
            xv.pan(-dx);
        }
    }
    if resp.double_clicked() {
        (y.min, y.max) = y.default;
        xv.reset();
    }
    let x = xv.axis;
    if y.max - y.min < 1e-6 {
        y.max = y.min + 1e-6;
    }

    let x_of = |v: f64| area.left() + (x.frac(v) as f32) * area.width();
    let y_of = |v: f64| area.bottom() - ((v - y.min) / (y.max - y.min)) as f32 * area.height();

    // Grille.
    let grid = Stroke::new(1.0, Color32::from_white_alpha(14));
    let font = FontId::proportional(9.5);
    match x {
        XAxis::LogFreq { .. } => {
            // Graduations 1, 2, 5 × 10ⁿ ; plus serrées une fois zoomé.
            let (lo, hi) = x.range();
            let mults: &[f64] = if hi / lo < 8.0 { &[1.0, 1.5, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0] } else { &[1.0, 2.0, 5.0] };
            let ticks: Vec<f64> = (0..6).flat_map(|e| mults.iter().map(move |m| m * 10f64.powi(e))).filter(|f| (10.0..=24_000.0).contains(f)).collect();
            for f in ticks {
                if x.contains(f) {
                    let px = x_of(f);
                    p.line_segment([pos2(px, area.top()), pos2(px, area.bottom())], grid);
                    p.text(pos2(px, area.bottom() + 3.0), Align2::CENTER_TOP, fmt_freq(f), font.clone(), TEXT_DIM);
                }
            }
        }
        XAxis::Linear { min, max, unit } => {
            let step = nice_step(max - min, 8.0);
            let mut v = (min / step).ceil() * step;
            while v <= max + 1e-9 {
                let px = x_of(v);
                p.line_segment([pos2(px, area.top()), pos2(px, area.bottom())], grid);
                p.text(pos2(px, area.bottom() + 3.0), Align2::CENTER_TOP, trim(v), font.clone(), TEXT_DIM);
                v += step;
            }
            p.text(pos2(area.left() - 4.0, area.bottom() + 3.0), Align2::RIGHT_TOP, unit, font.clone(), TEXT_DIM);
        }
    }
    let step = nice_step(y.max - y.min, (area.height() / 28.0).max(2.0) as f64);
    let mut v = (y.min / step).ceil() * step;
    while v <= y.max + 1e-9 {
        let py = y_of(v);
        let s = if v.abs() < step * 1e-6 { Stroke::new(1.0, Color32::from_white_alpha(40)) } else { grid };
        p.line_segment([pos2(area.left(), py), pos2(area.right(), py)], s);
        p.text(pos2(area.left() - 4.0, py), Align2::RIGHT_CENTER, trim(v), font.clone(), TEXT_DIM);
        v += step;
    }
    p.rect_stroke(area, CornerRadius::ZERO, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);

    // Courbes : une colonne de pixels garde son minimum et son maximum (des milliers de cases
    // FFT par pixel dans l'aigu).
    let clip = ui.painter_at(area);
    for s in series.iter().filter(|s| s.bars) {
        // Une barre par bande, du bas du cadre jusqu'au niveau, avec un liseré clair en haut.
        for pair in s.points.as_chunks::<2>().0 {
            let ([x0, v], [x1, _]) = (pair[0], pair[1]);
            if !v.is_finite() || x1 < x.value(0.0) || x0 > x.value(1.0) {
                continue;
            }
            let (l, r) = (x_of(x0.max(x.value(0.0))), x_of(x1.min(x.value(1.0))));
            let gap = if r - l > 4.0 { 1.0 } else { 0.0 };
            let top = y_of(v).clamp(area.top(), area.bottom());
            let bar = Rect::from_min_max(pos2(l + gap, top), pos2((r - gap).max(l + gap + 0.5), area.bottom()));
            clip.rect_filled(bar, CornerRadius::ZERO, s.color.gamma_multiply(0.55));
            clip.line_segment([bar.left_top(), bar.right_top()], Stroke::new(s.width, s.color));
        }
    }
    for s in series.iter().filter(|s| !s.bars) {
        let mut pts: Vec<Pos2> = Vec::new();
        let mut col: Option<(i32, f32, f32, f32)> = None; // (colonne, x, min, max)
        let flush = |pts: &mut Vec<Pos2>, c: (i32, f32, f32, f32)| {
            let (_, px, lo, hi) = c;
            pts.push(pos2(px, lo));
            if hi != lo {
                pts.push(pos2(px, hi));
            }
        };
        for &[vx, vy] in s.points {
            if !x.contains(vx) || !vy.is_finite() {
                continue;
            }
            let px = x_of(vx);
            let py = y_of(vy).clamp(area.top() - 2.0, area.bottom() + 2.0);
            let c = px.floor() as i32;
            match &mut col {
                Some((cc, _, lo, hi)) if *cc == c => {
                    *lo = lo.min(py);
                    *hi = hi.max(py);
                }
                _ => {
                    if let Some(prev) = col.take() {
                        flush(&mut pts, prev);
                    }
                    col = Some((c, px, py, py));
                }
            }
        }
        if let Some(prev) = col {
            flush(&mut pts, prev);
        }
        if pts.len() >= 2 {
            clip.add(Shape::line(pts, Stroke::new(s.width, s.color)));
        }
    }

    // Lecture au survol : valeur de chaque courbe à l'abscisse du pointeur.
    if let Some(pt) = resp.hover_pos().filter(|pt| area.contains(*pt)) {
        let vx = x.value(((pt.x - area.left()) / area.width()) as f64);
        clip.line_segment([pos2(pt.x, area.top()), pos2(pt.x, area.bottom())], Stroke::new(1.0, Color32::from_white_alpha(60)));
        let xs = match x {
            XAxis::LogFreq { .. } => format!("{} Hz", fmt_freq(vx)),
            XAxis::Linear { unit, .. } => format!("{vx:.2} {unit}"),
        };
        let mut lines = vec![(xs, TEXT)];
        for s in series {
            let i = s.points.partition_point(|q| q[0] < vx);
            if let Some(q) = s.points.get(i).or_else(|| s.points.last()) {
                if q[1].is_finite() {
                    lines.push((format!("{} : {:.1} {y_unit}", s.name, q[1]), s.color));
                }
            }
        }
        let mut ty = area.top() + 4.0;
        let tx = if pt.x > area.center().x { area.left() + 6.0 } else { area.right() - 6.0 };
        let align = if pt.x > area.center().x { Align2::LEFT_TOP } else { Align2::RIGHT_TOP };
        for (text, c) in lines {
            let r = clip.text(pos2(tx, ty), align, text, FontId::proportional(11.0), c);
            ty = r.bottom() + 1.0;
        }
    }
    resp
}

fn trim(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
    if s == "-0" {
        "0".into()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axes() {
        let x = XAxis::AUDIO;
        assert!((x.frac(20.0)).abs() < 1e-12);
        assert!((x.frac(20_000.0) - 1.0).abs() < 1e-12);
        assert!((x.value(x.frac(1234.0)) - 1234.0).abs() < 1e-9);
        assert_eq!(fmt_freq(1500.0), "1.5k");
        assert_eq!(fmt_freq(20_000.0), "20k");
        assert_eq!(nice_step(100.0, 8.0), 20.0);
        assert_eq!(trim(-0.0), "0");
        // Zoom horizontal autour du milieu, déplacement borné, retour.
        let mut v = XView::AUDIO;
        v.zoom(0.5, 0.1);
        let (a, b) = v.axis.range();
        assert!(a > 400.0 && b < 1100.0 && (a * b).sqrt() > 600.0, "{a} {b}");
        v.pan(-10.0);
        assert!((v.axis.range().0 - 20.0).abs() < 1e-9);
        v.zoom(0.5, 100.0);
        assert_eq!(v.axis, XAxis::AUDIO);
    }
}
