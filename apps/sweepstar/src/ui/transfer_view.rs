//! Vue « Transfert » : fonction de transfert bi-canal en direct, référence (boucle) → micro,
//! à la manière d'un analyseur double canal : magnitude et phase lissées, cohérence, retard
//! de la référence réglable ou trouvé automatiquement. Le calcul tourne dans le fil de mesure
//! tant que la vue est affichée.

use std::collections::HashMap;
use std::sync::Arc;

use egui::{Color32, RichText};
use star_core::i18n::tr;
use star_measure::delay::ms_to_m;
use star_measure::smooth::smooth_power;
use star_measure::transfer::Transfer;
use star_measure::Complex64;
use star_ui::icons::{self, Icon};
use star_ui::theme::*;

use super::plot::{plot, Series, XView, YView};
use super::sweep_view::{smooth_response, SMOOTHINGS};
use crate::controller::Controller;
use crate::traces::TransferSnapshot;

const FFT_SIZES: [usize; 5] = [4096, 8192, 16_384, 32_768, 65_536];
/// Blocs moyennés (0 : moyenne infinie).
const AVERAGES: [u32; 8] = [1, 2, 4, 8, 16, 32, 64, 0];

/// Courbes prêtes à tracer.
#[derive(Default)]
struct Curves {
    mag: Vec<[f64; 2]>,
    phase: Vec<[f64; 2]>,
    /// Cohérence en %.
    coh: Vec<[f64; 2]>,
}

fn curves(h: &[Complex64], coherence: &[f64], bin_hz: f64, smoothing: u32) -> Curves {
    let (db, ph) = smooth_response(h, bin_hz, smoothing);
    let coh = if smoothing > 0 { smooth_power(coherence, bin_hz, smoothing as f64) } else { coherence.to_vec() };
    let keep = |k: &usize| (10.0..=24_000.0).contains(&(*k as f64 * bin_hz));
    let pts = |v: &[f64], scale: f64| (1..v.len()).filter(keep).map(|k| [k as f64 * bin_hz, v[k] * scale]).collect();
    Curves { mag: pts(&db, 1.0), phase: pts(&ph, 1.0), coh: pts(&coh, 100.0) }
}

pub struct TransferView {
    /// Fréquences, partagées par les trois graphes.
    freq: XView,
    mag: YView,
    phase: YView,
    coh: YView,
    serial: u64,
    last: Option<Arc<Transfer>>,
    sample_rate: f64,
    blocks: usize,
    live: Curves,
    /// Lissage avec lequel `live` a été calculé.
    live_smoothing: Option<u32>,
    /// Courbes des mesures enregistrées, par mesure (adresse de l'Arc) et lissage.
    cache: HashMap<(usize, u32), Curves>,
}

impl Default for TransferView {
    fn default() -> Self {
        Self {
            freq: XView::AUDIO,
            mag: YView::new(-40.0, 20.0),
            phase: YView::new(-180.0, 180.0),
            coh: YView::new(0.0, 100.0),
            serial: 0,
            last: None,
            sample_rate: 48_000.0,
            blocks: 0,
            live: Curves::default(),
            live_smoothing: None,
            cache: HashMap::new(),
        }
    }
}

impl TransferView {
    /// Fonction de transfert en cours, pour la figer.
    pub fn snapshot(&self, delay_ms: f64) -> Option<TransferSnapshot> {
        self.last.as_ref().map(|t| TransferSnapshot {
            h: t.h.clone(),
            coherence: t.coherence.iter().map(|&c| c as f32).collect(),
            bin_hz: t.bin_hz,
            sample_rate: self.sample_rate,
            delay_ms,
        })
    }

