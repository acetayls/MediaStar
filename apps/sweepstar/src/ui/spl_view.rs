//! Sonomètre : bandeau de niveaux en tête du RTA, et fenêtre détachable (toujours devant)
//! avec la valeur choisie en très grand, les autres niveaux, l'historique des 15 dernières
//! minutes et les seuils réglementaires. Les niveaux viennent du fil de mesure (spl.rs) et
//! sont lus en dB SPL une fois le micro calibré, en dBFS sinon.

use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontId, RichText, Sense, Shape, Stroke};
use star_core::i18n::tr;
use star_ui::theme::*;

use crate::controller::{Controller, SplMain};
use crate::spl::SplState;

pub fn viewport() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("sweepstar-sonometre")
}

const MAINS: [SplMain; 5] = [SplMain::LaFast, SplMain::LaSlow, SplMain::Laeq1m, SplMain::Laeq15m, SplMain::Lceq15m];

fn main_label(m: SplMain) -> &'static str {
    match m {
        SplMain::LaFast => tr("LA rapide", "LA fast"),
        SplMain::LaSlow => tr("LA lent", "LA slow"),
        SplMain::Laeq1m => "LAeq 1 min",
        SplMain::Laeq15m => "LAeq 15 min",
        SplMain::Lceq15m => "LCeq 15 min",
    }
}

/// Niveaux lus (calibration ajoutée) et leur unité.
struct Reading<'a> {
    s: &'a SplState,
    cal: f64,
    calibrated: bool,
}

impl Reading<'_> {
    fn v(&self, dbfs: f64) -> Option<f64> {
        (dbfs > -150.0).then_some(dbfs + self.cal)
    }

    fn main(&self, m: SplMain) -> Option<f64> {
        let s = self.s;
        self.v(match m {
            SplMain::LaFast => s.la_fast,
            SplMain::LaSlow => s.la_slow,
            SplMain::Laeq1m => s.laeq_1m,
            SplMain::Laeq15m => s.laeq_15m,
            SplMain::Lceq15m => s.lceq_15m,
        })
    }

    fn unit(&self, c_weighted: bool) -> &'static str {
        match (self.calibrated, c_weighted) {
            (true, false) => "dB(A)",
            (true, true) => "dB(C)",
            (false, false) => "dBFS(A)",
            (false, true) => "dBFS(C)",
        }
    }
}

fn fmt(v: Option<f64>) -> String {
    v.map_or_else(|| "—".to_string(), |v| format!("{v:.1}"))
}

/// Couleur d'un niveau face à son seuil : vert, orange à 3 dB du seuil, rouge au-delà.
fn level_color(v: Option<f64>, limit: f64, calibrated: bool) -> Color32 {
    match v {
        Some(v) if calibrated && v >= limit => STOP,
        Some(v) if calibrated && v >= limit - 3.0 => METER_MID,
        Some(_) if calibrated => GO,
        _ => TEXT,
    }
}

fn limit_for(m: SplMain, ctrl: &Controller) -> f64 {
    if m == SplMain::Lceq15m {
        ctrl.prefs.spl_limit_c
    } else {
        ctrl.prefs.spl_limit_a
    }
}

/// Bandeau en tête du RTA : valeur principale en grand, les autres à côté, bouton Détacher.
pub fn strip(ui: &mut egui::Ui, ctrl: &mut Controller) {
    let s = ctrl.spl_state();
    let r = Reading { s: &s, cal: ctrl.prefs.mic_cal_db.unwrap_or(0.0), calibrated: ctrl.prefs.mic_cal_db.is_some() };
    let m = ctrl.prefs.spl_main;
    ui.horizontal(|ui| {
        let v = r.main(m);
        ui.vertical(|ui| {
            ui.label(RichText::new(main_label(m)).small().color(TEXT_DIM));
            ui.horizontal(|ui| {
                ui.label(RichText::new(fmt(v)).size(40.0).strong().monospace().color(level_color(v, limit_for(m, ctrl), r.calibrated)));
                ui.label(RichText::new(r.unit(m == SplMain::Lceq15m)).size(16.0).color(TEXT_DIM));
            });
        });
        ui.add_space(18.0);
        for (label, val, c) in [
            (tr("LA rapide", "LA fast"), r.v(s.la_fast), false),
            ("LAeq 1 min", r.v(s.laeq_1m), false),
            ("LAeq 15 min", r.v(s.laeq_15m), false),
            ("LCeq 15 min", r.v(s.lceq_15m), true),
            (tr("LA max", "LA max"), r.v(s.la_max), false),
        ] {
            let limit = if c { ctrl.prefs.spl_limit_c } else { ctrl.prefs.spl_limit_a };
            ui.vertical(|ui| {
                ui.label(RichText::new(label).small().color(TEXT_DIM));
                ui.label(RichText::new(fmt(val)).size(18.0).monospace().color(level_color(val, limit, r.calibrated)));
            });
            ui.add_space(8.0);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .selectable_label(ctrl.prefs.spl_window, tr("Détacher", "Detach"))
                .on_hover_text(tr("Ouvre le sonomètre dans une fenêtre à part, toujours devant", "Opens the sound level meter in its own window, always on top"))
                .clicked()
            {
                ctrl.prefs.spl_window = !ctrl.prefs.spl_window;
                if ctrl.prefs.spl_window {
                    star_ui::floating::raise(ui.ctx(), viewport());
                }
            }
        });
    });
    if !r.calibrated {
        ui.label(RichText::new(tr("Micro non calibré : niveaux en dBFS (Calibrer… pour lire des dB SPL).", "Mic not calibrated: levels in dBFS (Calibrate… to read dB SPL).")).small().color(TEXT_DIM));
    }
}

