//! Interface de SweepStar, à l'allure de JingleStar : barre du haut (logo, onglets, ARRÊTER,
//! Préférences, Aide), panneau gauche (signal, entrées, sweep, traces), vue centrale, barre
//! d'état. Les Préférences et l'Aide sont des fenêtres flottantes.

mod measures_view;
mod playlist_view;
mod plot;
mod prefs_view;
mod rta_view;
mod sweep_view;
mod transfer_view;

use std::time::{Duration, Instant};

use egui::{vec2, Align, Color32, CornerRadius, Key, Layout, Rect, RichText, Sense, Stroke};
use star_core::i18n::tr;
use star_ui::dialogs::{Dialogs, Filter, Kind};
use star_ui::icons::{self, Icon};
use star_ui::theme::{self, *};
use star_ui::app_bar;
use star_ui::widgets::segmented;

use crate::controller::{Controller, SignalKind, APP};

const AIDE_FR: &str = include_str!("../../docs/aide.md");
const HELP_EN: &str = include_str!("../../docs/help.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Sweep,
    Transfer,
    Rta,
}

/// Que faire du fichier choisi.
#[derive(Debug, Clone, PartialEq)]
pub enum Pick {
    /// Export WAV de l'IR : index de trace (`None` : la dernière mesure).
    Wav(Option<usize>),
    Rew(Option<usize>),
    /// Fichiers à ajouter à la playlist.
    AddFiles,
    /// Fichiers de mesure à importer.
    ImportTraces,
    /// Dossier des mesures.
    MeasuresDir,
}

pub struct SweepStarApp {
    pub ctrl: Controller,
    tab: Tab,
    help: star_ui::help::HelpWindow,
    prefs: Option<prefs_view::PrefsDraft>,
    dialogs: Dialogs<Pick>,
    sweep: sweep_view::SweepView,
    rta: rta_view::RtaView,
    transfer: transfer_view::TransferView,
    /// Panneau des réglages (signal, entrées, sweep) déplié.
    show_settings: bool,
    last_frame: Instant,
    show_playlist: bool,
    /// Panneau des mesures déplié.
    show_measures: bool,
}

