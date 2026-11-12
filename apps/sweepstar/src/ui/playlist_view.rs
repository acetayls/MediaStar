//! Panneau « Playlist » : fichiers de test à jouer au soundcheck, à l'allure de JingleStar.
//! Double-clic sur un morceau pour le jouer, glisser pour réordonner, fichiers et dossiers
//! déposés depuis l'explorateur. Même sortie et même ARRÊT (Échap) que le générateur.

use egui::{vec2, Align, Color32, CornerRadius, Layout, RichText, Sense};
use star_core::i18n::tr;
use star_ui::icons::{self, Icon};
use star_ui::theme::*;
use star_ui::widgets::fmt_time;

use crate::controller::Controller;

/// Action demandée par le panneau à l'appli (boîte de dialogue).
pub enum PlaylistAction {
    AddFiles,
}

pub fn show(ui: &mut egui::Ui, ctrl: &mut Controller) -> Option<PlaylistAction> {
    let mut action = None;
    ui.horizontal(|ui| {
        ui.label(RichText::new("PLAYLIST").strong().color(ACCENT));
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if ui.add_enabled(!ctrl.session.playlist.is_empty(), egui::Button::new((icons::atom(Icon::Trash), tr("Vider", "Clear"))).small()).clicked() {
                ctrl.clear_playlist();
            }
            if ui.add(egui::Button::new((icons::atom(Icon::Plus), tr("Ajouter…", "Add…"))).small()).clicked() {
                action = Some(PlaylistAction::AddFiles);
            }
        });
    });

    // Transport.
    let playing = ctrl.file_playing();
    ui.horizontal(|ui| {
        let b = |icon| egui::Button::new(icons::atom(icon)).min_size(vec2(34.0, 28.0));
        if ui.add(b(Icon::Prev)).on_hover_text(tr("Morceau précédent", "Previous track")).clicked() {
            ctrl.prev_track();
        }
        let play = egui::Button::new(icons::atom_colored(Icon::Play, if playing { PLAYING_INK } else { GO })).fill(if playing { GO } else { PANEL_HI }).min_size(vec2(44.0, 28.0));
        if ui.add(play).on_hover_text(tr("Jouer le morceau sélectionné", "Play the selected track")).clicked() {
            ctrl.play_track(ctrl.track.unwrap_or(0));
        }
        if ui.add(b(Icon::Stop)).on_hover_text(tr("Arrêter (Échap)", "Stop (Esc)")).clicked() {
            ctrl.stop_all();
        }
        if ui.add(b(Icon::Next)).on_hover_text(tr("Morceau suivant", "Next track")).clicked() {
            ctrl.next_track();
        }
        let sel = ctrl.track.unwrap_or(0);
        let lp = ctrl.is_looped(sel);
        if ui
            .add_enabled(!ctrl.session.playlist.is_empty(), egui::Button::selectable(lp, (icons::atom(Icon::Refresh), tr("Boucle", "Loop"))))
            .on_hover_text(tr("Le morceau sélectionné repart au début à la fin", "The selected track restarts at the end"))
            .clicked()
        {
            ctrl.toggle_loop(sel);
        }
    });
    ui.horizontal(|ui| {
        if ctrl.is_loading_track() {
            ui.spinner();
            ui.label(RichText::new(tr("Chargement…", "Loading…")).color(TEXT_DIM));
        } else if playing {
            let (pos, len) = ctrl.track_time();
            if let Some(to) = seek_bar(ui, pos, len) {
                ctrl.seek(to);
            }
        } else {
            ui.label(RichText::new(tr("Arrêté", "Stopped")).color(TEXT_DIM));
        }
    });
    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 120.0).max(60.0);
        ui.label(tr("Niveau", "Level"));
        let mut db = ctrl.session.playlist_db;
        if ui.add(egui::Slider::new(&mut db, -60.0..=0.0).suffix(" dB").step_by(0.5)).changed() {
            ctrl.set_playlist_level(db);
        }
    });
    ui.separator();

    // Liste : double-clic pour jouer, glisser pour réordonner.
    let mut play = None;
    let mut remove = None;
    let mut moved = None;
    let mut toggle = None;
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        if ctrl.session.playlist.is_empty() {
            ui.add_space(20.0);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(tr("Glissez ici des fichiers audio ou un dossier,\nou cliquez sur Ajouter…", "Drop audio files or a folder here,\nor click Add…")).color(TEXT_DIM));
            });
        }
        for (i, path) in ctrl.session.playlist.iter().enumerate() {
            let name = path.file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            let current = ctrl.track == Some(i);
            let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 24.0), Sense::click_and_drag());
            let fill = if current && playing {
                GO.gamma_multiply(0.35)
            } else if current {
                ACCENT.gamma_multiply(0.25)
            } else if resp.hovered() {
                PANEL_HI
            } else {
                Color32::TRANSPARENT
            };
            let p = ui.painter();
            p.rect_filled(rect, CornerRadius::same(3), fill);
            p.text(rect.left_center() + vec2(6.0, 0.0), egui::Align2::LEFT_CENTER, format!("{:>2}", i + 1), egui::FontId::monospace(11.0), TEXT_DIM);
            p.text(rect.left_center() + vec2(30.0, 0.0), egui::Align2::LEFT_CENTER, &name, egui::FontId::proportional(13.0), TEXT);
            // Bouton de boucle du morceau, au bout de la ligne.
            let looped = ctrl.is_looped(i);
            let lr = egui::Rect::from_center_size(rect.right_center() - vec2(14.0, 0.0), vec2(22.0, 20.0));
            let lresp = ui.interact(lr, ui.scope_id().with(("boucle", i)), Sense::click());
            if looped || lresp.hovered() || resp.hovered() {
                let col = if looped { ACCENT } else if lresp.hovered() { TEXT } else { TEXT_DIM };
                icons::paint(ui.painter(), egui::Rect::from_center_size(lr.center(), vec2(14.0, 14.0)), Icon::Refresh, col);
            }
            if lresp.on_hover_text(tr("Jouer ce morceau en boucle", "Play this track in a loop")).clicked() {
                toggle = Some(i);
            }
            resp.dnd_set_drag_payload(i);
            if let Some(from) = resp.dnd_release_payload::<usize>() {
                moved = Some((*from, i));
            }
            if resp.dnd_hover_payload::<usize>().is_some() {
                p.hline(rect.x_range(), rect.top(), egui::Stroke::new(2.0, ACCENT));
            }
            let resp = resp.on_hover_text(path.display().to_string());
            if resp.double_clicked() {
                play = Some(i);
            }
            resp.context_menu(|ui| {
                if ui.button(tr("Jouer", "Play")).clicked() {
                    play = Some(i);
                    ui.close();
                }
                if ui.button(tr("Retirer de la playlist", "Remove from playlist")).clicked() {
                    remove = Some(i);
                    ui.close();
                }
            });
        }
    });
    if let Some((from, to)) = moved {
        ctrl.move_track(from, to);
    }
    if let Some(i) = toggle {
        ctrl.toggle_loop(i);
    }
    if let Some(i) = remove {
        ctrl.remove_track(i);
    }
    if let Some(i) = play {
        ctrl.play_track(i);
    }
    action
}

