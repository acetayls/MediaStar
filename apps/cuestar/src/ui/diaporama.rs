//! Module Diaporama : reprise du projet « Diaporama » (Electron) sur le moteur CueStar.
//!
//! v1 : dossier (sous-dossiers), ordre aléatoire remélangé à chaque tour, boucle, durée des
//! photos, ajustement (image entière, remplir, auto), vidéos lues en entier avec ou sans son.
//! Le minutage et l'enchaînement tournent dans le moteur de sortie.
//! À venir : transitions, Ken Burns, fond flou, musique de fond, commandes OSC.

use crate::link::OutputLink;
use crate::protocol::{Command, Fit};
use egui::{vec2, Color32, Key, Rect, RichText, Sense};
use star_ui::theme::*;
use star_ui::widgets::{mode_chip, pressed, transport_button, TransportIcon};
use std::path::{Path, PathBuf};

const MEDIA_EXT: &[&str] = &[
    "jpg", "jpeg", "png", "bmp", "gif", "tif", "tiff", "webp", "heic", "mp4", "mov", "mkv", "avi", "webm", "m4v", "mpg", "mpeg",
    "ts", "wmv",
];

type Settings = crate::project::DiaporamaData;

pub struct Diaporama {
    s: Settings,
    files: Vec<PathBuf>,
    /// Dernier dépôt : message affiché au-dessus de la liste (médias ajoutés, fichiers ignorés).
    drop_note: Option<String>,
}

impl Default for Diaporama {
    fn default() -> Self {
        Self { s: Settings::default(), files: Vec::new(), drop_note: None }
    }
}

fn is_media(p: &Path) -> bool {
    p.extension().map(|e| MEDIA_EXT.contains(&e.to_string_lossy().to_lowercase().as_str())).unwrap_or(false)
}

/// « 1 média ajouté », « 3 médias ajoutés ».
fn plural(n: usize, word: &str, tail: &str) -> String {
    let s = if n > 1 { "s" } else { "" };
    format!("{n} {word}{s}{}", if tail.is_empty() { String::new() } else { format!(" {tail}{s}") })
}

fn scan(dir: &Path, recursive: bool, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort_by_key(|p| p.to_string_lossy().to_lowercase());
    for p in entries {
        if p.is_dir() {
            if recursive {
                scan(&p, true, out);
            }
        } else if is_media(&p) {
            out.push(p);
        }
    }
}

impl Diaporama {
    pub fn to_data(&self) -> Settings {
        self.s.clone()
    }

    /// Projet ouvert : réglages remplacés, dossier relu.
    pub fn load_data(&mut self, d: &Settings) {
        self.s = d.clone();
        self.rescan();
    }

    /// Commande OSC « diaporama/… » (mêmes adresses que le Diaporama d'origine).
    pub fn handle_osc(&mut self, cmd: &str, m: &crate::osc::Msg, out: &mut OutputLink) {
        let running = out.state.slide.is_some();
        match cmd {
            "play" => {
                if running {
                    out.send(Command::Play)
                } else {
                    self.start(out)
                }
            }
            "pause" => out.send(Command::Pause),
            "toggle" => {
                if !running {
                    self.start(out)
                } else {
                    out.send(if out.state.paused { Command::Play } else { Command::Pause })
                }
            }
            "stop" => out.send(Command::Stop),
            "next" => out.send(Command::Next),
            "prev" => out.send(Command::Prev),
            "goto" => {
                if let Some(i) = m.f64().filter(|v| *v >= 1.0) {
                    out.send(Command::Goto { index: i as usize - 1 });
                }
            }
            "settings/shuffle" => self.s.shuffle = m.flag(),
            "settings/loop" => self.s.looping = m.flag(),
            "settings/recursive" => {
                self.s.recursive = m.flag();
                self.rescan();
            }
            "settings/video_sound" => self.s.video_sound = m.flag(),
            "settings/photo_duration" => {
                if let Some(v) = m.f64() {
                    self.s.photo_seconds = v.clamp(1.0, 60.0);
                }
            }
            "settings/fit" => {
                let fit = match m.text().unwrap_or_default().to_lowercase().as_str() {
                    "contain" => Some(Fit::Contain),
                    "cover" => Some(Fit::Cover),
                    "auto" => Some(Fit::Auto),
                    _ => None,
                };
                if let Some(f) = fit {
                    self.s.fit = f;
                    if running {
                        out.send(Command::SetFit { fit: f });
                    }
                }
            }
            "settings/folder" => {
                if let Some(p) = m.text() {
                    self.s.folder = Some(p.into());
                    self.rescan();
                }
            }
            _ => {}
        }
        if running && cmd.starts_with("settings/") {
            out.send(Command::SlideshowSettings {
                photo_seconds: self.s.photo_seconds,
                looping: self.s.looping,
                shuffle: self.s.shuffle,
                video_sound: self.s.video_sound,
            });
        }
    }

