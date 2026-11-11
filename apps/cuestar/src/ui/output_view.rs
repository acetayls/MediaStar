//! Fenêtre Préférences, en onglets : écran et moteur de la sortie, audio, mires de test, mapping 4 coins, journal.

use crate::link::OutputLink;
use crate::output::warp;
use crate::prefs::{fps_label, Cadence, Decode, DelayUnit, MachinePrefs, RenderApi, Warp, FPS_CHOICES};
use crate::protocol::{Command, PatternKind, WarpGuides};
use egui::{pos2, vec2, Color32, Key, Pos2, Rect, RichText, Sense, Stroke};
use star_ui::theme::*;
use std::time::{Duration, Instant};

const CORNER_NAMES: [&str; 4] = ["Haut gauche", "Haut droit", "Bas droit", "Bas gauche"];
const MID_NAMES: [&str; 5] = ["Milieu haut", "Milieu droit", "Milieu bas", "Milieu gauche", "Centre"];

/// Onglets de la fenêtre Préférences.
#[derive(Default, Clone, Copy, PartialEq)]
enum Tab {
    #[default]
    Engine,
    Audio,
    Patterns,
    Mapping,
    System,
    Log,
}

const TABS: [(Tab, &str); 6] = [
    (Tab::Engine, "Vidéo"),
    (Tab::Audio, "Audio"),
    (Tab::Patterns, "Mires de test"),
    (Tab::Mapping, "Mapping 4 coins"),
    (Tab::System, "Système"),
    (Tab::Log, "Log"),
];

pub struct OutputView {
    /// Fenêtre flottante ouverte.
    pub open: bool,
    /// Réglages moteur en cours d'édition (appliqués en relançant la sortie).
    draft: Option<MachinePrefs>,
    tab: Tab,
    selected: usize,
    last_warp_send: Instant,
    warp_dirty: bool,
    /// Grille 8 × 8 avec les repères de la sortie.
    guide_grid: bool,
    /// Repères envoyés à la sortie en dernier.
    guides_sent: Option<WarpGuides>,
    /// Dernière modification refusée (image retournée), pour le message.
    refused: Option<Instant>,
    log: super::log_view::LogView,
    six25: super::six25::Six25,
}

impl Default for OutputView {
    fn default() -> Self {
        Self { open: false, draft: None, tab: Tab::Engine, selected: 0, last_warp_send: Instant::now(), warp_dirty: false, guide_grid: false, guides_sent: None, refused: None, log: Default::default(), six25: Default::default() }
    }
}

fn section(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::new().fill(PANEL).corner_radius(4).inner_margin(12).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.label(RichText::new(title).size(16.0).strong());
        ui.add_space(6.0);
        add(ui);
    });
    ui.add_space(10.0);
}

/// Ouvre le panneau « Écrans » du système d'exploitation.
fn open_display_settings() -> Result<(), String> {
    use std::process::Command;
    #[cfg(windows)]
    let tries: &[(&str, &[&str])] = &[("explorer.exe", &["ms-settings:display"])];
    #[cfg(target_os = "macos")]
    let tries: &[(&str, &[&str])] = &[
        ("open", &["x-apple.systempreferences:com.apple.Displays-Settings.extension"]),
        ("open", &["/System/Library/PreferencePanes/Displays.prefPane"]),
    ];
    #[cfg(all(unix, not(target_os = "macos")))]
    let tries: &[(&str, &[&str])] = &[
        ("gnome-control-center", &["display"]),
        ("systemsettings", &["kcm_kscreen"]),
        ("kcmshell6", &["kcm_kscreen"]),
        ("xfce4-display-settings", &[]),
        ("arandr", &[]),
    ];
    for (prog, args) in tries {
        if Command::new(prog).args(*args).spawn().is_ok() {
            return Ok(());
        }
    }
    Err("réglages d'affichage introuvables".into())
}