/// Fenêtre détachée du sonomètre ; la refermer remet `spl_window` à faux.
pub fn window(ctx: &egui::Context, ctrl: &mut Controller) {
    let vp = egui::ViewportBuilder::default()
        .with_title(format!("SweepStar — {}", tr("Niveau sonore", "Sound level")))
        .with_inner_size([420.0, 460.0])
        .with_min_inner_size([260.0, 220.0])
        .with_window_level(egui::WindowLevel::AlwaysOnTop);
    let vp = star_ui::floating::placed(ctx, "sonometre", vp);
    let mut open = true;
    ctx.show_viewport_immediate(viewport(), vp, |ui, _| {
        star_ui::floating::track(ui.ctx(), "sonometre");
        if ui.ctx().input(|i| i.viewport().close_requested()) {
            open = false;
        }
        if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            ctrl.stop_all();
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(12)).show(ui, |ui| content(ui, ctrl));
    });
    if !open {
        ctrl.prefs.spl_window = false;
    }
}

fn content(ui: &mut egui::Ui, ctrl: &mut Controller) {
    let s = ctrl.spl_state();
    let r = Reading { s: &s, cal: ctrl.prefs.mic_cal_db.unwrap_or(0.0), calibrated: ctrl.prefs.mic_cal_db.is_some() };
    let m = ctrl.prefs.spl_main;

    ui.horizontal(|ui| {
        egui::ComboBox::from_id_salt("spl-principal").selected_text(main_label(m)).width(130.0).show_ui(ui, |ui| {
            for k in MAINS {
                ui.selectable_value(&mut ctrl.prefs.spl_main, k, main_label(k));
            }
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button(tr("Remettre à zéro", "Reset")).on_hover_text(tr("Repart de zéro pour les Leq, le maximum et la crête", "Restarts the Leq values, maximum and peak")).clicked() {
                ctrl.reset_spl();
            }
        });
    });

    // Valeur principale, à la taille de la fenêtre.
    let v = r.main(m);
    let col = level_color(v, limit_for(m, ctrl), r.calibrated);
    let w = ui.available_width();
    let size = (w / 4.2).clamp(36.0, 160.0);
    ui.vertical_centered(|ui| {
        ui.label(RichText::new(fmt(v)).size(size).strong().monospace().color(col));
        ui.label(RichText::new(r.unit(m == SplMain::Lceq15m)).size((size / 4.0).max(14.0)).color(TEXT_DIM));
    });
    if !r.calibrated {
        ui.vertical_centered(|ui| {
            ui.label(RichText::new(tr("Micro non calibré : dBFS", "Mic not calibrated: dBFS")).small().color(ACCENT));
        });
    }
    ui.add_space(6.0);

    // Les autres niveaux.
    let limit_a = ctrl.prefs.spl_limit_a;
    let limit_c = ctrl.prefs.spl_limit_c;
    egui::Grid::new("spl-valeurs").num_columns(4).spacing([14.0, 4.0]).show(ui, |ui| {
        let cell = |ui: &mut egui::Ui, label: &str, val: Option<f64>, col: Color32| {
            ui.label(RichText::new(label).color(TEXT_DIM));
            ui.label(RichText::new(fmt(val)).size(17.0).monospace().color(col));
        };
        cell(ui, tr("LA rapide", "LA fast"), r.v(s.la_fast), TEXT);
        cell(ui, tr("LA lent", "LA slow"), r.v(s.la_slow), TEXT);
        ui.end_row();
        cell(ui, "LAeq 1 min", r.v(s.laeq_1m), level_color(r.v(s.laeq_1m), limit_a, r.calibrated));
        cell(ui, "LAeq 15 min", r.v(s.laeq_15m), level_color(r.v(s.laeq_15m), limit_a, r.calibrated));
        ui.end_row();
        cell(ui, tr("LC rapide", "LC fast"), r.v(s.lc_fast), TEXT);
        cell(ui, "LCeq 15 min", r.v(s.lceq_15m), level_color(r.v(s.lceq_15m), limit_c, r.calibrated));
        ui.end_row();
        cell(ui, tr("LZ rapide", "LZ fast"), r.v(s.lz_fast), TEXT);
        cell(ui, tr("Crête", "Peak"), r.v(s.peak), if s.peak > -1.0 { STOP } else { TEXT });
        ui.end_row();
        cell(ui, tr("LA max", "LA max"), r.v(s.la_max), TEXT);
        cell(ui, "LAeq", r.v(s.laeq_total), TEXT);
        ui.end_row();
        cell(ui, "LCeq", r.v(s.lceq_total), TEXT);
        ui.end_row();
    });
    ui.label(RichText::new(format!("{} {}", tr("LAeq et LCeq depuis la remise à zéro :", "LAeq and LCeq since reset:"), fmt_duration(s.total_s))).small().color(TEXT_DIM));
    ui.add_space(6.0);

    // Historique : LAeq de chaque seconde sur 15 minutes, seuil en rouge.
    let h = (ui.available_height() - 34.0).max(60.0);
    history(ui, h, &s, r.cal, limit_a, r.calibrated);

    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("Seuils sur 15 min", "15-min limits")).small().color(TEXT_DIM));
        ui.add(egui::DragValue::new(&mut ctrl.prefs.spl_limit_a).range(60.0..=140.0).speed(0.5).max_decimals(1).suffix(" dB(A)"));
        ui.add(egui::DragValue::new(&mut ctrl.prefs.spl_limit_c).range(60.0..=150.0).speed(0.5).max_decimals(1).suffix(" dB(C)"));
    })
    .response
    .on_hover_text(tr(
        "En France (décret 2017-1244) : 102 dB(A) et 118 dB(C) en Leq sur 15 minutes ; 94 dB(A) et 104 dB(C) pour les spectacles destinés aux enfants.",
        "In France (decree 2017-1244): 102 dB(A) and 118 dB(C) Leq over 15 minutes; 94 dB(A) and 104 dB(C) for shows for children.",
    ));
}

