//! Éditeur d'un cue de la Cuelist, comme celui d'un jingle de
//! JingleStar : volume, lecture une fois ou en boucle, fondus au noir, frise de vignettes avec
//! ses poignées d'entrée et de sortie, égaliseur de la bande son. Et l'aperçu où l'on se
//! déplace dans le média sans toucher à la sortie.

use crate::preview::Preview;
use crate::protocol::Clip;
use egui::{pos2, vec2, Align, Color32, CornerRadius, Layout, Rect, RichText, Sense, Stroke, Ui};
use star_core::i18n::tr;
use star_core::sound::SoundSettings;
use star_ui::theme::*;
use star_ui::widgets::{fmt_time, waveform_editor, Markers, WaveView};
use std::path::Path;

/// Vignettes de la frise.
const STRIP_COUNT: usize = 24;

pub fn is_image(p: &Path) -> bool {
    p.extension().map(|e| ["jpg", "jpeg", "png", "bmp", "gif", "tif", "tiff", "webp"].contains(&e.to_string_lossy().to_lowercase().as_str())).unwrap_or(false)
}

pub fn is_audio(p: &Path) -> bool {
    p.extension().map(|e| ["mp3", "wav", "flac", "aac", "m4a", "ogg"].contains(&e.to_string_lossy().to_lowercase().as_str())).unwrap_or(false)
}

/// Fond de l'éditeur : vignettes de la vidéo côte à côte sur la partie visible, assombries
/// pendant les fondus (le noir que verra la salle).
fn strip_background(tiles: Vec<Option<egui::TextureHandle>>, duration: f64) -> impl FnOnce(&WaveView) {
    move |w: &WaveView| {
        let p = w.painter;
        let r = w.rect;
        let n = tiles.len().max(1);
        let tile_w = (r.height() * 16.0 / 9.0).max(8.0);
        let mut x = r.left();
        while x < r.right() {
            let cell = Rect::from_min_size(pos2(x, r.top()), vec2(tile_w.min(r.right() - x), r.height()));
            let t = w.t_of(cell.center().x);
            let i = ((t / duration.max(1e-6)) * n as f64).floor().clamp(0.0, (n - 1) as f64) as usize;
            match tiles.get(i).and_then(|t| t.as_ref()) {
                Some(tex) => {
                    let full = Rect::from_min_size(cell.min, vec2(tile_w, r.height()));
                    let uv_w = (cell.width() / tile_w).clamp(0.0, 1.0);
                    p.image(tex.id(), cell, Rect::from_min_max(pos2(0.0, 0.0), pos2(uv_w, 1.0)), Color32::WHITE);
                    let _ = full;
                }
                None => {
                    p.rect_filled(cell.shrink(1.0), CornerRadius::same(2), PANEL_HI);
                }
            }
            x += tile_w;
        }
        // Fondus : voile noir selon le gain, colonne par colonne.
        let step = 3.0;
        let mut x = r.left();
        while x < r.right() {
            let g = (w.fade_gain)(w.t_of(x + step / 2.0));
            if g < 0.999 {
                let a = ((1.0 - g.clamp(0.0, 1.0)) * 235.0) as u8;
                p.rect_filled(Rect::from_x_y_ranges(x..=(x + step).min(r.right()), r.y_range()), CornerRadius::ZERO, Color32::from_black_alpha(a));
            }
            x += step;
        }
    }
}