/// Onglet Système, comme celui de JingleStar : appliqué tout de suite, sans relancer la sortie.
fn system(ui: &mut egui::Ui, prefs: &mut MachinePrefs) {
    use star_core::i18n::{tr, Lang};
    section(ui, "Système", |ui| {
        let before = prefs.clone();
        egui::Grid::new("system").num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
            ui.label(tr("Langue", "Language"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut prefs.language, Lang::Fr, "Français");
                ui.selectable_value(&mut prefs.language, Lang::En, "English");
            });
            ui.end_row();

            ui.label(tr("Touche Échap", "Esc key"));
            ui.checkbox(&mut prefs.escape_black, tr("Échap met la sortie au noir", "Esc sends the output to black"))
                .on_hover_text(tr("Sinon, seul le bouton NOIR coupe l'image", "Otherwise only the BLACK button cuts the picture"));
            ui.end_row();

            ui.label(tr("Anti double-GO", "Double-GO protection"));
            ui.add(egui::DragValue::new(&mut prefs.double_go_ms).range(0..=2000).speed(10).suffix(" ms")).on_hover_text(tr(
                "Un second GO dans ce délai est ignoré (0 = désactivé)",
                "A second GO within this delay is ignored (0 = off)",
            ));
            ui.end_row();

            ui.label(tr("Veille", "Sleep"));
            ui.checkbox(&mut prefs.prevent_sleep, tr("Empêcher la mise en veille", "Prevent sleep"))
                .on_hover_text(tr("Ni l'écran ni l'ordinateur ne se mettent en veille tant que CueStar est ouvert", "Neither the screen nor the computer sleeps while CueStar is open"));
            ui.end_row();
        });
        if *prefs != before {
            if prefs.language != before.language {
                star_core::i18n::set_lang(prefs.language);
            }
            prefs.save();
        }
        ui.add_space(6.0);
        // Version de build, pour savoir quelle mise à jour est installée.
        let v = super::build_version();
        if ui
            .add(egui::Label::new(RichText::new(&v).monospace().size(10.0).color(TEXT_DIM)).sense(Sense::click()))
            .on_hover_text(tr("Version de build (clic pour copier)", "Build version (click to copy)"))
            .clicked()
        {
            ui.ctx().copy_text(v);
        }
    });
}

pub enum Action {
    None,
    /// Les réglages moteur ont changé : relancer la sortie.
    Restart,
}

/// Fenêtre du système des Préférences.
pub fn viewport_id() -> egui::ViewportId {
    egui::ViewportId::from_hash_of("cuestar-preferences")
}

impl OutputView {
    /// Fenêtre flottante du système (peut sortir de la régie).
    pub fn show(&mut self, ctx: &egui::Context, prefs: &mut MachinePrefs, out: &mut OutputLink, pattern: &mut Option<PatternKind>) -> Action {
        if !self.open {
            self.sync_guides(prefs, out, false);
            return Action::None;
        }
        let vp = egui::ViewportBuilder::default().with_title("CueStar — Préférences").with_inner_size([1040.0, 660.0]).with_min_inner_size([600.0, 400.0]);
        let mut close = false;
        let action = ctx.show_viewport_immediate(viewport_id(), vp, |ui, _| {
            close = ui.ctx().input(|i| i.viewport().close_requested());
            let typing = ui.ctx().egui_wants_keyboard_input();
            egui::CentralPanel::default()
                .frame(egui::Frame::new().fill(BG).inner_margin(10))
                .show(ui, |ui| self.ui(ui, prefs, out, pattern, typing))
                .inner
        });
        if close {
            self.open = false;
            self.draft = None;
        }
        let editing = self.open && self.tab == Tab::Mapping;
        self.sync_guides(prefs, out, editing);
        action
    }

