//! Vue RTA : niveau du micro par bandes fractionnaires d'octave (courbe ou barres),
//! pondération A/C/Z, moyenne exponentielle réglable, maintien des crêtes ; en dB SPL une
//! fois le micro calibré (calibreur à 1 kHz).

use egui::{Color32, RichText};
use star_core::i18n::tr;
use star_measure::rta::{band_powers, fractional_octave_bands, Band, Weighting};
use star_measure::smooth::smooth_power;
use super::plot::{plot, Series, XView, YView};
use star_ui::theme::*;

use crate::controller::{Controller, RtaWeighting};
use crate::traces::RtaSnapshot;

const FRACTIONS: [u32; 6] = [1, 3, 6, 12, 24, 48];
/// Lissages proposés (1/n d'octave, 0 : aucun).
const SMOOTHINGS: [u32; 6] = [0, 24, 12, 6, 3, 1];
/// Pas des points de la courbe continue (1/n d'octave).
const CURVE_STEP: f64 = 48.0;

/// Réglages d'affichage d'un spectre de RTA.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RtaLook {
    pub fraction: u32,
    pub smoothing: u32,
    pub weighting: RtaWeighting,
    pub curve: bool,
    /// Calibration du micro : dB SPL = dBFS + cette valeur.
    pub cal: Option<f64>,
}

impl RtaLook {
    pub fn of(ctrl: &Controller) -> Self {
        let p = &ctrl.prefs;
        Self { fraction: p.rta_fraction, smoothing: p.rta_smoothing, weighting: p.rta_weighting, curve: p.rta_curve, cal: p.mic_cal_db }
    }
}

/// Bandes affichées : bandes normalisées (barres), ou bande glissante de même largeur
/// centrée tous les 1/48 d'octave (courbe).
fn display_bands(look: RtaLook) -> Vec<Band> {
    if !look.curve {
        return fractional_octave_bands(look.fraction, 20.0, 20_000.0);
    }
    let half = 2f64.powf(0.5 / look.fraction.max(1) as f64);
    let n = (CURVE_STEP * (20_000.0f64 / 20.0).log2()).ceil() as i32;
    (0..=n).map(|i| 20.0 * 2f64.powf(i as f64 / CURVE_STEP)).map(|c| Band { center: c, low: c / half, high: c * half }).collect()
}

/// Points à tracer (Hz, dB) d'un spectre de puissance par case.
pub fn rta_points(power: &[f64], bin_hz: f64, look: RtaLook) -> Vec<[f64; 2]> {
    if power.is_empty() || bin_hz <= 0.0 {
        return Vec::new();
    }
    let bands = display_bands(look);
    let w = weighting(look.weighting);
    let smoothed;
    let power = if look.smoothing > 0 {
        smoothed = smooth_power(power, bin_hz, look.smoothing as f64);
        &smoothed[..]
    } else {
        power
    };
    let offset = look.cal.unwrap_or(0.0);
    let levels = band_powers(power, bin_hz, &bands).into_iter().zip(&bands).map(|(p, b)| star_measure::db10(p) + w.gain_db(b.center) + offset);
    if look.curve {
        bands.iter().zip(levels).map(|(b, l)| [b.center, l]).collect()
    } else {
        // Barres en escalier : chaque bande de son bord bas à son bord haut.
        bands.iter().zip(levels).flat_map(|(b, l)| [[b.low, l], [b.high * 0.9999, l]]).collect()
    }
}

pub fn unit(w: RtaWeighting, calibrated: bool) -> &'static str {
    match (w, calibrated) {
        (RtaWeighting::A, false) => "dBFS(A)",
        (RtaWeighting::C, false) => "dBFS(C)",
        (RtaWeighting::Z, false) => "dBFS",
        (RtaWeighting::A, true) => "dB(A)",
        (RtaWeighting::C, true) => "dB(C)",
        (RtaWeighting::Z, true) => "dB SPL",
    }
}