/// Image ajustée dans `rect`, tournée de `rot` degrés (sens des aiguilles d'une montre).
pub fn paint_texture(painter: &egui::Painter, tex: &egui::TextureHandle, rect: Rect, rot: u16) {
    let ts = tex.size_vec2();
    let quarter = rot == 90 || rot == 270;
    let shown = if quarter { vec2(ts.y, ts.x) } else { ts };
    let k = (rect.width() / shown.x.max(1.0)).min(rect.height() / shown.y.max(1.0));
    let r = Rect::from_center_size(rect.center(), ts * k);
    let mut mesh = egui::Mesh::with_texture(tex.id());
    mesh.add_rect_with_uv(r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    if rot != 0 {
        mesh.rotate(egui::emath::Rot2::from_angle((rot as f32).to_radians()), rect.center());
    }
    painter.add(egui::Shape::mesh(mesh));
}

/// Image de l'aperçu, ajustée dans `rect` (16:9) et tournée comme sur la sortie, ou la vignette
/// fixe en attendant.
pub fn preview_image(ui: &mut Ui, preview: &mut Preview, path: &Path, rect: Rect, fallback: Option<&egui::TextureHandle>, rot: u16) {
    ui.painter().rect_filled(rect, 3, Color32::BLACK);
    let ctx = ui.ctx().clone();
    let live = if preview.state.path.as_deref() == Some(path) { preview.texture(&ctx).cloned() } else { None };
    match live.as_ref().or(fallback) {
        // Image de l'aperçu : brute, la rotation du fichier s'ajoute à celle du cue.
        Some(t) => paint_texture(&ui.painter_at(rect), t, rect, if live.is_some() { (rot + preview.state.rotate) % 360 } else { rot }),
        None => {
            let txt = if is_audio(path) { tr("son seul", "audio only") } else { "…" };
            ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, txt, egui::FontId::proportional(14.0), TEXT_DIM);
        }
    }
}

/// Point à caler sur l'image affichée par l'aperçu.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mark {
    In(f64),
    Out(f64),
}

impl Mark {
    /// Pose le point dans les réglages de l'élément (la sortie reste après l'entrée).
    pub fn apply(self, clip: &mut Clip, duration: f64) {
        let s = &mut clip.sound;
        match self {
            Mark::In(t) => {
                s.start = t.max(0.0);
                if s.end.is_some_and(|e| e <= s.start) {
                    s.end = None;
                }
            }
            Mark::Out(t) => {
                if t > s.start {
                    s.end = if duration > 0.0 && t >= duration - 1e-3 { None } else { Some(t) };
                }
            }
        }
    }
}

/// Temps à l'image près : « 1:23.12 » (secondes et numéro d'image dans la seconde), ou au
/// centième si la cadence est inconnue.
pub fn fmt_frames(t: f64, fps: f64) -> String {
    let t = t.max(0.0);
    if fps > 1.0 {
        let per = fps.round().max(1.0) as u64;
        let n = (t * fps + 1e-6).floor() as u64;
        let (secs, f) = (n / per, n % per);
        let h = secs / 3600;
        if h > 0 {
            format!("{h}:{:02}:{:02}.{f:02}", (secs / 60) % 60, secs % 60)
        } else {
            format!("{}:{:02}.{f:02}", secs / 60, secs % 60)
        }
    } else {
        let cs = (t * 100.0).round() as u64;
        format!("{}:{:02},{:02}", cs / 6000, (cs / 100) % 60, cs % 100)
    }
}