    fn ui(&mut self, ui: &mut egui::Ui, prefs: &mut MachinePrefs, out: &mut OutputLink, pattern: &mut Option<PatternKind>, typing: bool) -> Action {
        let mut action = Action::None;
        ui.horizontal_wrapped(|ui| {
            for (tab, label) in TABS {
                ui.selectable_value(&mut self.tab, tab, RichText::new(label).size(15.0));
            }
        });
        ui.separator();
        if self.tab == Tab::Log {
            // Défilement propre au journal (collé en bas), pas celui des autres onglets.
            self.log.ui(ui);
        } else {
            egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| match self.tab {
                Tab::Engine => {
                    if self.engine(ui, prefs, out) {
                        action = Action::Restart;
                    }
                }
                Tab::Audio => self.audio(ui, prefs, out),
                Tab::Patterns => self.patterns(ui, out, pattern),
                Tab::Mapping => self.mapping(ui, prefs, out, pattern, typing),
                Tab::System => system(ui, prefs),
                Tab::Log => {}
            });
        }
        action
    }

    /// Écran et moteur. Vrai quand il faut relancer la sortie.
    fn engine(&mut self, ui: &mut egui::Ui, prefs: &mut MachinePrefs, out: &OutputLink) -> bool {
        let mut restart = false;
        let d = self.draft.get_or_insert_with(|| prefs.clone());
        section(ui, "Vidéo", |ui| {
            let s = &out.state;
            if let Some(m) = &s.monitor {
                let mode = if s.fullscreen { "plein écran" } else { "dans une fenêtre" };
                ui.label(RichText::new(format!("Actuellement : {} — {}×{} @ {:.2} Hz, {mode}", crate::display_names::label(&m.name), m.width, m.height, m.refresh_hz)).color(TEXT_DIM));
            }
            egui::Grid::new("engine").num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
                ui.label("Écran de sortie");
                let auto = "Auto (premier écran secondaire)";
                let current = d.screen.as_deref().map(crate::display_names::label).unwrap_or_else(|| auto.into());
                egui::ComboBox::from_id_salt("screen").selected_text(current).width(320.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut d.screen, None, auto);
                    for m in &s.monitors {
                        let txt = format!("{}  {}×{} @ {:.2} Hz{}", crate::display_names::label(&m.name), m.width, m.height, m.refresh_hz, if m.primary { " (principal)" } else { "" });
                        ui.selectable_value(&mut d.screen, Some(m.name.clone()), txt).on_hover_text(&m.name);
                    }
                });
                ui.end_row();

                ui.label("FPS de sortie");
                // Sans choix enregistré, l'écran garde sa fréquence : c'est elle qui est affichée.
                let shown = d.fps.or_else(|| s.monitor.as_ref().map(|m| m.refresh_hz.round()).filter(|hz| *hz > 0.0));
                let text = shown.map(fps_label).unwrap_or_else(|| "—".into());
                egui::ComboBox::from_id_salt("fps").selected_text(text).width(80.0).show_ui(ui, |ui| {
                    for f in FPS_CHOICES {
                        ui.selectable_value(&mut d.fps, Some(f), fps_label(f));
                    }
                })
                .response
                .on_hover_text("Images par seconde de la sortie : l'écran de sortie est passé sur cette fréquence (en Hz) à l'ouverture de la sortie, et la retrouve en quittant CueStar. Windows, macOS, Linux X11 (pas sous Wayland). Noir un instant au changement.");
                ui.end_row();

                ui.label("Avec un seul écran");
                ui.checkbox(&mut d.windowed_if_single, "Sortie dans une fenêtre")
                    .on_hover_text("Quand l'ordinateur n'a qu'un écran (pas de vidéoprojecteur branché) : la sortie s'ouvre dans une fenêtre au lieu de recouvrir la régie en plein écran.");
                ui.end_row();

                ui.label("Décodage");
                egui::ComboBox::from_id_salt("decode").selected_text(d.decode.label()).width(320.0).show_ui(ui, |ui| {
                    for v in Decode::ALL {
                        ui.selectable_value(&mut d.decode, v, v.label());
                    }
                });
                ui.end_row();

                ui.label("Rendu de secours");
                egui::ComboBox::from_id_salt("render").selected_text(d.render.label()).width(320.0).show_ui(ui, |ui| {
                    for v in RenderApi::available() {
                        ui.selectable_value(&mut d.render, *v, v.label());
                    }
                });
                ui.end_row();

                ui.label("Petite machine");
                ui.checkbox(&mut d.light, "Rendu léger");
                ui.end_row();

                ui.label("Synchro de l'image");
                egui::ComboBox::from_id_salt("cadence").selected_text(d.cadence.label()).width(320.0).show_ui(ui, |ui| {
                    for v in Cadence::ALL {
                        ui.selectable_value(&mut d.cadence, v, v.label()).on_hover_text(v.help());
                    }
                })
                .response
                .on_hover_text(d.cadence.help());
                ui.end_row();

                ui.label("");
                ui.checkbox(&mut d.interpolation, "Interpolation (lisse le rythme, ex. 24 i/s sur 60 Hz)")
                    .on_hover_text("Fond deux images voisines quand la cadence ne tombe pas juste. Demande un peu de GPU.");
                ui.end_row();
            });
            ui.label(RichText::new(d.cadence.help()).small().color(TEXT_DIM));
            let dec = if s.hwdec.is_empty() || s.hwdec == "no" { "CPU".to_string() } else { format!("GPU ({})", s.hwdec) };
            ui.label(RichText::new(format!("Décodeur en cours : {dec}")).color(TEXT_DIM));
            if ui
                .button("Réglages d'affichage du système…")
                .on_hover_text("Ouvre les réglages d'écrans du système : résolution, fréquence, disposition, écran étendu")
                .clicked()
            {
                if let Err(e) = open_display_settings() {
                    ui.ctx().debug_text(e);
                }
            }
            let changed = !d.same_engine(prefs);
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                let b = egui::Button::new(RichText::new("Appliquer").color(if changed { PLAYING_INK } else { TEXT_DIM })).fill(if changed { ACCENT } else { PANEL_HI });
                if ui.add_enabled(changed, b).clicked() {
                    // La langue, le mapping, l'audio et l'onglet Système se règlent à chaud :
                    // le brouillon ne les remplace pas.
                    *prefs = MachinePrefs {
                        language: prefs.language,
                        warp: prefs.warp,
                        audio_device: prefs.audio_device.clone(),
                        volume: prefs.volume,
                        audio_delay_ms: prefs.audio_delay_ms,
                        audio_delay_unit: prefs.audio_delay_unit,
                        prevent_sleep: prefs.prevent_sleep,
                        escape_black: prefs.escape_black,
                        double_go_ms: prefs.double_go_ms,
                        ..d.clone()
                    };
                    prefs.save();
                    restart = true;
                }
                if changed {
                    if ui.button("Annuler").clicked() {
                        *d = prefs.clone();
                    }
                    ui.label(RichText::new("Relance la sortie : noir un instant").color(METER_MID));
                }
            });
        });
        restart
    }

    /// Sortie audio et volume général, appliqués tout de suite (sans relancer la sortie).
    fn audio(&mut self, ui: &mut egui::Ui, prefs: &mut MachinePrefs, out: &mut OutputLink) {
        section(ui, "Audio", |ui| {
            let before = (prefs.audio_device.clone(), prefs.volume);
            let before_delay = (prefs.audio_delay_ms, prefs.audio_delay_unit);
            egui::Grid::new("audio").num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
                ui.label("Sortie audio");
                let auto = "Par défaut du système";
                let current = prefs
                    .audio_device
                    .as_ref()
                    .map(|n| out.state.audio_devices.iter().find(|(d, _)| d == n).map(|(_, desc)| desc.clone()).unwrap_or_else(|| n.clone()))
                    .unwrap_or_else(|| auto.into());
                egui::ComboBox::from_id_salt("audio-dev").selected_text(current).width(280.0).show_ui(ui, |ui| {
                    ui.selectable_value(&mut prefs.audio_device, None, auto);
                    for (name, desc) in &out.state.audio_devices {
                        if name == "auto" {
                            continue;
                        }
                        let label = if desc.is_empty() { name.clone() } else { desc.clone() };
                        ui.selectable_value(&mut prefs.audio_device, Some(name.clone()), label).on_hover_text(name);
                    }
                });
                ui.end_row();
                ui.label("Volume");
                ui.add(egui::Slider::new(&mut prefs.volume, 0.0..=100.0).suffix(" %").step_by(1.0));
                ui.end_row();
                ui.label("Délai de sortie");
                ui.horizontal(|ui| {
                    // Les trois unités côte à côte : celle qu'on modifie recalcule les deux autres.
                    let fps = prefs.fps.or_else(|| out.state.monitor.as_ref().map(|m| m.refresh_hz.round()).filter(|hz| *hz > 0.0)).unwrap_or(25.0);
                    for unit in DelayUnit::ALL {
                        let mut v = unit.from_ms(prefs.audio_delay_ms, fps);
                        let (max, speed, decimals) = match unit {
                            DelayUnit::Ms => (2000.0, 1.0, 0),
                            DelayUnit::Frames => (2.0 * fps, 0.1, 1),
                            DelayUnit::Meters => (2.0 * crate::prefs::SOUND_SPEED, 0.1, 1),
                        };
                        let suffix = match unit {
                            DelayUnit::Ms => " ms".to_string(),
                            DelayUnit::Frames => format!(" images (à {fps:.0} i/s)"),
                            DelayUnit::Meters => " m".to_string(),
                        };
                        if ui.add(egui::DragValue::new(&mut v).range(0.0..=max).speed(speed).max_decimals(decimals).suffix(suffix)).changed() {
                            prefs.audio_delay_ms = unit.to_ms(v, fps);
                            prefs.audio_delay_unit = unit;
                        }
                    }
                })
                .response
                .on_hover_text("Retarde le son par rapport à l'image : pour caler un vidéoprojecteur lent, ou des enceintes loin de la scène (343 m/s)");
                ui.end_row();
            });
            if out.state.audio_devices.is_empty() {
                ui.label(RichText::new("La liste des sorties apparaît quand la sortie vidéo est prête.").color(TEXT_DIM));
            }
            ui.label(RichText::new("Carte son, HDMI de l'écran de sortie, interface Dante… S'applique tout de suite.").color(TEXT_DIM));
            if (prefs.audio_device.clone(), prefs.volume) != before {
                if prefs.audio_device != before.0 {
                    out.send(Command::AudioDevice { name: prefs.audio_device.clone() });
                }
                if prefs.volume != before.1 {
                    out.send(Command::Volume { value: prefs.volume });
                }
                prefs.save();
            }
            if (prefs.audio_delay_ms, prefs.audio_delay_unit) != before_delay {
                if prefs.audio_delay_ms != before_delay.0 {
                    out.send(Command::AudioDelay { ms: prefs.audio_delay_ms });
                }
                prefs.save();
            }
        });
    }

    fn patterns(&mut self, ui: &mut egui::Ui, out: &mut OutputLink, pattern: &mut Option<PatternKind>) {
        section(ui, "Mires six25", |ui| self.six25.ui(ui, out, pattern));
        section(ui, "Mires générées", |ui| {
            ui.horizontal_wrapped(|ui| {
                for k in PatternKind::ALL {
                    let sel = *pattern == Some(k);
                    let b = egui::Button::new(RichText::new(k.label()).color(if sel { PLAYING_INK } else { TEXT }))
                        .fill(if sel { ACCENT } else { PANEL_HI })
                        .min_size(vec2(110.0, 30.0));
                    if ui.add(b).clicked() {
                        *pattern = if sel { None } else { Some(k) };
                        out.send(Command::Pattern { kind: *pattern });
                    }
                }
            });
            ui.add_space(4.0);
            if ui.add_enabled(pattern.is_some() || self.six25.playing.is_some(), egui::Button::new("Retirer la mire")).clicked() {
                if pattern.is_some() {
                    *pattern = None;
                    out.send(Command::Pattern { kind: None });
                }
                self.six25.stop(out);
            }
        });
    }

    /// Position à l'écran (fraction) de la poignée `i` : coins, puis points intermédiaires.
    fn handle_pos(w: &Warp, i: usize) -> [f32; 2] {
        if i < 4 {
            w.corners[i]
        } else {
            let n = warp::NODES[i - 4];
            warp::forward(&w.geom(), n[0], n[1])
        }
    }

    /// Déplace la poignée `i` de (dx, dy) en fraction de l'écran, dans les bords de l'écran.
    /// Un coin déplacé emmène ses points intermédiaires (leur décalage reste le même).
    fn move_handle(w: &mut Warp, i: usize, dx: f32, dy: f32) {
        if i < 4 {
            let c = &mut w.corners[i];
            c[0] = (c[0] + dx).clamp(0.0, 1.0);
            c[1] = (c[1] + dy).clamp(0.0, 1.0);
        } else {
            let p = Self::handle_pos(w, i);
            let to = [(p[0] + dx).clamp(0.0, 1.0), (p[1] + dy).clamp(0.0, 1.0)];
            let o = &mut w.mesh_offsets[i - 4];
            o[0] += to[0] - p[0];
            o[1] += to[1] - p[1];
        }
    }

    fn mapping(&mut self, ui: &mut egui::Ui, prefs: &mut MachinePrefs, out: &mut OutputLink, pattern: &mut Option<PatternKind>, typing: bool) {
        let before = prefs.warp;
        let handles = if prefs.warp.mesh { 9 } else { 4 };
        if self.selected >= handles {
            self.selected = 0;
        }
        section(ui, "Mapping 4 coins", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut prefs.warp.enabled, "Activer");
                if ui.button("Réinitialiser").on_hover_text("Image droite : coins aux coins de l'écran, points intermédiaires à leur place").clicked() {
                    prefs.warp.corners = Warp::IDENTITY;
                    prefs.warp.mesh_offsets = [[0.0; 2]; 5];
                }
                ui.checkbox(&mut prefs.warp.mesh, "Points intermédiaires")
                    .on_hover_text("Ajoute un point au milieu de chaque bord et au centre (grille 3 × 3) pour courber l'image, comme le mesh warp de Resolume ou MadMapper");
                if prefs.warp.mesh && prefs.warp.mesh_offsets != [[0.0; 2]; 5] && ui.small_button("Remettre les points au milieu").clicked() {
                    prefs.warp.mesh_offsets = [[0.0; 2]; 5];
                }
            });
            ui.horizontal_wrapped(|ui| {
                ui.label("Bord adouci");
                ui.add(egui::DragValue::new(&mut prefs.warp.feather).range(0.0..=16.0).speed(0.05).suffix(" px").max_decimals(1))
                    .on_hover_text("Fondu du contour de l'image déformée sur quelques pixels : plus de crénelage sur les bords en biais (0 : bord net)");
                ui.separator();
                ui.checkbox(&mut self.guide_grid, "Grille sur la sortie").on_hover_text("Avec les poignées, une grille 8 × 8 suit la déformation de l'image sur la sortie");
                let grid = *pattern == Some(PatternKind::Grid);
                if ui.selectable_label(grid, "Mire grille").on_hover_text("Remplace l'image par la mire grille pendant le réglage").clicked() {
                    *pattern = if grid { None } else { Some(PatternKind::Grid) };
                    out.send(Command::Pattern { kind: *pattern });
                }
            });
            ui.label(
                RichText::new("Les poignées numérotées s'affichent sur la sortie tant que cet onglet est ouvert. Glisse-les ; flèches = 1 pixel, Maj + flèches = 10 pixels sur la poignée choisie. L'image ne peut pas se retourner ni se croiser.")
                    .color(TEXT_DIM),
            );
            if self.refused.is_some_and(|t| t.elapsed() < Duration::from_secs(2)) {
                ui.label(RichText::new("Position refusée : l'image se retournerait ou se croiserait.").color(METER_MID));
            }
            ui.add_space(6.0);

            let (mw, mh) = out.state.monitor.as_ref().map(|m| (m.width.max(1) as f32, m.height.max(1) as f32)).unwrap_or((1920.0, 1080.0));
            let width = ui.available_width().min(720.0);
            let (rect, _) = ui.allocate_exact_size(vec2(width, width * mh / mw), Sense::hover());
            self.editor(ui, rect, &mut prefs.warp);

            // Ajustement fin au clavier de la poignée choisie.
            if !typing {
                let (dx, dy, shift) = ui.input(|i| {
                    let mut d = (0.0f32, 0.0f32);
                    if i.key_pressed(Key::ArrowLeft) {
                        d.0 -= 1.0;
                    }
                    if i.key_pressed(Key::ArrowRight) {
                        d.0 += 1.0;
                    }
                    if i.key_pressed(Key::ArrowUp) {
                        d.1 -= 1.0;
                    }
                    if i.key_pressed(Key::ArrowDown) {
                        d.1 += 1.0;
                    }
                    (d.0, d.1, i.modifiers.shift)
                });
                if dx != 0.0 || dy != 0.0 {
                    let k = if shift { 10.0 } else { 1.0 };
                    Self::move_handle(&mut prefs.warp, self.selected, dx * k / mw, dy * k / mh);
                }
            }

            ui.add_space(6.0);
            egui::Grid::new("corners").num_columns(3).spacing([12.0, 6.0]).show(ui, |ui| {
                for i in 0..handles {
                    let name = if i < 4 { CORNER_NAMES[i] } else { MID_NAMES[i - 4] };
                    if ui.selectable_label(self.selected == i, format!("{}  {name}", i + 1)).clicked() {
                        self.selected = i;
                    }
                    let p = Self::handle_pos(&prefs.warp, i);
                    let (mut x, mut y) = (p[0] * mw, p[1] * mh);
                    let cx = ui.add(egui::DragValue::new(&mut x).range(0.0..=mw).speed(1.0).prefix("x ").suffix(" px")).changed();
                    let cy = ui.add(egui::DragValue::new(&mut y).range(0.0..=mh).speed(1.0).prefix("y ").suffix(" px")).changed();
                    if cx || cy {
                        Self::move_handle(&mut prefs.warp, i, x / mw - p[0], y / mh - p[1]);
                    }
                    ui.end_row();
                }
            });
        });
        if prefs.warp != before {
            // Jamais d'image retournée ou croisée : la modification est refusée.
            if !warp::valid(&prefs.warp.geom()) {
                prefs.warp = before;
                self.refused = Some(Instant::now());
                return;
            }
            // Déplacer une poignée active le mapping.
            if prefs.warp.corners != before.corners || prefs.warp.mesh_offsets != before.mesh_offsets {
                prefs.warp.enabled = true;
            }
            prefs.save();
            self.warp_dirty = true;
        }
    }

    /// Aperçu de l'écran de sortie : contour, grille déformée et poignées numérotées.
    fn editor(&mut self, ui: &mut egui::Ui, rect: Rect, warp: &mut Warp) {
        let p = ui.painter_at(rect.expand(8.0));
        p.rect_filled(rect, 2, Color32::from_rgb(0x10, 0x10, 0x12));
        p.rect_stroke(rect, 2, Stroke::new(1.0, BORDER), egui::StrokeKind::Inside);
        let to_screen = |q: [f32; 2]| pos2(rect.left() + q[0] * rect.width(), rect.top() + q[1] * rect.height());
        let col = if warp.enabled { ACCENT } else { TEXT_DIM };
        let g = warp.geom();

        // Grille 16 × 9 déformée comme l'image.
        let line = |a: (f32, f32), b: (f32, f32)| -> Vec<Pos2> {
            (0..=32)
                .map(|t| {
                    let t = t as f32 / 32.0;
                    to_screen(warp::forward(&g, a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t))
                })
                .collect()
        };
        for i in 1..16 {
            let u = i as f32 / 16.0;
            p.add(egui::Shape::line(line((u, 0.0), (u, 1.0)), Stroke::new(1.0, col.gamma_multiply(0.3))));
        }
        for i in 1..9 {
            let v = i as f32 / 9.0;
            p.add(egui::Shape::line(line((0.0, v), (1.0, v)), Stroke::new(1.0, col.gamma_multiply(0.3))));
        }
        let mut outline = line((0.0, 0.0), (1.0, 0.0));
        outline.extend(line((1.0, 0.0), (1.0, 1.0)));
        outline.extend(line((1.0, 1.0), (0.0, 1.0)));
        outline.extend(line((0.0, 1.0), (0.0, 0.0)));
        p.add(egui::Shape::closed_line(outline, Stroke::new(2.0, col)));

        let n = if warp.mesh { 9 } else { 4 };
        for i in 0..n {
            let pos = to_screen(Self::handle_pos(warp, i));
            let id = ui.scope_id().with(("corner", i));
            let resp = ui.interact(Rect::from_center_size(pos, vec2(22.0, 22.0)), id, Sense::click_and_drag());
            if resp.clicked() || resp.drag_started() {
                self.selected = i;
            }
            if resp.dragged() {
                let d = resp.drag_delta();
                Self::move_handle(warp, i, d.x / rect.width(), d.y / rect.height());
            }
            let hot = resp.hovered() || resp.dragged() || self.selected == i;
            let r = if i < 4 { 6.0 } else { 5.0 };
            p.circle_filled(pos, if hot { r + 2.0 } else { r }, if self.selected == i { ACCENT } else { TEXT });
            p.circle_stroke(pos, r + 3.0, Stroke::new(1.0, PLAYING_INK));
            // Numéro vers l'intérieur de l'écran, comme sur la sortie.
            let off = vec2(if pos.x < rect.center().x { 12.0 } else { -12.0 }, if pos.y < rect.center().y { 12.0 } else { -12.0 });
            p.text(pos + off, egui::Align2::CENTER_CENTER, format!("{}", i + 1), egui::FontId::proportional(11.0), if self.selected == i { ACCENT } else { TEXT_DIM });
        }
    }

    /// Repères sur la sortie : affichés tant que l'onglet Mapping est ouvert, retirés sinon.
    fn sync_guides(&mut self, prefs: &MachinePrefs, out: &mut OutputLink, visible: bool) {
        let want = visible.then_some(WarpGuides { selected: self.selected, grid: self.guide_grid });
        if want != self.guides_sent {
            self.guides_sent = want;
            self.warp_dirty = true;
            self.last_warp_send = Instant::now() - Duration::from_secs(1);
        }
        // Envoi limité pendant un glisser : mpv recompile le shader à chaque envoi (affichage de secours).
        if self.warp_dirty && self.last_warp_send.elapsed() >= Duration::from_millis(40) {
            out.send(Command::Warp { geom: prefs.warp.active(), guides: self.guides_sent });
            self.last_warp_send = Instant::now();
            self.warp_dirty = false;
        }
    }
}