/// Niveau global (dB, pondéré) de 20 Hz à 20 kHz.
fn total_level(power: &[f64], bin_hz: f64, w: Weighting) -> f64 {
    let sum: f64 = power.iter().enumerate().filter(|(k, _)| (20.0..=20_000.0).contains(&(*k as f64 * bin_hz))).map(|(k, &p)| p * w.gain(k as f64 * bin_hz)).sum();
    star_measure::db10(sum)
}

/// Niveau (dBFS) de la bande d'octave de 1 kHz : celui d'un calibreur.
fn level_1k(power: &[f64], bin_hz: f64) -> f64 {
    let b = Band { center: 1000.0, low: 1000.0 / 2f64.sqrt(), high: 1000.0 * 2f64.sqrt() };
    star_measure::db10(band_powers(power, bin_hz, &[b])[0])
}

pub struct RtaView {
    avg: Vec<f64>,
    bin_hz: f64,
    serial: u64,
    hold: Vec<[f64; 2]>,
    /// Réglages avec lesquels `hold` a été calculé.
    look: Option<RtaLook>,
    view: YView,
    freq: XView,
    /// La vue est réglée pour des dB SPL.
    view_spl: bool,
    /// Fenêtre de calibration ouverte, niveau du calibreur (dB SPL).
    cal_open: bool,
    cal_ref: f64,
}

impl Default for RtaView {
    fn default() -> Self {
        Self { avg: Vec::new(), bin_hz: 0.0, serial: 0, hold: Vec::new(), look: None, view: YView::new(-100.0, 0.0), freq: XView::AUDIO, view_spl: false, cal_open: false, cal_ref: 94.0 }
    }
}

fn weighting(w: RtaWeighting) -> Weighting {
    match w {
        RtaWeighting::A => Weighting::A,
        RtaWeighting::C => Weighting::C,
        RtaWeighting::Z => Weighting::Z,
    }
}