    pub fn show(&mut self, ui: &mut egui::Ui, ctrl: &mut Controller) {
        let has_ref = ctrl.prefs.audio.ref_channel.is_some();
        ctrl.run_transfer(has_ref);
        let smoothing = ctrl.prefs.tf_smoothing;
        let frame = ctrl.transfer_frame();
        if frame.serial != self.serial || self.live_smoothing != Some(smoothing) {
            self.serial = frame.serial;
            self.blocks = frame.blocks;
            self.sample_rate = frame.sample_rate;
            self.last = frame.data;
            self.live = self.last.as_ref().map(|t| curves(&t.h, &t.coherence, t.bin_hz, smoothing)).unwrap_or_default();
            self.live_smoothing = Some(smoothing);
        }

        // Réglages.
        ui.horizontal(|ui| {
            ui.label(RichText::new(tr("FONCTION DE TRANSFERT", "TRANSFER FUNCTION")).small().strong().color(TEXT_DIM));
            ui.add_space(12.0);
            let fs = ctrl.sample_rate();
            let p = &mut ctrl.prefs;
            ui.label("FFT");
            egui::ComboBox::from_id_salt("tf-fft").selected_text(format!("{}", p.tf_fft)).width(80.0).show_ui(ui, |ui| {
                for n in FFT_SIZES {
                    ui.selectable_value(&mut p.tf_fft, n, format!("{n}  ({:.2} Hz)", fs / n as f64));
                }
            });
            ui.label(tr("Moyenne", "Averaging"));
            let avg_label = |n: u32| if n == 0 { tr("Infinie", "Infinite").to_string() } else { format!("{n}") };
            egui::ComboBox::from_id_salt("tf-moyenne").selected_text(avg_label(p.tf_avg)).width(70.0).show_ui(ui, |ui| {
                for n in AVERAGES {
                    ui.selectable_value(&mut p.tf_avg, n, avg_label(n));
                }
            });
            ui.label(tr("Lissage", "Smoothing"));
            let label = |n: u32| if n == 0 { tr("Aucun", "None").to_string() } else { format!("1/{n} oct.") };
            egui::ComboBox::from_id_salt("tf-lissage").selected_text(label(p.tf_smoothing)).width(90.0).show_ui(ui, |ui| {
                for n in SMOOTHINGS {
                    ui.selectable_value(&mut p.tf_smoothing, n, label(n));
                }
            });
            if ui.button(tr("Remettre à zéro", "Reset")).on_hover_text(tr("Repart d'une moyenne vide", "Starts again from an empty average")).clicked() {
                ctrl.reset_transfer();
            }
        });
        ui.horizontal(|ui| {
            ui.label(tr("Retard de la référence", "Reference delay"));
            let mut ms = ctrl.prefs.tf_delay_ms;
            if ui.add(egui::DragValue::new(&mut ms).range(0.0..=1000.0).speed(0.02).max_decimals(2).suffix(" ms")).on_hover_text(tr("Temps de vol jusqu'au micro : aligne la référence sur le micro", "Time of flight to the mic: aligns the reference with the mic")).changed() {
                ctrl.set_tf_delay_ms(ms);
            }
            ui.label(RichText::new(format!("{:.2} m", ms_to_m(ctrl.prefs.tf_delay_ms, ctrl.prefs.temperature_c))).color(TEXT_DIM));
            let tracking = ctrl.prefs.tf_delay_track;
            if ctrl.finding_delay && !tracking {
                ui.spinner();
            } else if ui
                .add_enabled(has_ref && !tracking, egui::Button::new(tr("Trouver le délai", "Find delay")))
                .on_hover_text(tr("Cherche le retard du micro sur la référence (jouez un bruit rose)", "Finds the mic's delay on the reference (play pink noise)"))
                .clicked()
            {
                ctrl.find_delay();
            }
            ui.add_enabled(has_ref, egui::Checkbox::new(&mut ctrl.prefs.tf_delay_track, tr("Suivi auto", "Auto track"))).on_hover_text(tr(
                "Cherche le délai toutes les secondes et le corrige s'il bouge (micro déplacé, température)",
                "Looks for the delay every second and corrects it when it moves (mic moved, temperature)",
            ));
            if has_ref {
                ui.label(RichText::new(format!("{} {}", self.blocks, tr("blocs", "blocks"))).color(TEXT_DIM));
            }
        });
        if !has_ref {
            ui.add_space(4.0);
            icons::label(
                ui,
                Icon::Warning,
                ACCENT,
                RichText::new(tr("La fonction de transfert compare la boucle de référence au micro : choisissez l'entrée de la boucle dans les Préférences.", "The transfer function compares the reference loop with the mic: choose the loop input in Preferences.")).color(ACCENT),
            );
        } else if self.last.is_none() {
            ui.label(RichText::new(tr("Jouez un bruit rose (ou de la musique) : la référence est la boucle, la mesure est le micro.", "Play pink noise (or music): the reference is the loop, the measurement is the mic.")).color(TEXT_DIM));
        }

        // Mesures de transfert enregistrées et visibles.
        let shown: Vec<(Arc<TransferSnapshot>, Color32, String)> = ctrl.traces.iter().filter(|t| t.visible).filter_map(|t| t.transfer().map(|s| (s.clone(), rgb(t.color), t.name.clone()))).collect();
        let alive: Vec<usize> = shown.iter().map(|(s, ..)| Arc::as_ptr(s) as usize).collect();
        self.cache.retain(|(p, _), _| alive.contains(p));
        for (snap, ..) in &shown {
            self.cache.entry((Arc::as_ptr(snap) as usize, smoothing)).or_insert_with(|| {
                let coh: Vec<f64> = snap.coherence.iter().map(|&c| c as f64).collect();
                curves(&snap.h, &coh, snap.bin_hz, smoothing)
            });
        }
        let live_name = tr("Direct", "Live");
        let series = |pick: fn(&Curves) -> &Vec<[f64; 2]>| -> Vec<Series> {
            let mut v: Vec<Series> = shown
                .iter()
                .filter_map(|(snap, c, n)| self.cache.get(&(Arc::as_ptr(snap) as usize, smoothing)).map(|cv| Series::line(pick(cv), *c, 1.3, n)))
                .collect();
            v.push(Series::line(pick(&self.live), ACCENT, 1.6, live_name));
            v
        };
        let (mag, phase, coh) = (series(|c| &c.mag), series(|c| &c.phase), series(|c| &c.coh));

        let h = ui.available_height();
        ui.label(RichText::new(tr("MAGNITUDE", "MAGNITUDE")).small().strong().color(TEXT_DIM));
        plot(ui, (h * 0.45 - 20.0).max(120.0), &mut self.freq, &mut self.mag, "dB", &mag);
        ui.label(RichText::new(tr("PHASE", "PHASE")).small().strong().color(TEXT_DIM));
        plot(ui, (h * 0.30 - 20.0).max(90.0), &mut self.freq, &mut self.phase, "°", &phase);
        ui.label(RichText::new(tr("COHÉRENCE", "COHERENCE")).small().strong().color(TEXT_DIM));
        let rest = (ui.available_height() - 4.0).max(70.0);
        plot(ui, rest, &mut self.freq, &mut self.coh, "%", &coh);
    }
}