/// Barre de déplacement de l'aperçu, avec les points d'entrée et de sortie de l'élément,
/// puis le temps à l'image près, les boutons lecture, image précédente et suivante, et
/// « Entrée ici » / « Sortie ici » qui calent le point sur l'image affichée (si `marks` ; dans
/// l'éditeur du cue, ces boutons sont avec les points).
/// Glisser : saut aux images clés (fluide) ; au relâché, position exacte.
pub fn scrub_bar(ui: &mut Ui, id: impl std::hash::Hash, preview: &mut Preview, path: &Path, clip: &Clip, marks: bool) -> Option<Mark> {
    let mut mark = None;
    let ctx = ui.ctx().clone();
    let len = if preview.state.path.as_deref() == Some(path) { preview.state.duration } else { 0.0 };
    let mut t = if len > 0.0 { preview.state.time.clamp(0.0, len) } else { 0.0 };
    let (hit, bar) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::click_and_drag());
    let r = Rect::from_center_size(hit.center(), vec2(hit.width(), 6.0));
    let frac_at = |x: f32| ((x - r.left()) / r.width()).clamp(0.0, 1.0) as f64;
    let x_at = |t: f64| r.left() + r.width() * (t / len.max(1e-6)).clamp(0.0, 1.0) as f32;
    let p = ui.painter();
    p.rect_filled(r, CornerRadius::same(3), PANEL_HI);
    if len > 0.0 {
        if let Some(pos) = bar.interact_pointer_pos() {
            let to = frac_at(pos.x) * len;
            if bar.dragged() {
                preview.seek(&ctx, to, false);
                t = to;
            }
            if bar.clicked() || bar.drag_stopped() {
                preview.seek(&ctx, to, true);
                t = to;
            }
        }
        if bar.hovered() || bar.dragged() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        let p = ui.painter();
        // Partie jouée de l'élément, entre ses poignées.
        let s = &clip.sound;
        let (st, _, _, end) = s.markers(len);
        p.rect_filled(Rect::from_x_y_ranges(x_at(st)..=x_at(end), r.y_range()), CornerRadius::same(3), GO.gamma_multiply(0.35));
        for (x, c) in [(x_at(st), GO), (x_at(end), STOP)] {
            p.line_segment([pos2(x, hit.top()), pos2(x, hit.bottom())], Stroke::new(2.0, c));
        }
        let x = x_at(t);
        p.line_segment([pos2(x, hit.top()), pos2(x, hit.bottom())], Stroke::new(2.0, Color32::WHITE));
    }
    ui.horizontal(|ui| {
        let playing = !preview.state.paused && len > 0.0;
        let small = |ui: &mut Ui, icon, salt: &str, tip: &str| {
            let (r, _) = ui.allocate_exact_size(vec2(30.0, 22.0), Sense::hover());
            star_ui::widgets::transport_button(ui, r, salt, icon, None, PANEL_HI, TEXT).on_hover_text(tip).clicked()
        };
        use star_ui::widgets::TransportIcon;
        let (icon, tip) = if playing {
            (TransportIcon::Pause, tr("Mettre l'aperçu en pause", "Pause the preview"))
        } else {
            (TransportIcon::Play, tr("Lire l'aperçu (sans son, la sortie ne bouge pas)", "Play the preview (muted, the output is not touched)"))
        };
        if small(ui, icon, "pv-play", tip) {
            preview.play(&ctx, !playing);
        }
        if small(ui, TransportIcon::Prev, "pv-back", tr("Image précédente", "Previous frame")) {
            preview.step(&ctx, true);
        }
        if small(ui, TransportIcon::Next, "pv-fwd", tr("Image suivante (→)", "Next frame (→)")) {
            preview.step(&ctx, false);
        }
        let fps = preview.state.fps;
        ui.label(RichText::new(fmt_frames(t, fps)).monospace().color(TEXT))
            .on_hover_text(if fps > 1.0 { format!("{} {fps:.3} {}", tr("minutes:secondes.image, à", "minutes:seconds.frame, at"), tr("images/s", "fps")) } else { tr("Cadence inconnue : temps au centième", "Unknown frame rate: time in hundredths").to_string() });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(RichText::new(format!("-{}", fmt_time((len - t).max(0.0)))).monospace().color(TEXT_DIM));
            if len > 0.0 && marks {
                if ui.add(egui::Button::new(RichText::new(tr("Sortie ici ]", "Out here ]")).color(STOP)).small())
                    .on_hover_text(tr("Le point de sortie se cale sur l'image affichée (touche O)", "Set the out point on the frame shown (O key)"))
                    .clicked()
                {
                    mark = Some(Mark::Out(t));
                }
                if ui.add(egui::Button::new(RichText::new(tr("[ Entrée ici", "[ In here")).color(GO)).small())
                    .on_hover_text(tr("Le point d'entrée se cale sur l'image affichée (touche I)", "Set the in point on the frame shown (I key)"))
                    .clicked()
                {
                    mark = Some(Mark::In(t));
                }
            }
        });
    });
    let _ = id;
    mark
}