impl SweepStarApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        if let Some(p) = APP.config_file("fenetres.json") {
            star_ui::floating::remember_in(p);
        }
        let ctx = cc.egui_ctx.clone();
        Self {
            ctrl: Controller::new(move || ctx.request_repaint()),
            tab: Tab::Sweep,
            help: star_ui::help::HelpWindow::new(AIDE_FR, HELP_EN).app("SweepStar").version(crate::VERSION),
            prefs: None,
            dialogs: Dialogs::default(),
            sweep: sweep_view::SweepView::default(),
            rta: rta_view::RtaView::default(),
            transfer: transfer_view::TransferView::default(),
            show_settings: true,
            last_frame: Instant::now(),
            show_playlist: false,
            show_measures: true,
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            // Barre commune : logo, nom, version et onglets.
            app_bar::identity(ui, app_bar::Logo::Impulse, "SweepStar", false);
            ui.add_space(16.0);
            app_bar::tabs(ui, &mut self.tab, &[(Tab::Sweep, tr("Sweep et IR", "Sweep & IR")), (Tab::Transfer, tr("Transfert", "Transfer")), (Tab::Rta, "RTA")], app_bar::TAB_WIDTH);

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let stop = egui::Button::new((icons::atom_colored(Icon::Stop, Color32::WHITE), RichText::new(tr("ARRÊTER", "STOP")).strong().color(Color32::WHITE)))
                    .fill(STOP)
                    .min_size(vec2(130.0, 32.0));
                let stop = ui.add(stop).on_hover_text(tr("Arrête le signal et la mesure (Échap)", "Stops the signal and the measurement (Esc)"));
                if star_ui::widgets::pressed(ui, &stop) {
                    self.ctrl.stop_all();
                }
                ui.add_space(8.0);
                // Boutons à icône seule (barre commune) : le nom est dans l'infobulle.
                if app_bar::icon_button(ui, Icon::Book, self.help.open, tr("Aide (F1)", "Help (F1)")).clicked() {
                    self.help.open_or_raise(ui.ctx());
                }
                if app_bar::icon_button(ui, Icon::Gear, self.prefs.is_some(), tr("Préférences", "Preferences")).clicked() {
                    self.open_prefs(ui.ctx());
                }
                if ui
                    .selectable_label(self.show_playlist, (icons::atom(Icon::Note), "Playlist"))
                    .on_hover_text(tr("Fichiers de test pour le soundcheck", "Test files for the soundcheck"))
                    .clicked()
                {
                    self.show_playlist = !self.show_playlist;
                }
            });
        });
    }

    fn open_prefs(&mut self, ctx: &egui::Context) {
        if self.prefs.is_some() {
            star_ui::floating::raise(ctx, prefs_view::viewport());
            return;
        }
        self.prefs = Some(prefs_view::PrefsDraft::new(&self.ctrl.prefs.audio));
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            match &self.ctrl.device {
                Some(d) => {
                    icons::icon_only(ui, Icon::Dot, if d.input.is_some() { GO } else { ACCENT });
                    let input = d.input.clone().unwrap_or_else(|| tr("aucune entrée", "no input").into());
                    ui.label(format!("{} · {} → {} · {} Hz", d.host, d.output, input, d.sample_rate));
                }
                None => {
                    icons::icon_only(ui, Icon::Dot, STOP);
                    ui.label(tr("Aucun périphérique audio", "No audio device"));
                }
            }
            if self.ctrl.is_opening() {
                ui.spinner();
                ui.label(RichText::new(tr("Ouverture des périphériques…", "Opening devices…")).color(TEXT_DIM));
            }
            if let Some(e) = &self.ctrl.device_error {
                icons::label(ui, Icon::Warning, STOP, RichText::new(e.to_string()).color(STOP));
            }
            if let Some(s) = &self.ctrl.status {
                ui.label(RichText::new(s).color(TEXT_DIM));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let n = self.ctrl.overruns;
                let col = if n > 0 { STOP } else { TEXT_DIM };
                ui.label(RichText::new(format!("{} : {n}", tr("Pertes", "Drops"))).monospace().color(col))
                    .on_hover_text(tr("Blocs d'entrée perdus (tampon plein) depuis le lancement", "Input blocks lost (buffer full) since launch"));
            });
        });
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            section(ui, tr("SIGNAL", "SIGNAL"));
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("«").on_hover_text(tr("Replier le panneau", "Collapse the panel")).clicked() {
                    self.show_settings = false;
                }
            });
        });
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover());
            let kinds = [SignalKind::Pink, SignalKind::PeriodicPink, SignalKind::White, SignalKind::Sine];
            let labels = [tr("Rose", "Pink"), tr("Rose pér.", "Per. pink"), tr("Blanc", "White"), tr("Sinus", "Sine")];
            let sel = kinds.iter().position(|k| *k == self.ctrl.prefs.signal).unwrap_or(0);
            if let Some(i) = segmented(ui, rect, "signal", &labels, sel) {
                self.ctrl.prefs.signal = kinds[i];
                if self.ctrl.playing() && self.ctrl.measuring.is_none() {
                    self.ctrl.play_signal();
                }
            }
            if self.ctrl.prefs.signal == SignalKind::PeriodicPink {
                ui.label(RichText::new(tr("Bruit rose périodique de 65 536 points : spectre exact pour le transfert", "65,536-point periodic pink noise: exact spectrum for transfer")).small().color(TEXT_DIM));
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().slider_width = (ui.available_width() - 140.0).max(80.0);
                ui.label(tr("Niveau", "Level"));
                let mut db = self.ctrl.prefs.level_db;
                if ui.add(egui::Slider::new(&mut db, -60.0..=0.0).suffix(" dBFS").step_by(0.5)).changed() {
                    self.ctrl.set_level(db);
                }
            });
            if self.ctrl.prefs.signal == SignalKind::Sine {
                ui.horizontal(|ui| {
                    ui.label(tr("Fréquence", "Frequency"));
                    let mut hz = self.ctrl.prefs.sine_hz;
                    if ui.add(egui::DragValue::new(&mut hz).range(10.0..=20_000.0).speed(5.0).suffix(" Hz")).changed() {
                        self.ctrl.set_sine_hz(hz);
                    }
                });
            }
            ui.horizontal(|ui| {
                let playing = self.ctrl.playing() && self.ctrl.measuring.is_none();
                let play = egui::Button::new((icons::atom_colored(Icon::Play, if playing { PLAYING_INK } else { GO }), tr("Jouer", "Play")))
                    .fill(if playing { GO } else { PANEL_HI })
                    .min_size(vec2(100.0, 28.0));
                if ui.add_enabled(self.ctrl.measuring.is_none(), play).clicked() {
                    self.ctrl.play_signal();
                }
                if ui.add(egui::Button::new((icons::atom(Icon::Stop), tr("Arrêter", "Stop"))).min_size(vec2(90.0, 28.0))).on_hover_text("Échap").clicked() {
                    self.ctrl.stop_all();
                }
            });
            meter_row(ui, tr("Sortie", "Output"), self.ctrl.out_meter);

            ui.add_space(8.0);
            section(ui, tr("ENTRÉES", "INPUTS"));
            let a = &self.ctrl.prefs.audio;
            match a.ref_channel {
                Some(r) => meter_row(ui, &format!("{} (in {})", tr("Boucle", "Loop"), r + 1), self.ctrl.in_meters[r.min(31)]),
                None => {
                    ui.label(RichText::new(tr("Pas de boucle de référence (Préférences)", "No reference loop (Preferences)")).small().color(TEXT_DIM));
                }
            }
            meter_row(ui, &format!("{} (in {})", tr("Micro", "Mic"), a.mic_channel + 1), self.ctrl.in_meters[a.mic_channel.min(31)]);

        });
    }

    fn on_picked(&mut self, pick: Pick, files: Vec<std::path::PathBuf>) {
        match pick {
            Pick::AddFiles => {
                if !files.is_empty() {
                    self.ctrl.add_files(files);
                }
                return;
            }
            Pick::ImportTraces => {
                if !files.is_empty() {
                    self.ctrl.import_traces(files);
                }
                return;
            }
            Pick::MeasuresDir => {
                if let Some(d) = files.into_iter().next() {
                    self.ctrl.set_measures_dir(d);
                }
                return;
            }
            Pick::Wav(_) | Pick::Rew(_) => {}
        }
        let Some(path) = files.into_iter().next() else { return };
        let (idx, wav) = match pick {
            Pick::Wav(i) => (i, true),
            Pick::Rew(i) => (i, false),
            Pick::AddFiles | Pick::ImportTraces | Pick::MeasuresDir => return,
        };
        let (m, name) = match idx {
            Some(i) => {
                if let Some(t) = self.ctrl.traces.get_mut(i) {
                    if let Err(e) = t.load() {
                        self.ctrl.status = Some(format!("{} : {e}", tr("mesure illisible", "unreadable measurement")));
                        return;
                    }
                }
                match self.ctrl.traces.get(i).and_then(|t| t.measurement().map(|m| (m.clone(), t.name.clone()))) {
                    Some(v) => v,
                    None => return,
                }
            }
            None => match &self.ctrl.current {
                Some(m) => (m.clone(), tr("Dernière mesure", "Last measurement").to_string()),
                None => return,
            },
        };
        let res = if wav {
            crate::export::write_wav_f32(&path, m.sample_rate as u32, &m.ir)
        } else {
            let pts = sweep_view::rew_points(&m, self.ctrl.prefs.smoothing);
            std::fs::write(&path, crate::export::rew_text(&name, &pts)).map_err(Into::into)
        };
        self.ctrl.status = Some(match res {
            Ok(()) => format!("{} {}", tr("Exporté :", "Exported:"), path.display()),
            Err(e) => format!("{} : {e}", tr("Export impossible", "Export failed")),
        });
    }

    fn frame(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let dt = self.last_frame.elapsed().as_secs_f32();
        self.last_frame = Instant::now();
        self.ctrl.poll(dt);
        if let Some((pick, files)) = self.dialogs.take() {
            self.on_picked(pick, files);
        }
        // Échap arrête tout, même pendant une saisie.
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.ctrl.stop_all();
        }
        if ctx.input(|i| i.key_pressed(Key::F1)) {
            self.help.toggle();
        }
        // Fichiers ou dossiers déposés depuis l'explorateur : dans la playlist.
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        // Les fichiers de mesure (.ssm) vont dans le panneau des mesures.
        let (measures, audio): (Vec<_>, Vec<_>) = dropped.into_iter().partition(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case(crate::traces::EXTENSION)));
        if !measures.is_empty() {
            self.ctrl.import_traces(measures);
            self.show_measures = true;
        }
        if !audio.is_empty() {
            self.ctrl.add_files(audio);
            self.show_playlist = true;
        }

        star_ui::app_bar::show(ui, |ui| self.toolbar(ui));
        egui::Panel::bottom("status")
            .exact_size(26.0)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(12, 4)))
            .show(ui, |ui| self.status_bar(ui));
        if self.show_settings {
            egui::Panel::left("reglages")
                .resizable(true)
                .default_size(290.0)
                .min_size(250.0)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(10))
                .show(ui, |ui| self.left_panel(ui));
        } else {
            egui::Panel::left("reglages-replie")
                .exact_size(30.0)
                .resizable(false)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(2, 10)))
                .show(ui, |ui| {
                    if ui.vertical_centered(|ui| ui.small_button("»").on_hover_text(tr("Afficher le signal et le sweep", "Show signal and sweep")).clicked()).inner {
                        self.show_settings = true;
                    }
                });
        }
        if self.tab != Tab::Transfer {
            self.ctrl.run_transfer(false);
        }
        if self.show_measures {
            let mut action = None;
            egui::Panel::right("mesures")
                .resizable(true)
                .default_size(270.0)
                .min_size(220.0)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(10))
                .show(ui, |ui| action = measures_view::show(ui, &mut self.ctrl, self.tab, &self.rta, &self.transfer));
            let ssm = || Filter::new(tr("Mesures SweepStar", "SweepStar measurements"), &[crate::traces::EXTENSION]);
            let save = |name: &str, ext: &str| Kind::Save(Filter::new(if ext == "wav" { "WAV".to_string() } else { tr("Texte", "Text").to_string() }, &[ext]), format!("{name}.{ext}"));
            let name = |c: &Controller, i: Option<usize>| i.and_then(|i| c.traces.get(i)).map_or_else(|| "sweepstar".to_string(), |t| t.name.clone());
            match action {
                Some(measures_view::MeasuresAction::Collapse) => self.show_measures = false,
                Some(measures_view::MeasuresAction::Import) => self.dialogs.show(&ctx, Pick::ImportTraces, Kind::OpenMany(ssm())),
                Some(measures_view::MeasuresAction::ChooseDir) => self.dialogs.show(&ctx, Pick::MeasuresDir, Kind::Folder),
                Some(measures_view::MeasuresAction::Wav(i)) => self.dialogs.show(&ctx, Pick::Wav(i), save(&name(&self.ctrl, i), "wav")),
                Some(measures_view::MeasuresAction::Rew(i)) => self.dialogs.show(&ctx, Pick::Rew(i), save(&name(&self.ctrl, i), "txt")),
                None => {}
            }
        } else {
            egui::Panel::right("mesures-replie")
                .exact_size(30.0)
                .resizable(false)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(egui::Margin::symmetric(2, 10)))
                .show(ui, |ui| {
                    if measures_view::collapsed(ui) {
                        self.show_measures = true;
                    }
                });
        }
        if self.show_playlist {
            let mut action = None;
            egui::Panel::right("playlist")
                .resizable(true)
                .default_size(320.0)
                .min_size(240.0)
                .frame(egui::Frame::new().fill(PANEL).inner_margin(10))
                .show(ui, |ui| action = playlist_view::show(ui, &mut self.ctrl));
            if let Some(playlist_view::PlaylistAction::AddFiles) = action {
                let exts: Vec<&str> = crate::audio::decode::AUDIO_EXTENSIONS.to_vec();
                self.dialogs.show(&ctx, Pick::AddFiles, Kind::OpenMany(Filter::new(tr("Fichiers audio", "Audio files"), &exts)));
            }
        }
        egui::CentralPanel::default().frame(egui::Frame::new().fill(BG).inner_margin(10)).show(ui, |ui| match self.tab {
            Tab::Sweep => self.sweep.show(ui, &mut self.ctrl),
            Tab::Rta => self.rta.show(ui, &mut self.ctrl),
            Tab::Transfer => self.transfer.show(ui, &mut self.ctrl),
        });

        if let Some(draft) = &mut self.prefs {
            if !prefs_view::show(&ctx, draft, &mut self.ctrl) {
                self.prefs = None;
            }
        }
        self.help.show(&ctx);

        let busy = self.ctrl.playing() || self.ctrl.is_loading_track() || self.ctrl.measuring.is_some() || self.tab != Tab::Sweep || self.ctrl.in_meters.iter().any(|d| *d > -100.0);
        ctx.request_repaint_after(Duration::from_millis(if busy { 33 } else { 250 }));
    }
}

