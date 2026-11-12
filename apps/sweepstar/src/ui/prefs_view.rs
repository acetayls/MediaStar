//! Fenêtre flottante des Préférences : langue, pilote, sortie, entrée, tampon (appliqués en
//! rouvrant les périphériques), canaux du signal, de la boucle de référence et du micro
//! (appliqués tout de suite).

use egui::RichText;
use star_core::i18n::{tr, Lang};
use star_ui::theme::*;

use crate::audio::engine::{host_names, list_inputs, list_outputs, DeviceDesc};
use crate::controller::{AudioPrefs, Controller};

pub fn viewport() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("sweepstar-preferences")
}

pub struct PrefsDraft {
    audio: AudioPrefs,
    hosts: Vec<String>,
    outputs: Vec<DeviceDesc>,
    inputs: Vec<DeviceDesc>,
}

impl PrefsDraft {
    pub fn new(audio: &AudioPrefs) -> Self {
        let mut d = Self { audio: audio.clone(), hosts: host_names(), outputs: Vec::new(), inputs: Vec::new() };
        d.refresh();
        d
    }

    fn refresh(&mut self) {
        self.outputs = list_outputs(self.audio.host.as_deref());
        self.inputs = list_inputs(self.audio.host.as_deref());
    }
}

fn device_combo(ui: &mut egui::Ui, id: &str, value: &mut Option<String>, devices: &[DeviceDesc]) {
    let label = value.clone().unwrap_or_else(|| tr("Par défaut", "Default").into());
    egui::ComboBox::from_id_salt(id).selected_text(label).width(300.0).show_ui(ui, |ui| {
        if ui.selectable_label(value.is_none(), tr("Par défaut", "Default")).clicked() {
            *value = None;
        }
        for d in devices {
            let text = format!("{}{} ({} {})", d.name, if d.is_default { " ★" } else { "" }, d.channels, tr("canaux", "channels"));
            if ui.selectable_label(value.as_deref() == Some(d.name.as_str()), text).clicked() {
                *value = Some(d.name.clone());
            }
        }
    });
}