/// Raccourcis de l'aperçu dans la fenêtre d'édition (hors saisie de texte) : ← → image par
/// image, Maj + ← → une seconde, K lecture / pause, I et O calent l'entrée et la sortie.
fn preview_keys(ui: &mut Ui, preview: &mut Preview, path: &Path) -> Option<Mark> {
    let ctx = ui.ctx().clone();
    if ctx.text_edit_focused() || !preview.shows(path) || preview.state.image {
        return None;
    }
    let len = preview.state.duration;
    let none = egui::Modifiers::NONE;
    let shift = egui::Modifiers::SHIFT;
    let (back, fwd, back_s, fwd_s, play, set_in, set_out) = ctx.input_mut(|i| {
        (
            i.count_and_consume_key(none, egui::Key::ArrowLeft),
            i.count_and_consume_key(none, egui::Key::ArrowRight),
            i.consume_key(shift, egui::Key::ArrowLeft),
            i.consume_key(shift, egui::Key::ArrowRight),
            i.consume_key(none, egui::Key::K),
            i.consume_key(none, egui::Key::I),
            i.consume_key(none, egui::Key::O),
        )
    });
    for _ in 0..back {
        preview.step(&ctx, true);
    }
    for _ in 0..fwd {
        preview.step(&ctx, false);
    }
    if back_s || fwd_s {
        let t = (preview.state.time + if back_s { -1.0 } else { 1.0 }).clamp(0.0, len.max(0.0));
        preview.seek(&ctx, t, true);
    }
    if play {
        let playing = !preview.state.paused;
        preview.play(&ctx, !playing);
    }
    let t = preview.state.time.clamp(0.0, len.max(0.0));
    if set_in {
        Some(Mark::In(t))
    } else if set_out {
        Some(Mark::Out(t))
    } else {
        None
    }
}

/// Timecode « HH:MM:SS:FF » (heures, minutes, secondes, image dans la seconde). Cadence
/// inconnue (son seul) : « HH:MM:SS.cc », au centième.
pub fn fmt_tc(t: f64, fps: f64) -> String {
    let t = t.max(0.0);
    let (per, rate, sep) = if fps > 1.0 { (fps.round().max(1.0) as u64, fps, ':') } else { (100, 100.0, '.') };
    let n = (t * rate + 1e-6).floor() as u64;
    let (secs, f) = (n / per, n % per);
    format!("{:02}:{:02}:{:02}{sep}{f:02}", secs / 3600, (secs / 60) % 60, secs % 60)
}

/// Lit un timecode saisi : « HH:MM:SS:FF » (image dans la seconde), ou « [[HH:]MM:]SS[.ss] »
/// (secondes, éventuellement décimales).
pub fn parse_tc(s: &str, fps: f64) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    let parts: Vec<&str> = s.split([':', ';']).map(str::trim).collect();
    let (per, rate) = if fps > 1.0 { (fps.round().max(1.0), fps) } else { (100.0, 100.0) };
    let num = |x: &str| if x.is_empty() { Some(0.0) } else { x.parse::<f64>().ok().filter(|v| v.is_finite() && *v >= 0.0) };
    if parts.len() == 4 {
        let (h, m, sec, f) = (num(parts[0])?, num(parts[1])?, num(parts[2])?, num(parts[3])?);
        let n = ((h * 3600.0 + m * 60.0 + sec.floor()) * per + f.floor().min(per - 1.0)).round();
        Some(n / rate)
    } else if (1..=3).contains(&parts.len()) {
        parts.iter().try_fold(0.0, |acc, x| Some(acc * 60.0 + num(x)?))
    } else {
        None
    }
}