fn fmt_duration(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    if s >= 3600 {
        format!("{} h {:02} min", s / 3600, s / 60 % 60)
    } else {
        format!("{} min {:02} s", s / 60, s % 60)
    }
}

fn history(ui: &mut egui::Ui, height: f32, s: &SplState, cal: f64, limit: f64, calibrated: bool) {
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let p = ui.painter_at(rect);
    p.rect_filled(rect, CornerRadius::same(4), Color32::from_rgb(0x12, 0x12, 0x14));
    let area = rect.shrink2(vec2(30.0, 6.0)).translate(vec2(10.0, 0.0));
    let vals: Vec<f64> = s.history_a.iter().map(|&v| v as f64 + cal).collect();
    // Échelle verticale : jusqu'au seuil + 6 dB (en SPL), ou les valeurs présentes.
    let finite: Vec<f64> = vals.iter().copied().filter(|v| *v > -150.0 + cal).collect();
    let (mut lo, mut hi) = if calibrated { (limit - 40.0, limit + 6.0) } else { (-80.0, 0.0) };
    if let (Some(mn), Some(mx)) = (finite.iter().copied().reduce(f64::min), finite.iter().copied().reduce(f64::max)) {
        lo = lo.min(mn - 3.0);
        hi = hi.max(mx + 3.0);
    }
    let y_of = |v: f64| area.bottom() - ((v - lo) / (hi - lo)) as f32 * area.height();
    let font = FontId::proportional(9.5);
    let step = if hi - lo > 60.0 { 20.0 } else { 10.0 };
    let mut v = (lo / step).ceil() * step;
    while v <= hi {
        let y = y_of(v);
        p.line_segment([pos2(area.left(), y), pos2(area.right(), y)], Stroke::new(1.0, Color32::from_white_alpha(14)));
        p.text(pos2(area.left() - 4.0, y), Align2::RIGHT_CENTER, format!("{v:.0}"), font.clone(), TEXT_DIM);
        v += step;
    }
    if calibrated {
        let y = y_of(limit);
        p.line_segment([pos2(area.left(), y), pos2(area.right(), y)], Stroke::new(1.0, STOP.gamma_multiply(0.8)));
    }
    // 15 minutes sur la largeur, le présent à droite.
    // Une case vaut un peu plus d'une seconde (12 demi-blocs à 48 kHz) : l'axe suit le temps.
    let span = crate::spl::HISTORY_S as f32;
    let per = if s.history_s > 0.0 && !vals.is_empty() { s.history_s as f32 / vals.len() as f32 } else { 1.0 };
    let x_of = |i: usize, len: usize| area.right() - (len - 1 - i) as f32 * per / span * area.width();
    let pts: Vec<egui::Pos2> = vals.iter().enumerate().filter(|(_, v)| **v > -150.0 + cal).map(|(i, &v)| pos2(x_of(i, vals.len()), y_of(v).clamp(area.top(), area.bottom()))).collect();
    if pts.len() >= 2 {
        p.add(Shape::line(pts, Stroke::new(1.5, ACCENT)));
    }
    for (min, label) in [(5.0, "−5 min"), (10.0, "−10 min")] {
        let x = area.right() - min * 60.0 / span * area.width();
        p.line_segment([pos2(x, area.top()), pos2(x, area.bottom())], Stroke::new(1.0, Color32::from_white_alpha(14)));
        p.text(pos2(x, area.bottom() - 2.0), Align2::CENTER_BOTTOM, label, font.clone(), TEXT_DIM);
    }
    resp.on_hover_text(tr("LAeq de chaque seconde sur les 15 dernières minutes ; la ligne rouge est le seuil", "LAeq of each second over the last 15 minutes; the red line is the limit"));
}