/// Affiche la fenêtre ; rend `false` quand elle est fermée.
pub fn show(ctx: &egui::Context, draft: &mut PrefsDraft, ctrl: &mut Controller) -> bool {
    let mut open = true;
    let vp = egui::ViewportBuilder::default()
        .with_title(format!("SweepStar — {}", tr("Préférences", "Preferences")))
        .with_inner_size([560.0, 520.0])
        .with_min_inner_size([440.0, 320.0]);
    let vp = star_ui::floating::placed(ctx, "preferences", vp);
    ctx.show_viewport_immediate(viewport(), vp, |ui, _| {
        star_ui::floating::track(ui.ctx(), "preferences");
        if ui.ctx().input(|i| i.viewport().close_requested()) {
            open = false;
        }
        if ui.ctx().input(|i| i.key_pressed(egui::Key::Escape)) {
            ctrl.stop_all();
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(12)).show(ui, |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                ui.heading(tr("Langue", "Language"));
                ui.horizontal(|ui| {
                    for (l, name) in [(Lang::Fr, "Français"), (Lang::En, "English")] {
                        if ui.selectable_label(ctrl.prefs.lang == l, name).clicked() {
                            ctrl.prefs.lang = l;
                            star_core::i18n::set_lang(l);
                        }
                    }
                });
                ui.add_space(10.0);
                ui.heading(tr("Périphériques", "Devices"));
                let mut refresh = false;
                egui::Grid::new("audio").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label(tr("Pilote", "Driver"));
                    let host_label = draft.audio.host.clone().unwrap_or_else(|| tr("Par défaut", "Default").into());
                    egui::ComboBox::from_id_salt("host").selected_text(host_label).width(300.0).show_ui(ui, |ui| {
                        if ui.selectable_label(draft.audio.host.is_none(), tr("Par défaut", "Default")).clicked() {
                            draft.audio.host = None;
                            refresh = true;
                        }
                        for h in &draft.hosts {
                            if ui.selectable_label(draft.audio.host.as_deref() == Some(h), h).clicked() {
                                draft.audio.host = Some(h.clone());
                                refresh = true;
                            }
                        }
                    });
                    ui.end_row();
                    ui.label(tr("Sortie", "Output"));
                    device_combo(ui, "sortie", &mut draft.audio.output, &draft.outputs);
                    ui.end_row();
                    ui.label(tr("Entrée", "Input"));
                    device_combo(ui, "entree", &mut draft.audio.input, &draft.inputs);
                    ui.end_row();
                    ui.label(tr("Tampon", "Buffer"));
                    let label = draft.audio.buffer_size.map_or_else(|| tr("Pilote", "Driver").to_string(), |n| format!("{n}"));
                    egui::ComboBox::from_id_salt("tampon").selected_text(label).show_ui(ui, |ui| {
                        ui.selectable_value(&mut draft.audio.buffer_size, None, tr("Pilote", "Driver"));
                        for n in [64, 128, 256, 512, 1024, 2048] {
                            ui.selectable_value(&mut draft.audio.buffer_size, Some(n), n.to_string());
                        }
                    });
                    ui.end_row();
                });
                if refresh {
                    draft.audio.output = None;
                    draft.audio.input = None;
                    draft.refresh();
                }
                ui.label(RichText::new(tr(
                    "Choisissez la même carte en entrée et en sortie : elles partagent alors la même horloge.",
                    "Pick the same interface for input and output: they then share the same clock.",
                ))
                .small()
                .color(TEXT_DIM));
                let changed = draft.audio.host != ctrl.prefs.audio.host
                    || draft.audio.output != ctrl.prefs.audio.output
                    || draft.audio.input != ctrl.prefs.audio.input
                    || draft.audio.buffer_size != ctrl.prefs.audio.buffer_size;
                if ui.add_enabled(changed, egui::Button::new(tr("Appliquer", "Apply"))).clicked() {
                    let a = &mut ctrl.prefs.audio;
                    a.host = draft.audio.host.clone();
                    a.output = draft.audio.output.clone();
                    a.input = draft.audio.input.clone();
                    a.buffer_size = draft.audio.buffer_size;
                    ctrl.open_device();
                    ctrl.save_prefs();
                }

                ui.add_space(10.0);
                ui.heading("Sweep");
                egui::Grid::new("sweep-reglages").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    let p = &mut ctrl.prefs;
                    ui.label(tr("Durée", "Length"));
                    ui.add(egui::DragValue::new(&mut p.sweep_secs).range(1.0..=16.0).speed(0.05).max_decimals(1).suffix(" s"))
                        .on_hover_text(tr("Plus long : meilleur rapport signal/bruit", "Longer: better signal-to-noise ratio"));
                    ui.end_row();
                    ui.label(tr("De", "From"));
                    ui.add(egui::DragValue::new(&mut p.f1).range(5.0..=1000.0).speed(1.0).suffix(" Hz"));
                    ui.end_row();
                    ui.label(tr("À", "To"));
                    ui.add(egui::DragValue::new(&mut p.f2).range(1000.0..=24_000.0).speed(50.0).suffix(" Hz"));
                    ui.end_row();
                    ui.label(tr("Traîne", "Tail"));
                    ui.add(egui::DragValue::new(&mut p.tail_secs).range(0.1..=10.0).speed(0.05).max_decimals(1).suffix(" s"))
                        .on_hover_text(tr("Silence enregistré après le sweep, pour la réverbération de la salle", "Silence recorded after the sweep, for the room's reverberation"));
                    ui.end_row();
                });

                ui.add_space(10.0);
                ui.heading(tr("Canaux", "Channels"));
                let outs = ctrl.device.as_ref().map_or(2, |d| d.out_channels).max(1);
                let ins = ctrl.device.as_ref().map_or(2, |d| d.in_channels).max(1);
                let mut chan_changed = false;
                ui.label(tr("Sorties du signal", "Signal outputs"));
                ui.horizontal_wrapped(|ui| {
                    for c in 0..outs.min(64) {
                        let mut on = ctrl.prefs.audio.out_channels.contains(&c);
                        if ui.toggle_value(&mut on, format!("{}", c + 1)).changed() {
                            if on {
                                ctrl.prefs.audio.out_channels.push(c);
                                ctrl.prefs.audio.out_channels.sort_unstable();
                            } else {
                                ctrl.prefs.audio.out_channels.retain(|&x| x != c);
                            }
                            chan_changed = true;
                        }
                    }
                });
                egui::Grid::new("canaux").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                    ui.label(tr("Micro", "Microphone"));
                    let mic = ctrl.prefs.audio.mic_channel;
                    egui::ComboBox::from_id_salt("micro").selected_text(format!("in {}", mic + 1)).show_ui(ui, |ui| {
                        for c in 0..ins.min(32) {
                            chan_changed |= ui.selectable_value(&mut ctrl.prefs.audio.mic_channel, c, format!("in {}", c + 1)).changed();
                        }
                    });
                    ui.end_row();
                    ui.label(tr("Boucle de référence", "Reference loop"));
                    let r = ctrl.prefs.audio.ref_channel;
                    let label = r.map_or_else(|| tr("Aucune", "None").to_string(), |c| format!("in {}", c + 1));
                    egui::ComboBox::from_id_salt("boucle").selected_text(label).show_ui(ui, |ui| {
                        chan_changed |= ui.selectable_value(&mut ctrl.prefs.audio.ref_channel, None, tr("Aucune", "None")).changed();
                        for c in 0..ins.min(32) {
                            chan_changed |= ui.selectable_value(&mut ctrl.prefs.audio.ref_channel, Some(c), format!("in {}", c + 1)).changed();
                        }
                    });
                    ui.end_row();
                });
                ui.label(RichText::new(tr(
                    "La boucle de référence relie une sortie de la carte à une de ses entrées : la mesure s'en sert pour annuler la latence et la couleur de la carte. Sans boucle, le délai mesuré comprend la latence.",
                    "The reference loop connects an output of the interface to one of its inputs: the measurement uses it to cancel the interface's latency and colour. Without a loop, the measured delay includes the latency.",
                ))
                .small()
                .color(TEXT_DIM));
                if chan_changed {
                    ctrl.apply_channels();
                    ctrl.save_prefs();
                }
            });
        });
    });
    open
}