/// Barre de position du morceau : clic ou glisser pour s'y rendre (saut au relâché).
/// Rend la position demandée (s).
fn seek_bar(ui: &mut egui::Ui, pos: f64, len: f64) -> Option<f64> {
    let id = ui.scope_id().with("position");
    let label_w = 96.0;
    let (rect, resp) = ui.allocate_exact_size(vec2((ui.available_width() - label_w).max(60.0), 18.0), Sense::click_and_drag());
    let frac_at = |x: f32| (((x - rect.left()) / rect.width()) as f64).clamp(0.0, 1.0);
    // Pendant le glisser, la barre suit la souris ; le saut a lieu au relâché.
    let mut dragged: Option<f64> = ui.data(|d| d.get_temp(id));
    if let Some(p) = resp.interact_pointer_pos() {
        if resp.dragged() {
            dragged = Some(frac_at(p.x));
        }
    }
    let mut out = None;
    if resp.drag_stopped() || resp.clicked() {
        if let Some(p) = resp.interact_pointer_pos() {
            out = Some(frac_at(p.x) * len);
        } else if let Some(f) = dragged {
            out = Some(f * len);
        }
        dragged = None;
    }
    ui.data_mut(|d| match dragged {
        Some(f) => {
            d.insert_temp(id, f);
        }
        None => d.remove::<f64>(id),
    });
    let shown = dragged.unwrap_or(if len > 0.0 { pos / len } else { 0.0 }).clamp(0.0, 1.0);
    let p = ui.painter();
    let track = egui::Rect::from_center_size(rect.center(), vec2(rect.width(), 6.0));
    p.rect_filled(track, CornerRadius::same(3), PANEL_HI);
    let x = rect.left() + shown as f32 * rect.width();
    p.rect_filled(egui::Rect::from_min_max(track.min, egui::pos2(x, track.max.y)), CornerRadius::same(3), GO);
    p.circle_filled(egui::pos2(x, rect.center().y), if resp.hovered() || dragged.is_some() { 6.0 } else { 4.5 }, TEXT);
    if let Some(h) = resp.hover_pos() {
        resp.clone().on_hover_text_at_pointer(fmt_time(frac_at(h.x) * len));
    }
    let t = dragged.map_or(pos, |f| f * len);
    ui.label(RichText::new(format!("{} / {}", fmt_time(t), fmt_time(len))).monospace().color(TEXT));
    out
}