impl eframe::App for SweepStarApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.frame(ui);
    }

    fn on_exit(&mut self) {
        self.ctrl.stop_all();
        self.ctrl.flush_traces();
        self.ctrl.save_prefs();
        self.ctrl.save_session();
        star_ui::floating::save_places();
    }
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.label(RichText::new(title).small().strong().color(ACCENT));
}

/// Vu-mètre horizontal, −60 à 0 dBFS, avec la valeur en dB.
fn meter_row(ui: &mut egui::Ui, label: &str, db: f32) {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(vec2(96.0, 16.0), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_width(96.0);
            ui.add(egui::Label::new(RichText::new(label).small()).truncate());
        });
        let w = (ui.available_width() - 54.0).max(40.0);
        let (rect, _) = ui.allocate_exact_size(vec2(w, 12.0), Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(2), Color32::from_rgb(0x12, 0x12, 0x14));
        let frac = |d: f32| ((d + 60.0) / 60.0).clamp(0.0, 1.0);
        let level = frac(db);
        for (lo, hi, col) in [(0.0, frac(-12.0), METER_LOW), (frac(-12.0), frac(-3.0), METER_MID), (frac(-3.0), 1.0, METER_HIGH)] {
            let top = level.min(hi);
            if top > lo {
                let r = Rect::from_min_max(egui::pos2(rect.left() + lo * rect.width(), rect.top() + 1.0), egui::pos2(rect.left() + top * rect.width(), rect.bottom() - 1.0));
                p.rect_filled(r, CornerRadius::ZERO, col);
            }
        }
        for d in [-6.0, -12.0, -24.0, -40.0] {
            let x = rect.left() + frac(d) * rect.width();
            p.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], Stroke::new(1.0, Color32::from_black_alpha(140)));
        }
        let txt = if db <= -99.0 { "−∞".to_string() } else { format!("{db:.1}") };
        ui.label(RichText::new(txt).monospace().small().color(if db > -1.0 { STOP } else { TEXT_DIM }));
    });
}