/// Case de saisie d'un timecode : la valeur saisie est prise à la validation (Entrée ou clic
/// ailleurs).
fn tc_field(ui: &mut Ui, id: egui::Id, v: &mut f64, fps: f64, max: f64) {
    let buf_id = id.with("buf");
    let shown = fmt_tc(*v, fps);
    let editing = ui.memory(|m| m.has_focus(id));
    let mut buf = if editing { ui.data(|d| d.get_temp::<String>(buf_id)).unwrap_or(shown) } else { shown };
    let resp = ui.add(egui::TextEdit::singleline(&mut buf).id(id).font(egui::TextStyle::Monospace).desired_width(100.0).align(egui::Align2::CENTER_CENTER));
    let resp = resp.on_hover_text(if fps > 1.0 {
        tr("HH:MM:SS:II (heures, minutes, secondes, image). Accepte aussi MM:SS ou des secondes.", "HH:MM:SS:FF (hours, minutes, seconds, frame). Also accepts MM:SS or seconds.")
    } else {
        tr("HH:MM:SS.cc (au centième). Accepte aussi MM:SS ou des secondes.", "HH:MM:SS.cc (hundredths). Also accepts MM:SS or seconds.")
    });
    if resp.has_focus() {
        ui.data_mut(|d| d.insert_temp(buf_id, buf));
    } else {
        if resp.lost_focus() {
            if let Some(t) = parse_tc(&buf, fps) {
                *v = t.clamp(0.0, max.max(0.0));
            }
        }
        ui.data_mut(|d| d.remove::<String>(buf_id));
    }
}

/// Points du cue rassemblés : entrée et son fondu, boucle, sortie et son fondu, avec les
/// boutons qui calent l'entrée ou la sortie sur l'image affichée par l'aperçu (`now`).
fn points_rows(ui: &mut Ui, id_salt: impl std::hash::Hash + std::fmt::Debug + Copy, s: &mut SoundSettings, duration: f64, fps: f64, now: f64) -> Option<Mark> {
    let mut mark = None;
    let (st, ls, le, end) = s.markers(duration);
    let mut mk = Markers { start: st, loop_start: ls, loop_end: le, end };
    let before = mk;
    let fade_tip = tr("Les fondus partent du noir et y reviennent, image et son ensemble.", "Fades go from and to black, picture and sound together.");
    let id = |k: &str| egui::Id::unique(("cue-tc", id_salt, k.to_string()));
    egui::Grid::new(("clip-points", id_salt)).num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.label(RichText::new(tr("Entrée", "In point")).color(GO));
        ui.horizontal(|ui| {
            tc_field(ui, id("in"), &mut mk.start, fps, duration);
            ui.label(RichText::new(tr("fondu", "fade")).color(TEXT_DIM)).on_hover_text(fade_tip);
            ui.add(egui::DragValue::new(&mut s.fade_in).range(0.0..=30.0).speed(0.05).suffix(" s").max_decimals(2)).on_hover_text(fade_tip);
            if ui.add(egui::Button::new(RichText::new(tr("[ Entrée ici", "[ Set in")).color(GO)).small())
                .on_hover_text(tr("Le point d'entrée se cale sur l'image affichée (touche I)", "Set the in point on the frame shown (I key)"))
                .clicked()
            {
                mark = Some(Mark::In(now));
            }
        });
        ui.end_row();
        if s.looping {
            ui.label(RichText::new(tr("Boucle", "Loop")).color(ACCENT));
            ui.horizontal(|ui| {
                tc_field(ui, id("loop-in"), &mut mk.loop_start, fps, duration);
                ui.label("→");
                tc_field(ui, id("loop-out"), &mut mk.loop_end, fps, duration);
            });
            ui.end_row();
        }
        ui.label(RichText::new(tr("Sortie", "Out point")).color(STOP));
        ui.horizontal(|ui| {
            tc_field(ui, id("out"), &mut mk.end, fps, duration);
            ui.label(RichText::new(tr("fondu", "fade")).color(TEXT_DIM)).on_hover_text(fade_tip);
            ui.add(egui::DragValue::new(&mut s.fade_out).range(0.0..=30.0).speed(0.05).suffix(" s").max_decimals(2)).on_hover_text(fade_tip);
            if ui.add(egui::Button::new(RichText::new(tr("Sortie ici ]", "Set out ]")).color(STOP)).small())
                .on_hover_text(tr("Le point de sortie se cale sur l'image affichée (touche O)", "Set the out point on the frame shown (O key)"))
                .clicked()
            {
                mark = Some(Mark::Out(now));
            }
        });
        ui.end_row();
        ui.label("");
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("{} {}", tr("Durée", "Duration"), fmt_tc(mk.end - mk.start, fps))).monospace().color(TEXT_DIM));
            if ui.small_button(tr("Réinitialiser les points", "Reset points")).clicked() {
                mark = None;
                mk = Markers { start: 0.0, loop_start: 0.0, loop_end: duration, end: duration };
            }
        });
        ui.end_row();
    });
    if mk != before {
        mk.end = mk.end.max(mk.start);
        mk.loop_start = mk.loop_start.clamp(mk.start, mk.end);
        mk.loop_end = mk.loop_end.clamp(mk.loop_start, mk.end);
        set_markers(s, &mk);
    }
    mark
}