    /// Relit le dossier principal puis les dossiers et fichiers déposés, sans doublon.
    fn rescan(&mut self) {
        self.files.clear();
        if let Some(f) = self.s.folder.clone() {
            scan(&f, self.s.recursive, &mut self.files);
        }
        for p in &self.s.extra {
            if p.is_dir() {
                scan(p, self.s.recursive, &mut self.files);
            } else if p.is_file() && is_media(p) {
                self.files.push(p.clone());
            }
        }
        let mut seen = std::collections::HashSet::new();
        self.files.retain(|p| seen.insert(p.to_string_lossy().to_lowercase()));
    }

    /// Fichiers ou dossiers glissés depuis l'Explorateur : ajoutés à la liste.
    /// Un dossier déposé alors qu'aucun n'est choisi devient le dossier du diaporama.
    fn handle_drops(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        if dropped.is_empty() {
            return;
        }
        let (mut kept, mut decks, mut ignored) = (0, 0, 0);
        for p in dropped {
            if p.is_dir() || is_media(&p) {
                kept += 1;
                if p.is_dir() && self.s.folder.is_none() {
                    self.s.folder = Some(p);
                } else if self.s.folder.as_ref() != Some(&p) && !self.s.extra.contains(&p) {
                    self.s.extra.push(p);
                }
            } else if p.extension().map(|e| matches!(e.to_string_lossy().to_lowercase().as_str(), "pptx" | "ppsx" | "pdf")).unwrap_or(false) {
                decks += 1;
            } else {
                ignored += 1;
            }
        }
        let before = self.files.len();
        self.rescan();
        let mut parts = Vec::new();
        if kept > 0 {
            parts.push(plural(self.files.len().saturating_sub(before), "média", "ajouté"));
        }
        if decks > 0 {
            parts.push("PowerPoint et PDF : à déposer dans l'onglet Présentation".into());
        }
        if ignored > 0 {
            parts.push(format!("{} (format non pris en charge)", plural(ignored, "fichier", "ignoré")));
        }
        self.drop_note = (!parts.is_empty()).then(|| parts.join(" · "));
    }