impl RtaView {
    /// Spectre moyenné en cours, pour le figer.
    pub fn snapshot(&self) -> Option<RtaSnapshot> {
        (!self.avg.is_empty()).then(|| RtaSnapshot { power: self.avg.clone(), bin_hz: self.bin_hz })
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ctrl: &mut Controller) {
        // Nouveau spectre du fil de mesure : moyenne exponentielle.
        let frame = ctrl.rta_frame();
        if frame.serial != self.serial && !frame.power.is_empty() {
            self.serial = frame.serial;
            if self.avg.len() != frame.power.len() || self.bin_hz != frame.bin_hz {
                self.avg = frame.power.clone();
                self.bin_hz = frame.bin_hz;
            } else {
                let a = ctrl.prefs.rta_speed;
                for (g, &p) in self.avg.iter_mut().zip(&frame.power) {
                    *g += a * (p - *g);
                }
            }
        }

        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("RTA DU MICRO", "MICROPHONE RTA")).small().strong().color(TEXT_DIM));
            ui.add_space(12.0);
            let p = &mut ctrl.prefs;
            egui::ComboBox::from_id_salt("rta-bandes").selected_text(format!("1/{} oct.", p.rta_fraction)).width(90.0).show_ui(ui, |ui| {
                for n in FRACTIONS {
                    ui.selectable_value(&mut p.rta_fraction, n, format!("1/{n} oct."));
                }
            });
            for (w, l) in [(RtaWeighting::Z, "Z"), (RtaWeighting::A, "A"), (RtaWeighting::C, "C")] {
                ui.selectable_value(&mut p.rta_weighting, w, l);
            }
            ui.add_space(8.0);
            ui.label(tr("Moyenne", "Averaging"));
            for (v, l) in [(1.0, tr("Rapide", "Fast")), (0.25, tr("Moyenne", "Medium")), (0.05, tr("Lente", "Slow"))] {
                if ui.selectable_label((p.rta_speed - v).abs() < 1e-9, l).clicked() {
                    p.rta_speed = v;
                }
            }
            if ui.button(tr("Effacer les crêtes", "Clear peaks")).clicked() {
                self.hold.clear();
            }
            let cal_label = if p.mic_cal_db.is_some() { tr("Calibré", "Calibrated") } else { tr("Calibrer…", "Calibrate…") };
            if ui
                .selectable_label(self.cal_open, cal_label)
                .on_hover_text(tr("Calibrer le micro avec un calibreur, pour lire des dB SPL", "Calibrate the mic with a calibrator, to read dB SPL"))
                .clicked()
            {
                self.cal_open = !self.cal_open;
            }
        });
        ui.horizontal(|ui| {
            let p = &mut ctrl.prefs;
            ui.label(tr("Affichage", "Display"));
            ui.selectable_value(&mut p.rta_curve, true, tr("Courbe", "Curve")).on_hover_text(tr(
                "Courbe continue : bande glissante de la largeur choisie, un point tous les 1/48 d'octave",
                "Continuous curve: sliding band of the chosen width, one point every 1/48 octave",
            ));
            ui.selectable_value(&mut p.rta_curve, false, tr("Barres", "Bars"));
            ui.add_space(8.0);
            ui.label(tr("Lissage", "Smoothing"));
            let label = |n: u32| if n == 0 { tr("Aucun", "None").to_string() } else { format!("1/{n} oct.") };
            egui::ComboBox::from_id_salt("rta-lissage").selected_text(label(p.rta_smoothing)).width(90.0).show_ui(ui, |ui| {
                for n in SMOOTHINGS {
                    ui.selectable_value(&mut p.rta_smoothing, n, label(n));
                }
            });
        });

        // Niveau global, en grand.
        let w = weighting(ctrl.prefs.rta_weighting);
        let cal = ctrl.prefs.mic_cal_db;
        if !self.avg.is_empty() {
            let l = total_level(&self.avg, self.bin_hz, w) + cal.unwrap_or(0.0);
            let letter = match ctrl.prefs.rta_weighting {
                RtaWeighting::A => "LA",
                RtaWeighting::C => "LC",
                RtaWeighting::Z => "LZ",
            };
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{letter} {l:.1} {}", unit(ctrl.prefs.rta_weighting, cal.is_some()))).size(20.0).strong().color(ACCENT));
                if cal.is_none() {
                    ui.label(RichText::new(tr("micro non calibré : niveaux en dBFS", "mic not calibrated: levels in dBFS")).color(TEXT_DIM));
                }
            });
        }
        if cal.is_some() != self.view_spl {
            self.view_spl = cal.is_some();
            self.view = if self.view_spl { YView::new(20.0, 120.0) } else { YView::new(-100.0, 0.0) };
        }
        if self.cal_open {
            self.calibration_window(ui.ctx(), ctrl);
        }

        let look = RtaLook::of(ctrl);
        let levels = rta_points(&self.avg, self.bin_hz, look);
        if self.hold.len() != levels.len() || self.look != Some(look) {
            self.hold = levels.clone();
            self.look = Some(look);
        }
        for (h, l) in self.hold.iter_mut().zip(&levels) {
            h[1] = h[1].max(l[1]);
        }
        // Traces RTA enregistrées et visibles, derrière la mesure en direct.
        let snaps: Vec<(Vec<[f64; 2]>, Color32, String)> = ctrl
            .traces
            .iter()
            .filter(|t| t.visible)
            .filter_map(|t| t.rta().map(|r| (rta_points(&r.power, r.bin_hz, look), rgb(t.color), t.name.clone())))
            .collect();
        // Barres pleines pour le direct ; crêtes et traces en lignes par-dessus.
        let mut s: Vec<Series> = vec![Series { bars: !look.curve, ..Series::line(&levels, ACCENT, 2.0, tr("Niveau", "Level")) }];
        s.extend(snaps.iter().map(|(p, c, n)| Series::line(p, *c, 1.5, n)));
        s.push(Series::line(&self.hold, Color32::from_white_alpha(70), 1.0, tr("Crêtes", "Peaks")));
        if look.curve {
            // En courbe, le direct passe devant.
            s.rotate_left(1);
        }
        let h = (ui.available_height() - 4.0).max(150.0);
        plot(ui, h, &mut self.freq, &mut self.view, unit(ctrl.prefs.rta_weighting, cal.is_some()), &s);
    }

    /// Fenêtre de calibration : le calibreur posé sur le micro donne la correspondance
    /// dBFS → dB SPL, lue sur la bande d'octave de 1 kHz.
    fn calibration_window(&mut self, ctx: &egui::Context, ctrl: &mut Controller) {
        let mut open = true;
        egui::Window::new(tr("Calibration du micro", "Microphone calibration")).open(&mut open).resizable(false).collapsible(false).default_width(340.0).show(ctx, |ui| {
            ui.label(tr(
                "Posez le calibreur (1 kHz) sur le micro, allumez-le, puis cliquez sur Calibrer. La moyenne Lente donne une lecture plus stable.",
                "Fit the calibrator (1 kHz) on the mic, switch it on, then click Calibrate. Slow averaging gives a steadier reading.",
            ));
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(tr("Niveau du calibreur", "Calibrator level"));
                for v in [94.0, 114.0] {
                    ui.selectable_value(&mut self.cal_ref, v, format!("{v:.0}"));
                }
                ui.add(egui::DragValue::new(&mut self.cal_ref).range(60.0..=140.0).speed(0.1).max_decimals(1).suffix(" dB SPL"));
            });
            let reading = (!self.avg.is_empty()).then(|| (level_1k(&self.avg, self.bin_hz), total_level(&self.avg, self.bin_hz, Weighting::Z)));
            match reading {
                Some((band, total)) => {
                    ui.label(RichText::new(format!("{} {band:.1} dBFS  ·  {} {total:.1} dBFS", tr("Octave de 1 kHz :", "1 kHz octave:"), tr("global :", "overall:"))).monospace());
                    let pure = total - band < 1.0;
                    if !pure {
                        icons_warning(ui, tr("Le signal n'est pas un 1 kHz pur : le calibreur est-il bien en place ?", "The signal is not a pure 1 kHz: is the calibrator fitted?"));
                    }
                    if ui.add_enabled(band > -100.0, egui::Button::new(RichText::new(tr("Calibrer", "Calibrate")).strong())).clicked() {
                        ctrl.prefs.mic_cal_db = Some(self.cal_ref - band);
                        ctrl.save_prefs();
                    }
                }
                None => {
                    ui.label(RichText::new(tr("En attente du signal du micro…", "Waiting for the mic signal…")).color(TEXT_DIM));
                }
            }
            ui.separator();
            match ctrl.prefs.mic_cal_db {
                Some(mut c) => {
                    ui.horizontal(|ui| {
                        ui.label(tr("0 dBFS =", "0 dBFS ="));
                        if ui.add(egui::DragValue::new(&mut c).range(60.0..=200.0).speed(0.1).max_decimals(1).suffix(" dB SPL")).on_hover_text(tr("Réglage à la main de la calibration", "Manual calibration adjustment")).changed() {
                            ctrl.prefs.mic_cal_db = Some(c);
                        }
                        if ui.button(tr("Retirer", "Remove")).clicked() {
                            ctrl.prefs.mic_cal_db = None;
                            ctrl.save_prefs();
                        }
                    });
                    ui.label(RichText::new(tr("Valable pour ce micro, ce canal et ce gain de préampli.", "Valid for this mic, this channel and this preamp gain.")).small().color(TEXT_DIM));
                }
                None => {
                    ui.label(RichText::new(tr("Micro non calibré.", "Mic not calibrated.")).color(TEXT_DIM));
                }
            }
        });
        if !open {
            self.cal_open = false;
            ctrl.save_prefs();
        }
    }
}

fn icons_warning(ui: &mut egui::Ui, text: &str) {
    star_ui::icons::label(ui, star_ui::icons::Icon::Warning, ACCENT, RichText::new(text).color(ACCENT));
}