fn set_markers(s: &mut SoundSettings, mk: &Markers) {
    s.start = mk.start;
    s.end = Some(mk.end);
    s.loop_start = Some(mk.loop_start);
    s.loop_end = Some(mk.loop_end);
}

/// Corps de l'éditeur d'un élément : rotation, aperçu centré, frise avec ses poignées et la
/// barre de déplacement dessous, points, volume, égaliseur. La lecture (une fois ou en
/// boucle) se règle avec la transition d'entrée, dans la fenêtre du cue.
/// À placer dans une fenêtre ; `clip` est modifié en place.
pub fn clip_editor(ui: &mut Ui, id_salt: impl std::hash::Hash + std::fmt::Debug + Copy, path: &Path, clip: &mut Clip, preview: &mut Preview) {
    let ctx = ui.ctx().clone();
    let image = is_image(path);
    let audio = is_audio(path);
    preview.open(&ctx, path, clip.sound.start);
    preview.poll();

    if !audio {
        egui::Grid::new(("clip-fields", id_salt)).num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label(tr("Rotation", "Rotation"));
            ui.horizontal(|ui| {
                let mut r = clip.rotation();
                for (v, l) in [(0u16, "0°"), (90, "90°"), (180, "180°"), (270, "270°")] {
                    ui.radio_value(&mut r, v, l);
                }
                clip.rotate = r;
                ui.label(RichText::new(tr("sens des aiguilles d'une montre, aperçu et sortie", "clockwise, preview and output")).small().color(TEXT_DIM));
            });
            ui.end_row();
            if image {
                ui.label(tr("Affichage", "Display"));
                ui.horizontal(|ui| {
                    let mut timed = clip.image_secs.is_some();
                    ui.radio_value(&mut timed, false, tr("Jusqu'au média suivant", "Until the next media"));
                    ui.radio_value(&mut timed, true, tr("Pendant", "For"));
                    if timed {
                        let mut d = clip.image_secs.unwrap_or(5.0);
                        ui.add(egui::DragValue::new(&mut d).range(0.1..=3600.0).speed(0.1).suffix(" s").max_decimals(1));
                        clip.image_secs = Some(d);
                    } else {
                        clip.image_secs = None;
                    }
                });
                ui.end_row();
                ui.label(tr("Fondus au noir", "Fades to black"));
                ui.horizontal(|ui| {
                    ui.label(tr("entrée", "in"));
                    ui.add(egui::DragValue::new(&mut clip.sound.fade_in).range(0.0..=30.0).speed(0.05).suffix(" s").max_decimals(2));
                    ui.label(tr("sortie", "out"));
                    ui.add_enabled(
                        clip.image_secs.is_some(),
                        egui::DragValue::new(&mut clip.sound.fade_out).range(0.0..=30.0).speed(0.05).suffix(" s").max_decimals(2),
                    )
                    .on_disabled_hover_text(tr("Le fondu de sortie d'une image demande une durée d'affichage", "An image's fade out needs a display time"));
                });
                ui.end_row();
            }
        });
        ui.add_space(6.0);
    }

    // Aperçu centré : l'image à la position de la tête de lecture.
    let w = ui.available_width().min(420.0);
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(vec2(w, w * 9.0 / 16.0), Sense::hover());
        preview_image(ui, preview, path, rect, None, clip.rotation());
    });
    if let Some(e) = &preview.state.missing {
        ui.label(RichText::new(format!("{} ({e})", tr("Aperçu indisponible", "Preview unavailable"))).small().color(METER_MID));
    }
    if image {
        return;
    }

    // Frise avec ses poignées, puis dessous la barre de déplacement (verte entre l'entrée et
    // la sortie).
    ui.add_space(6.0);
    let shown = preview.shows(path);
    let duration = preview.state.duration;
    let before = (clip.sound.start, clip.sound.end);
    let mut seek_to = None;
    if shown && duration > 0.0 {
        if !audio {
            preview.request_strip(&ctx, path, STRIP_COUNT);
        }
        let tiles: Vec<Option<egui::TextureHandle>> = if audio { Vec::new() } else { preview.strip_textures(&ctx).to_vec() };
        let playhead = Some(preview.state.time);
        let s = &mut clip.sound;
        let (st, ls, le, end) = s.markers(duration);
        let mut mk = Markers { start: st, loop_start: ls, loop_end: le, end };
        if waveform_editor(ui, strip_background(tiles, duration), duration, &mut mk, s.looping, &mut s.fade_in, &mut s.fade_out, playhead, &mut seek_to) {
            set_markers(s, &mk);
        }
    } else {
        ui.label(RichText::new(tr("Lecture du média…", "Reading the media…")).color(TEXT_DIM));
    }
    if let Some(t) = seek_to {
        preview.seek(&ctx, t, true);
    }
    let keys = preview_keys(ui, preview, path);
    let mut mark = scrub_bar(ui, ("clip-scrub", id_salt), preview, path, clip, false).or(keys);
    ui.label(
        RichText::new(tr(
            "← → image par image · Maj + ← → une seconde · K lecture/pause · I entrée · O sortie",
            "← → frame by frame · Shift + ← → one second · K play/pause · I in · O out",
        ))
        .small()
        .color(TEXT_DIM),
    );

    // Points rassemblés.
    if shown && duration > 0.0 {
        ui.add_space(6.0);
        let now = preview.state.time.clamp(0.0, duration);
        let fps = preview.state.fps;
        if let Some(m) = points_rows(ui, id_salt, &mut clip.sound, duration, fps, now) {
            mark = Some(m);
        }
    }
    if let Some(m) = mark {
        m.apply(clip, duration);
    }

    // Volume, juste au-dessus de l'égaliseur.
    ui.add_space(6.0);
    egui::Grid::new(("clip-volume", id_salt)).num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
        ui.label("Volume");
        ui.add(egui::Slider::new(&mut clip.sound.volume_db, -40.0..=10.0).suffix(" dB").step_by(0.5));
        ui.end_row();
    });
    let eq = &mut clip.sound.eq;
    let eq_title = if eq.is_flat() { RichText::new(tr("Égaliseur", "Equalizer")) } else { RichText::new(tr("Égaliseur (actif)", "Equalizer (on)")).color(ACCENT) };
    let open_by_default = !eq.is_flat();
    egui::CollapsingHeader::new(eq_title).id_salt(("sound-eq", ("clip", id_salt))).default_open(open_by_default).show(ui, |ui| {
        star_ui::eq_view::eq_editor(ui, ("sound", ("clip", id_salt)), eq, 48_000);
    });

    // Un point déplacé : l'aperçu montre l'image exacte à cet endroit, même pendant le
    // glisser (mpv ne garde que le dernier saut demandé), pour caler le point à l'image près.
    if clip.sound.start != before.0 {
        preview.seek(&ctx, clip.sound.start, true);
    } else if clip.sound.end != before.1 {
        if let Some(e) = clip.sound.end {
            preview.seek(&ctx, e, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timecode_round_trip() {
        assert_eq!(fmt_tc(3723.5, 25.0), "01:02:03:12");
        assert_eq!(parse_tc("01:02:03:12", 25.0), Some(3723.48));
        assert_eq!(parse_tc("1:30", 25.0), Some(90.0));
        assert_eq!(parse_tc("12,5", 25.0), Some(12.5));
        assert_eq!(fmt_tc(61.25, 0.0), "00:01:01.25");
        let t = parse_tc("00:00:10:07", 29.97).unwrap();
        assert_eq!(fmt_tc(t, 29.97), "00:00:10:07");
        assert!(parse_tc("abc", 25.0).is_none());
    }
}
