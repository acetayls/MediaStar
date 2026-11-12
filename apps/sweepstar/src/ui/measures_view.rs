//! Panneau « Mesures », à la manière d'un analyseur : capturer la mesure ou le RTA en cours,
//! nommer, colorer, afficher ou masquer, superposer, supprimer ; chaque mesure est un
//! fichier du dossier des mesures, relu au lancement.

use egui::{vec2, Align, Color32, CornerRadius, Layout, RichText, Sense};
use star_core::i18n::tr;
use star_ui::icons::{self, Icon};
use star_ui::theme::*;

use super::rta_view::RtaView;
use super::transfer_view::TransferView;
use super::Tab;
use crate::controller::{Controller, TRACE_COLORS};
use crate::traces::TraceKind;

/// Actions qui demandent une boîte de dialogue à l'appli.
pub enum MeasuresAction {
    /// Export de l'IR en WAV : mesure `Some(i)` ou dernière mesure.
    Wav(Option<usize>),
    Rew(Option<usize>),
    Import,
    ChooseDir,
    Collapse,
}

pub fn show(ui: &mut egui::Ui, ctrl: &mut Controller, tab: Tab, rta: &RtaView, transfer: &TransferView) -> Option<MeasuresAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new(tr("MESURES", "MEASUREMENTS")).strong().color(ACCENT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.small_button("»").on_hover_text(tr("Replier le panneau", "Collapse the panel")).clicked() {
                action = Some(MeasuresAction::Collapse);
            }
        });
    });

    // Capture selon la vue.
    let kind = match tab {
        Tab::Sweep => TraceKind::Sweep,
        Tab::Rta => TraceKind::Rta,
        Tab::Transfer => TraceKind::Transfer,
    };
    // Mesure par sweep : le bouton MESURER est ici, au-dessus de la capture.
    if kind == TraceKind::Sweep {
        match ctrl.measuring {
            Some(p) => {
                ui.add(egui::ProgressBar::new(p).text(tr("Mesure… Échap pour arrêter", "Measuring… Esc to stop")));
            }
            None => {
                let go = egui::Button::new(RichText::new(tr("MESURER", "MEASURE")).strong().size(16.0).color(PLAYING_INK)).fill(GO).min_size(vec2(ui.available_width(), 38.0));
                if ui.add(go).on_hover_text(tr("Joue le sweep et calcule la réponse", "Plays the sweep and computes the response")).clicked() {
                    ctrl.measure();
                }
            }
        }
        let p = &ctrl.prefs;
        ui.label(RichText::new(format!("Sweep {:.1} s · {:.0} Hz – {:.1} kHz · {} {:.1} s", p.sweep_secs, p.f1, p.f2 / 1000.0, tr("traîne", "tail"), p.tail_secs)).small().color(TEXT_DIM))
            .on_hover_text(tr("Réglages du sweep dans les Préférences", "Sweep settings in Preferences"));
        ui.add_space(4.0);
    }

    let (label, enabled, hint) = match kind {
        TraceKind::Sweep => (tr("Capturer la mesure", "Capture measurement"), ctrl.current.is_some(), tr("Garde la dernière mesure dans la liste et le dossier", "Keeps the last measurement in the list and the folder")),
        TraceKind::Rta => (tr("Capturer le RTA", "Capture RTA"), rta.snapshot().is_some(), tr("Fige le spectre moyenné du RTA", "Freezes the averaged RTA spectrum")),
        TraceKind::Transfer => (tr("Capturer le transfert", "Capture transfer"), transfer.snapshot(0.0).is_some(), tr("Fige la fonction de transfert en cours", "Freezes the current transfer function")),
    };
    let b = egui::Button::new((icons::atom_colored(Icon::Plus, if enabled { PLAYING_INK } else { TEXT_DIM }), RichText::new(label).strong().color(if enabled { PLAYING_INK } else { TEXT_DIM })))
        .fill(if enabled { GO } else { PANEL_HI })
        .min_size(vec2(ui.available_width(), 30.0));
    if ui.add_enabled(enabled, b).on_hover_text(hint).clicked() {
        match kind {
            TraceKind::Sweep => ctrl.freeze(),
            TraceKind::Rta => {
                if let Some(s) = rta.snapshot() {
                    ctrl.capture_rta(s);
                }
            }
            TraceKind::Transfer => {
                if let Some(s) = transfer.snapshot(ctrl.prefs.tf_delay_ms) {
                    ctrl.capture_transfer(s);
                }
            }
        }
    }

    // Dernière mesure, pas encore capturée.
    if kind == TraceKind::Sweep && ctrl.current.is_some() {
        ui.horizontal(|ui| {
            swatch(ui, Color32::WHITE);
            ui.label(RichText::new(tr("Dernière mesure", "Last measurement")).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("REW").on_hover_text(tr("Exporter la réponse en fréquence (texte REW)", "Export the frequency response (REW text)")).clicked() {
                    action = Some(MeasuresAction::Rew(None));
                }
                if ui.small_button("WAV").on_hover_text(tr("Exporter la réponse impulsionnelle", "Export the impulse response")).clicked() {
                    action = Some(MeasuresAction::Wav(None));
                }
            });
        });
    }
    ui.separator();

    // Liste des mesures de la vue.
    let shown: Vec<usize> = (0..ctrl.traces.len()).filter(|&i| ctrl.traces[i].kind == kind).collect();
    let confirm_id = ui.make_persistent_id("supprimer-mesure");
    let mut confirm: Option<usize> = ui.data(|d| d.get_temp(confirm_id));
    let mut visible = None;
    let mut save = None;
    let mut recolor: Option<(usize, [u8; 3])> = None;
    let mut delete = None;
    let list_h = (ui.available_height() - 64.0).max(80.0);
    egui::ScrollArea::vertical().max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
        if shown.is_empty() {
            ui.add_space(12.0);
            ui.label(RichText::new(tr("Aucune mesure enregistrée.\nCapturez une mesure ou le RTA pour la garder ici.", "No saved measurement.\nCapture a measurement or the RTA to keep it here.")).color(TEXT_DIM));
        }
        for &i in shown.iter().rev() {
            let t = &mut ctrl.traces[i];
            ui.horizontal(|ui| {
                let mut v = t.visible;
                if ui.checkbox(&mut v, "").on_hover_text(tr("Afficher", "Show")).changed() {
                    visible = Some((i, v));
                }
                let sw = swatch(ui, rgb(t.color)).on_hover_text(tr("Couleur", "Colour"));
                egui::Popup::menu(&sw).close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside).show(|ui| {
                    ui.set_min_width(200.0);
                    egui::Grid::new(("palette", i)).spacing([4.0, 4.0]).show(ui, |ui| {
                        for (k, c) in TRACE_COLORS.iter().enumerate() {
                            let (r, resp) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::click());
                            ui.painter().rect_filled(r, CornerRadius::same(3), rgb(*c));
                            if *c == t.color {
                                ui.painter().rect_stroke(r, CornerRadius::same(3), egui::Stroke::new(2.0, TEXT), egui::StrokeKind::Outside);
                            }
                            if resp.clicked() {
                                recolor = Some((i, *c));
                            }
                            if k % 8 == 7 {
                                ui.end_row();
                            }
                        }
                    });
                    ui.separator();
                    let mut c = rgb(t.color);
                    if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::Opaque) {
                        recolor = Some((i, [c.r(), c.g(), c.b()]));
                    }
                });
                // Le menu à droite, puis le nom dans la place qui reste : la ligne ne
                // dépasse jamais la largeur du panneau.
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.menu_button("⋯", |ui| {
                        if t.kind == TraceKind::Sweep {
                            if ui.button(tr("Exporter l'IR en WAV…", "Export IR as WAV…")).clicked() {
                                action = Some(MeasuresAction::Wav(Some(i)));
                                ui.close();
                            }
                            if ui.button(tr("Exporter en texte REW…", "Export as REW text…")).clicked() {
                                action = Some(MeasuresAction::Rew(Some(i)));
                                ui.close();
                            }
                            ui.separator();
                        }
                        if ui.button((icons::atom(Icon::Trash), tr("Supprimer…", "Delete…"))).clicked() {
                            confirm = Some(i);
                            ui.close();
                        }
                    });
                    let edit = ui.add(egui::TextEdit::singleline(&mut t.name).desired_width(ui.available_width()));
                    if edit.lost_focus() {
                        save = Some(i);
                    }
                    edit.on_hover_text(t.date());
                });
            });
            if confirm == Some(i) {
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.label(RichText::new(tr("Supprimer le fichier ?", "Delete the file?")).color(STOP));
                    if ui.small_button(tr("Supprimer", "Delete")).clicked() {
                        delete = Some(i);
                        confirm = None;
                    }
                    if ui.small_button(tr("Annuler", "Cancel")).clicked() {
                        confirm = None;
                    }
                });
            }
        }
    });
    ui.data_mut(|d| match confirm {
        Some(i) => {
            d.insert_temp(confirm_id, i);
        }
        None => d.remove::<usize>(confirm_id),
    });
    if let Some((i, v)) = visible {
        ctrl.set_visible(i, v);
    }
    if let Some(i) = save {
        ctrl.save_trace(i);
    }
    if let Some((i, c)) = recolor {
        ctrl.set_color(i, c);
    }
    if let Some(i) = delete {
        ctrl.delete_trace(i);
    }

    // Bas du panneau : tout afficher ou masquer, import, dossier.
    ui.separator();
    ui.horizontal(|ui| {
        let any = !shown.is_empty();
        if ui.add_enabled(any, egui::Button::new(tr("Tout afficher", "Show all")).small()).clicked() {
            for &i in &shown {
                ctrl.set_visible(i, true);
            }
        }
        if ui.add_enabled(any, egui::Button::new(tr("Tout masquer", "Hide all")).small()).clicked() {
            for &i in &shown {
                ctrl.set_visible(i, false);
            }
        }
        if ui.small_button(tr("Importer…", "Import…")).on_hover_text(tr("Copie des fichiers .ssm dans le dossier des mesures", "Copies .ssm files into the measurements folder")).clicked() {
            action = Some(MeasuresAction::Import);
        }
    });
    ui.horizontal(|ui| {
        let dir = ctrl.measures_dir();
        if ui.small_button(tr("Dossier…", "Folder…")).on_hover_text(tr("Choisir le dossier des mesures", "Choose the measurements folder")).clicked() {
            action = Some(MeasuresAction::ChooseDir);
        }
        if ui.small_button(tr("Ouvrir", "Open")).on_hover_text(tr("Ouvrir le dossier dans l'explorateur", "Open the folder in the file manager")).clicked() {
            if let Some(d) = &dir {
                if let Err(e) = crate::traces::reveal(d) {
                    ctrl.status = Some(format!("{} : {e}", tr("dossier inaccessible", "folder unavailable")));
                }
            }
        }
        if let Some(d) = &dir {
            ui.add(egui::Label::new(RichText::new(d.display().to_string()).small().color(TEXT_DIM)).truncate()).on_hover_text(d.display().to_string());
        }
    });
    action
}

/// Bandeau replié : un bouton pour rouvrir le panneau.
pub fn collapsed(ui: &mut egui::Ui) -> bool {
    ui.vertical_centered(|ui| ui.small_button("«").on_hover_text(tr("Afficher les mesures", "Show measurements")).clicked()).inner
}

fn swatch(ui: &mut egui::Ui, c: Color32) -> egui::Response {
    let (r, resp) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::click());
    ui.painter().rect_filled(r.shrink(1.0), CornerRadius::same(2), c);
    resp
}
