//! Vue « Sweep et IR » : magnitude lissée, phase (délai retiré), réponse impulsionnelle
//! (ETC en dB ou linéaire), délai en ms et en mètres.

use std::collections::HashMap;
use std::sync::Arc;

use egui::{Color32, RichText};
use star_core::i18n::tr;
use star_measure::delay::ms_to_m;
use star_measure::smooth::smooth_power;
use star_measure::Complex64;
use star_ui::icons::{self, Icon};
use super::plot::{plot, Series, XAxis, XView, YView};
use star_ui::theme::*;

use crate::controller::Controller;
use crate::measure::{Measurement, Method};

/// Lissages proposés (1/n d'octave, 0 : aucun).
pub const SMOOTHINGS: [u32; 7] = [0, 1, 3, 6, 12, 24, 48];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IrMode {
    Etc,
    Linear,
}

/// Courbes prêtes à tracer d'une mesure, pour un lissage donné.
struct Curves {
    mag: Vec<[f64; 2]>,
    phase: Vec<[f64; 2]>,
    ir: Vec<[f64; 2]>,
    etc: Vec<[f64; 2]>,
}

pub struct SweepView {
    /// Fréquences, partagées par la magnitude et la phase.
    freq: XView,
    /// Temps de la réponse impulsionnelle.
    ir_x: XView,
    mag: YView,
    phase: YView,
    ir_db: YView,
    ir_lin: YView,
    ir_mode: IrMode,
    /// Fenêtre de temps de l'IR (ms après t = 0).
    ir_span_ms: f64,
    /// Courbes calculées, par mesure (adresse de l'Arc) et lissage.
    cache: HashMap<(usize, u32), Curves>,
}

impl Default for SweepView {
    fn default() -> Self {
        Self {
            freq: XView::AUDIO,
            ir_x: XView::new(XAxis::Linear { min: -5.0, max: 200.0, unit: "ms" }),
            mag: YView::new(-40.0, 20.0),
            phase: YView::new(-180.0, 180.0),
            ir_db: YView::new(-80.0, 0.0),
            ir_lin: YView::new(-1.0, 1.0),
            ir_mode: IrMode::Etc,
            ir_span_ms: 200.0,
            cache: HashMap::new(),
        }
    }
}

/// Magnitude (dB) et phase (°) lissées, case par case.
fn smoothed(m: &Measurement, smoothing: u32) -> (Vec<f64>, Vec<f64>) {
    smooth_response(&m.h, m.bin_hz, smoothing)
}

/// Magnitude (dB) et phase (°) d'une réponse complexe, lissées en 1/`smoothing` d'octave.
pub fn smooth_response(h: &[Complex64], bin_hz: f64, smoothing: u32) -> (Vec<f64>, Vec<f64>) {
    let power: Vec<f64> = h.iter().map(|c| c.norm_sqr()).collect();
    if smoothing == 0 {
        return (power.iter().map(|&p| star_measure::db10(p)).collect(), h.iter().map(|c| c.arg().to_degrees()).collect());
    }
    let frac = smoothing as f64;
    let p = smooth_power(&power, bin_hz, frac);
    // Phase : lissage de la réponse complexe (parties réelle et imaginaire).
    let re = smooth_power(&h.iter().map(|c| c.re).collect::<Vec<_>>(), bin_hz, frac);
    let im = smooth_power(&h.iter().map(|c| c.im).collect::<Vec<_>>(), bin_hz, frac);
    let ph = re.iter().zip(&im).map(|(&r, &i)| Complex64::new(r, i).arg().to_degrees()).collect();
    (p.iter().map(|&v| star_measure::db10(v)).collect(), ph)
}

fn curves(m: &Measurement, smoothing: u32) -> Curves {
    let (db, ph) = smoothed(m, smoothing);
    let in_band = |k: usize| {
        let f = k as f64 * m.bin_hz;
        (10.0..=24_000.0).contains(&f)
    };
    let mag = (1..db.len()).filter(|&k| in_band(k)).map(|k| [k as f64 * m.bin_hz, db[k]]).collect();
    let phase = (1..ph.len()).filter(|&k| in_band(k)).map(|k| [k as f64 * m.bin_hz, ph[k]]).collect();
    let ir = m.ir.iter().enumerate().map(|(i, &v)| [m.ir_time_ms(i), v as f64]).collect();
    let etc = m.etc.iter().enumerate().map(|(i, &v)| [m.ir_time_ms(i), v as f64]).collect();
    Curves { mag, phase, ir, etc }
}

/// Points de l'export REW : 1/48 d'octave de 10 Hz à la fréquence de Nyquist, lissage courant.
pub fn rew_points(m: &Measurement, smoothing: u32) -> Vec<(f64, f64, f64)> {
    let (db, ph) = smoothed(m, smoothing);
    let nyq = m.sample_rate / 2.0;
    let mut out = Vec::new();
    let mut f = 10.0;
    while f < nyq {
        let k = ((f / m.bin_hz).round() as usize).min(db.len() - 1);
        out.push((f, db[k], ph[k]));
        f *= 2f64.powf(1.0 / 48.0);
    }
    out
}