    fn start(&self, out: &mut OutputLink) {
        out.send(Command::Slideshow {
            items: self.files.iter().map(|p| p.to_string_lossy().into_owned()).collect(),
            photo_seconds: self.s.photo_seconds,
            looping: self.s.looping,
            shuffle: self.s.shuffle,
            fit: self.s.fit,
            video_sound: self.s.video_sound,
        });
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, out: &mut OutputLink, typing: bool) {
        let ctx = ui.ctx().clone();
        self.handle_drops(&ctx);
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let running = out.state.slide.is_some();
        if !typing {
            if ui.input_mut(|i| i.consume_key(egui::Modifiers::NONE, Key::Space)) {
                if !running {
                    self.start(out);
                } else {
                    out.send(if out.state.paused { Command::Play } else { Command::Pause });
                }
            }
            if running && ui.input(|i| i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::ArrowDown)) {
                out.send(Command::Next);
            }
            if running && ui.input(|i| i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::ArrowUp)) {
                out.send(Command::Prev);
            }
        }

        // En cours et transport, avec les boutons de la playlist de JingleStar (comme le Lecteur).
        let width = ui.available_width().min(640.0);
        let paused = running && out.state.paused;
        egui::Frame::new().fill(Color32::from_rgb(0x1c, 0x1c, 0x20)).corner_radius(4).inner_margin(6).show(ui, |ui| {
            ui.set_width(width - 12.0);
            let col = if paused { ACCENT } else if running { GO } else { TEXT_DIM };
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if paused {
                        ui.label(RichText::new("EN PAUSE").strong().small().color(ACCENT));
                    }
                    let count = match &out.state.slide {
                        Some((i, n, _)) => format!("{} / {}", i + 1, n),
                        None => format!("{} médias", self.files.len()),
                    };
                    ui.label(RichText::new(count).monospace().color(TEXT_DIM));
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        let name = match &out.state.slide {
                            Some((_, _, p)) => Path::new(p).file_stem().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
                            None => "Diaporama arrêté".into(),
                        };
                        ui.add(egui::Label::new(RichText::new(name).strong().size(15.0).color(col)).truncate());
                    });
                });
            });
        });
        let (row, _) = ui.allocate_exact_size(vec2(width, 38.0), Sense::hover());
        let gap = 4.0;
        let k = ((row.width() - 4.0 * gap) / (2.0 * 40.0 + 2.0 * 48.0 + 64.0)).min(1.0);
        let (side, stop_w) = (40.0 * k, 48.0 * k);
        let go_w = row.width() - 2.0 * side - 2.0 * stop_w - 4.0 * gap;
        let mut x = row.left();
        let mut cell = |w: f32| {
            let r = Rect::from_min_size(egui::pos2(x, row.top()), vec2(w, row.height()));
            x += w + gap;
            r
        };
        let (prev_r, go_r, pause_r, stop_r, next_r) = (cell(side), cell(go_w), cell(stop_w), cell(stop_w), cell(side));
        if transport_button(ui, prev_r, "dia-prev", TransportIcon::Prev, None, PANEL_HI, TEXT).on_hover_text("Média précédent (←)").clicked() && running {
            out.send(Command::Prev);
        }
        let go_label = if paused { "REPRENDRE" } else if running { "RELANCER" } else { "LANCER" };
        let go = transport_button(ui, go_r, "dia-go", TransportIcon::Play, Some(go_label), GO, Color32::from_rgb(0x0c, 0x1a, 0x0e))
            .on_hover_text(if paused { "Reprendre le diaporama" } else if running { "Relancer le diaporama depuis le début" } else { "Lancer le diaporama sur la sortie (Espace)" });
        if pressed(ui, &go) {
            if paused {
                out.send(Command::Play);
            } else {
                self.start(out);
            }
        }
        let (pause_fill, pause_fg) = if paused { (ACCENT, Color32::BLACK) } else { (PANEL_HI, if running { TEXT } else { TEXT_DIM }) };
        let pause = transport_button(ui, pause_r, "dia-pause", TransportIcon::Pause, None, pause_fill, pause_fg).on_hover_text("Pause / reprise (Espace)");
        if pressed(ui, &pause) && running {
            out.send(if paused { Command::Play } else { Command::Pause });
        }
        let stop = transport_button(ui, stop_r, "dia-stop", TransportIcon::Stop, None, STOP.gamma_multiply(0.75), Color32::WHITE).on_hover_text("Arrêter : écran noir (Échap)");
        if pressed(ui, &stop) {
            out.send(Command::Stop);
        }
        if transport_button(ui, next_r, "dia-next", TransportIcon::Next, None, PANEL_HI, TEXT).on_hover_text("Média suivant (→)").clicked() && running {
            out.send(Command::Next);
        }
        // Modes, comme dans le Lecteur : aléatoire et boucle.
        let (row, _) = ui.allocate_exact_size(vec2(width, 26.0), Sense::hover());
        let half = (row.width() - 4.0) / 2.0;
        let shuffle_r = Rect::from_min_size(row.min, vec2(half, row.height()));
        let loop_r = Rect::from_min_size(egui::pos2(row.left() + half + 4.0, row.top()), vec2(half, row.height()));
        let mut modes_changed = false;
        if mode_chip(ui, shuffle_r, "dia-shuffle", self.s.shuffle, "Aléatoire").on_hover_text("Ordre aléatoire, remélangé à chaque tour").clicked() {
            self.s.shuffle = !self.s.shuffle;
            modes_changed = true;
        }
        if mode_chip(ui, loop_r, "dia-loop", self.s.looping, "Tout boucler").on_hover_text("Reprendre au début à la fin du dossier").clicked() {
            self.s.looping = !self.s.looping;
            modes_changed = true;
        }
        if modes_changed && running {
            out.send(Command::SlideshowSettings {
                photo_seconds: self.s.photo_seconds,
                looping: self.s.looping,
                shuffle: self.s.shuffle,
                video_sound: self.s.video_sound,
            });
        }        ui.add_space(10.0);

        let before = (self.s.clone(), self.s.fit);
        egui::Frame::new().fill(PANEL).corner_radius(4).inner_margin(12).show(ui, |ui| {
            ui.set_width(ui.available_width());
            egui::Grid::new("diapo").num_columns(2).spacing([16.0, 10.0]).show(ui, |ui| {
                ui.label("Dossier");
                ui.horizontal(|ui| {
                    let txt = self.s.folder.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| "aucun".into());
                    ui.label(RichText::new(txt).color(if self.s.folder.is_some() { TEXT } else { TEXT_DIM }));
                    if ui.button("Choisir…").clicked() {
                        if let Some(d) = rfd::FileDialog::new().pick_folder() {
                            self.s.folder = Some(d);
                            self.rescan();
                        }
                    }
                    if ui.button("Relire").on_hover_text("Relit les dossiers (fichiers ajoutés ou retirés)").clicked() {
                        self.rescan();
                    }
                });
                ui.end_row();

                if !self.s.extra.is_empty() {
                    ui.label("Ajoutés");
                    ui.horizontal(|ui| {
                        let dirs = self.s.extra.iter().filter(|p| p.is_dir()).count();
                        let files = self.s.extra.len() - dirs;
                        let mut parts = Vec::new();
                        if dirs > 0 {
                            parts.push(plural(dirs, "dossier", ""));
                        }
                        if files > 0 {
                            parts.push(plural(files, "fichier", ""));
                        }
                        let list = self.s.extra.iter().map(|p| p.display().to_string()).collect::<Vec<_>>().join("\n");
                        ui.label(parts.join(", ")).on_hover_text(list);
                        if ui.button("Retirer").on_hover_text("Retire les dossiers et fichiers glissés (le dossier principal reste)").clicked() {
                            self.s.extra.clear();
                            self.drop_note = None;
                            self.rescan();
                        }
                    });
                    ui.end_row();
                }

                ui.label("Lecture");
                ui.horizontal(|ui| {
                    if ui.checkbox(&mut self.s.recursive, "Inclure les sous-dossiers").changed() {
                        self.rescan();
                    }
                });
                ui.end_row();

                ui.label("Durée des photos");
                ui.add(egui::Slider::new(&mut self.s.photo_seconds, 1.0..=60.0).suffix(" s").step_by(0.5));
                ui.end_row();

                ui.label("Ajustement");
                egui::ComboBox::from_id_salt("fit").selected_text(self.s.fit.label()).width(260.0).show_ui(ui, |ui| {
                    for f in Fit::ALL {
                        ui.selectable_value(&mut self.s.fit, f, f.label());
                    }
                });
                ui.end_row();

                ui.label("Vidéos");
                ui.checkbox(&mut self.s.video_sound, "Avec le son (lues en entier)");
                ui.end_row();
            });
            ui.add_space(4.0);
            ui.label(RichText::new("Les changements s'appliquent tout de suite, sans relancer le diaporama.").color(TEXT_DIM));
        });
        if self.s.fit != before.1 && running {
            out.send(Command::SetFit { fit: self.s.fit });
        }
        let b = &before.0;
        let live = (self.s.photo_seconds, self.s.looping, self.s.shuffle, self.s.video_sound) != (b.photo_seconds, b.looping, b.shuffle, b.video_sound);
        if live && running {
            out.send(Command::SlideshowSettings {
                photo_seconds: self.s.photo_seconds,
                looping: self.s.looping,
                shuffle: self.s.shuffle,
                video_sound: self.s.video_sound,
            });
        }

        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("Médias ({})", self.files.len())).strong());
            if let Some(n) = &self.drop_note {
                ui.label(RichText::new(n).small().color(TEXT_DIM));
            }
        });
        let list = egui::Frame::new().fill(SLOT_EMPTY).corner_radius(4).inner_margin(6).show(ui, |ui| {
            ui.set_min_size(ui.available_size());
            if self.files.is_empty() {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Choisis un dossier, ou glisse ici des photos, des vidéos ou des dossiers").color(TEXT_DIM));
                });
                return;
            }
            let current = out.state.slide.as_ref().map(|s| s.2.clone());
            let row_h = 22.0;
            let font = egui::FontId::proportional(14.0);
            let mono = egui::FontId::monospace(12.0);
            egui::ScrollArea::vertical().auto_shrink(false).show_rows(ui, row_h, self.files.len(), |ui, range| {
                for i in range {
                    let p = &self.files[i];
                    let live = current.as_deref() == Some(&*p.to_string_lossy());
                    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::click());
                    let painter = ui.painter_at(r);
                    // Média à l'antenne : fond teinté et filet vert à gauche, sans pictogramme sur le nom.
                    if live {
                        painter.rect_filled(r, 3.0, GO.gamma_multiply(0.14));
                        painter.rect_filled(Rect::from_min_size(r.min, vec2(3.0, r.height())), 1.5, GO);
                    } else if resp.hovered() {
                        painter.rect_filled(r, 3.0, PANEL_HI);
                    }
                    let mid = r.center().y;
                    painter.text(egui::pos2(r.left() + 40.0, mid), egui::Align2::RIGHT_CENTER, (i + 1).to_string(), mono.clone(), if live { GO } else { TEXT_DIM });
                    painter.text(egui::pos2(r.left() + 52.0, mid), egui::Align2::LEFT_CENTER, name, font.clone(), if live { GO } else { TEXT });
                    // Hors ordre aléatoire, un double-clic saute directement à ce média.
                    if resp.double_clicked() && running && !self.s.shuffle {
                        out.send(Command::Goto { index: i });
                    }
                }
            });
        });
        // Fichiers survolés depuis l'Explorateur : la liste s'encadre pour inviter au dépôt.
        if hovering {
            let r = list.response.rect;
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::unique("dia-drop")));
            painter.rect_filled(r, 4.0, Color32::from_black_alpha(170));
            painter.rect_stroke(r.shrink(2.0), 4.0, egui::Stroke::new(2.0, ACCENT), egui::StrokeKind::Inside);
            painter.text(r.center(), egui::Align2::CENTER_CENTER, "Déposer pour ajouter au diaporama", egui::FontId::proportional(18.0), ACCENT);
        }
    }
}