impl SweepView {
    pub fn show(&mut self, ui: &mut egui::Ui, ctrl: &mut Controller) {
        // Mesures à tracer : traces visibles, puis la dernière mesure (en blanc, devant).
        let mut shown: Vec<(Arc<Measurement>, Color32, String)> =
            ctrl.traces.iter().filter(|t| t.visible).filter_map(|t| t.measurement().map(|m| (m.clone(), rgb(t.color), t.name.clone()))).collect();
        if let Some(m) = &ctrl.current {
            shown.push((m.clone(), Color32::WHITE, tr("Dernière", "Last").to_string()));
        }
        let smoothing = ctrl.prefs.smoothing;
        // Le cache ne garde que les mesures encore affichées.
        let alive: Vec<usize> = shown.iter().map(|(m, ..)| Arc::as_ptr(m) as usize).collect();
        self.cache.retain(|(p, _), _| alive.contains(p));
        for (m, ..) in &shown {
            let key = (Arc::as_ptr(m) as usize, smoothing);
            self.cache.entry(key).or_insert_with(|| curves(m, smoothing));
        }

        // Ligne d'informations et réglages d'affichage.
        ui.horizontal(|ui| {
            match &ctrl.current.as_ref().or(ctrl.traces.iter().rev().filter(|t| t.visible).find_map(|t| t.measurement())) {
                Some(m) => {
                    let metres = ms_to_m(m.delay_ms, ctrl.prefs.temperature_c);
                    ui.label(RichText::new(format!("{} {:.2} ms  ·  {:.2} m", tr("Délai", "Delay"), m.delay_ms, metres)).size(16.0).strong().color(ACCENT));
                    match m.method {
                        Method::Reference => {
                            ui.label(RichText::new(tr("par la boucle de référence", "from the reference loop")).color(TEXT_DIM));
                        }
                        Method::InverseFilter => {
                            icons::label(ui, Icon::Warning, ACCENT, RichText::new(tr("sans boucle : le délai comprend la latence de la carte son", "no loop: the delay includes the sound card's latency")).color(ACCENT));
                        }
                    }
                    ui.label(RichText::new(format!("{} {:.1} dBFS", tr("micro", "mic"), m.mic_peak_db)).color(if m.mic_peak_db > -1.0 { STOP } else { TEXT_DIM }));
                    if let Some(r) = m.ref_peak_db {
                        ui.label(RichText::new(format!("{} {:.1} dBFS", tr("boucle", "loop"), r)).color(if r > -1.0 { STOP } else { TEXT_DIM }));
                    }
                }
                None => {
                    ui.label(RichText::new(tr("Réglez le niveau avec le bruit rose, puis cliquez sur MESURER.", "Set the level with pink noise, then click MEASURE.")).color(TEXT_DIM));
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add(egui::DragValue::new(&mut ctrl.prefs.temperature_c).range(-30.0..=50.0).speed(0.2).max_decimals(1).suffix(" °C"))
                    .on_hover_text(tr("Température de l'air, pour convertir le délai en mètres", "Air temperature, to convert the delay to metres"));
                let label = |n: u32| if n == 0 { tr("Sans lissage", "No smoothing").to_string() } else { format!("1/{n} oct.") };
                egui::ComboBox::from_id_salt("lissage").selected_text(label(smoothing)).width(110.0).show_ui(ui, |ui| {
                    for n in SMOOTHINGS {
                        ui.selectable_value(&mut ctrl.prefs.smoothing, n, label(n));
                    }
                });
            });
        });

        // Délai de la mesure mise en avant, pour le zoom sur le pic de l'IR.
        let peak_ms = ctrl.current.as_ref().or(ctrl.traces.iter().rev().filter(|t| t.visible).find_map(|t| t.measurement())).map(|m| m.delay_ms);
        let h = ui.available_height();
        let Self { freq, ir_x, mag, phase, ir_db, ir_lin, ir_mode, ir_span_ms, cache } = self;
        let series = |pick: fn(&Curves) -> &Vec<[f64; 2]>, width: f32| -> Vec<Series> {
            shown
                .iter()
                .filter_map(|(m, c, n)| cache.get(&(Arc::as_ptr(m) as usize, smoothing)).map(|cv| Series::line(pick(cv), *c, width, n)))
                .collect()
        };

        ui.label(RichText::new(tr("MAGNITUDE", "MAGNITUDE")).small().strong().color(TEXT_DIM));
        plot(ui, (h * 0.42 - 20.0).max(120.0), freq, mag, "dB", &series(|c| &c.mag, 1.5));

        ui.label(RichText::new(tr("PHASE (délai retiré)", "PHASE (delay removed)")).small().strong().color(TEXT_DIM));
        plot(ui, (h * 0.24 - 20.0).max(90.0), freq, phase, "°", &series(|c| &c.phase, 1.2));

        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("RÉPONSE IMPULSIONNELLE", "IMPULSE RESPONSE")).small().strong().color(TEXT_DIM));
            ui.selectable_value(ir_mode, IrMode::Etc, tr("ETC (dB)", "ETC (dB)"));
            ui.selectable_value(ir_mode, IrMode::Linear, tr("Linéaire", "Linear"));
            ui.add(egui::DragValue::new(ir_span_ms).range(5.0..=1000.0).speed(1.0).prefix(tr("jusqu'à ", "up to ")).suffix(" ms"));
            if ui
                .add_enabled(peak_ms.is_some(), egui::Button::new(tr("Zoom sur le pic", "Zoom on peak")))
                .on_hover_text(tr("Montre 1 ms avant et 4 ms après le pic (double-clic sur le graphe pour revenir)", "Shows 1 ms before and 4 ms after the peak (double-click the graph to go back)"))
                .clicked()
            {
                if let Some(d) = peak_ms {
                    ir_x.show_range(d - 1.0, d + 4.0);
                }
            }
        });
        ir_x.set_default(XAxis::Linear { min: -5.0, max: *ir_span_ms, unit: "ms" });
        let (s, view, unit) = match ir_mode {
            IrMode::Etc => (series(|c| &c.etc, 1.0), ir_db, "dB"),
            IrMode::Linear => (series(|c| &c.ir, 1.0), ir_lin, ""),
        };
        let rest = (ui.available_height() - 4.0).max(100.0);
        plot(ui, rest, ir_x, view, unit, &s);
    }
}
