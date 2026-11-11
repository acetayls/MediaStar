//! Fenêtre du serveur OldStar (egui, charte des applis Star) : adresse à taper et QR code,
//! état de la console et des appareils, journal, réglages de base. Le serveur et les clients
//! tournent dans leurs threads ; fermer la fenêtre arrête OldStar.

use egui::{Color32, RichText};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use crate::config::{Config, ConsoleKind};
use crate::hub::Hub;
use crate::journal::{self, Level};
use star_ui::theme;

#[derive(PartialEq, Clone, Copy)]
enum Tab {
    Connexion,
    Journal,
    Parametres,
}

pub struct OldStarUi {
    hub: Arc<Hub>,
    port: u16,
    /// Le port n'a pas pu être ouvert : message à afficher.
    server_error: Option<String>,
    addrs: Vec<(String, std::net::Ipv4Addr)>,
    selected: usize,
    qr: Option<(String, egui::TextureHandle)>,
    tab: Tab,
    /// Réglages en cours de modification (onglet Paramètres).
    edit: Config,
    saved_note: String,
}

impl OldStarUi {
    pub fn new(cc: &eframe::CreationContext<'_>, hub: Arc<Hub>, port: u16, server_error: Option<String>) -> Self {
        theme::apply(&cc.egui_ctx);
        let edit = hub.cfg();
        Self {
            hub,
            port,
            server_error,
            addrs: crate::server::addresses(),
            selected: 0,
            qr: None,
            tab: Tab::Connexion,
            edit,
            saved_note: String::new(),
        }
    }

    fn url(&self) -> String {
        match self.addrs.get(self.selected) {
            Some((_, ip)) => format!("http://{ip}:{}/", self.port),
            None => format!("http://localhost:{}/", self.port),
        }
    }

    /// QR code de l'adresse, en texture (recalculé si l'adresse change).
    fn qr_texture(&mut self, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        let url = self.url();
        if let Some((u, t)) = &self.qr {
            if *u == url {
                return Some(t.clone());
            }
        }
        let code = qrcode::QrCode::new(url.as_bytes()).ok()?;
        let n = code.width();
        let colors = code.to_colors();
        let quiet = 4;
        let scale = 8;
        let side = (n + 2 * quiet) * scale;
        let mut img = egui::ColorImage::new([side, side], vec![Color32::WHITE; side * side]);
        for y in 0..n {
            for x in 0..n {
                if colors[y * n + x] == qrcode::Color::Dark {
                    for dy in 0..scale {
                        for dx in 0..scale {
                            img.pixels[((y + quiet) * scale + dy) * side + (x + quiet) * scale + dx] = Color32::BLACK;
                        }
                    }
                }
            }
        }
        let tex = ctx.load_texture("qr", img, egui::TextureOptions::NEAREST);
        self.qr = Some((url, tex.clone()));
        Some(tex)
    }

    fn connexion(&mut self, ui: &mut egui::Ui) {
        if let Some(e) = &self.server_error {
            ui.label(RichText::new(format!("La page ne peut pas être servie : {e}")).color(theme::STOP).strong());
            ui.label("Changez le port dans Paramètres, puis relancez OldStar.");
            ui.separator();
        }
        let url = self.url();
        let tex = self.qr_texture(ui.ctx());
        ui.horizontal_top(|ui| {
            ui.vertical(|ui| {
                ui.label(RichText::new("Sur la tablette, ouvrez :").color(theme::TEXT_DIM));
                ui.label(RichText::new(&url).size(28.0).strong().color(theme::ACCENT));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Copier l'adresse").clicked() {
                        ui.ctx().copy_text(url.clone());
                    }
                    if ui.button("Ouvrir la page sur ce PC").clicked() {
                        open_url(&format!("http://localhost:{}/", self.port));
                    }
                });
                ui.add_space(10.0);
                if self.addrs.len() > 1 {
                    ui.label(RichText::new("Carte réseau :").color(theme::TEXT_DIM));
                    for (i, (name, ip)) in self.addrs.clone().iter().enumerate() {
                        ui.radio_value(&mut self.selected, i, format!("{ip}   ({name})"));
                    }
                } else if self.addrs.is_empty() {
                    ui.label(RichText::new("Aucune adresse réseau : le PC est-il branché au réseau ?").color(theme::STOP));
                } else {
                    ui.label(RichText::new(format!("Carte réseau : {}", self.addrs[0].0)).color(theme::TEXT_DIM));
                }
                if ui.small_button("Actualiser les adresses").clicked() {
                    self.addrs = crate::server::addresses();
                    self.selected = self.selected.min(self.addrs.len().saturating_sub(1));
                }
                ui.add_space(14.0);
                self.status(ui);
            });
            if let Some(tex) = tex {
                ui.add_space(16.0);
                ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(240.0, 240.0)).corner_radius(6.0));
            }
        });
    }

    fn status(&self, ui: &mut egui::Ui) {
        let cfg = self.hub.cfg();
        let st = self.hub.yamaha_status();
        let kind = match cfg.console {
            ConsoleKind::Yamaha => "Yamaha CL/QL",
            ConsoleKind::M32 => "Midas M32 / X32",
        };
        let (dot, text) = if st.connected {
            (theme::GO, format!("{kind} connectée{}", if st.model.is_empty() { String::new() } else { format!(" ({})", st.model) }))
        } else {
            (theme::STOP, format!("{kind} hors ligne{}", if st.error.is_empty() { String::new() } else { format!(" : {}", st.error) }))
        };
        ui.horizontal(|ui| {
            ui.label(RichText::new("●").color(dot));
            ui.label(text);
        });
        let n = self.hub.ws_clients.load(Ordering::Relaxed) + self.hub.poll_count();
        ui.horizontal(|ui| {
            ui.label(RichText::new("●").color(if n > 0 { theme::GO } else { theme::TEXT_DIM }));
            ui.label(match n {
                0 => "Aucun appareil connecté".to_string(),
                1 => "1 appareil connecté".to_string(),
                n => format!("{n} appareils connectés"),
            });
        });
    }

    fn journal(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Copier").clicked() {
                ui.ctx().copy_text(journal::text());
            }
            if ui.button("Effacer").clicked() {
                journal::clear();
            }
            if let Some(p) = star_core::log::log_path() {
                ui.label(RichText::new(format!("Fichier : {}", p.display())).color(theme::TEXT_DIM).small());
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            for l in journal::lines() {
                let color = match l.level {
                    Level::Info => theme::TEXT,
                    Level::Warn => theme::ACCENT,
                    Level::Error => theme::STOP,
                    Level::Detail => theme::TEXT_DIM,
                };
                ui.horizontal_wrapped(|ui| {
                    ui.label(RichText::new(&l.time).monospace().color(theme::TEXT_DIM));
                    ui.label(RichText::new(&l.text).color(color));
                });
            }
        });
    }

    fn parametres(&mut self, ui: &mut egui::Ui) {
        let e = &mut self.edit;
        egui::Grid::new("params").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label("Port de la page");
            ui.add(egui::DragValue::new(&mut e.http_port).range(1..=65535));
            ui.end_row();
            ui.label("Console");
            egui::ComboBox::from_id_salt("console")
                .selected_text(match e.console {
                    ConsoleKind::Yamaha => "Yamaha CL/QL",
                    ConsoleKind::M32 => "Midas M32 / X32",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut e.console, ConsoleKind::Yamaha, "Yamaha CL/QL");
                    ui.selectable_value(&mut e.console, ConsoleKind::M32, "Midas M32 / X32");
                });
            ui.end_row();
            match e.console {
                ConsoleKind::Yamaha => {
                    ui.label("Adresse IP de la console");
                    ui.text_edit_singleline(&mut e.yamaha.host);
                    ui.end_row();
                    ui.label("Port");
                    ui.add(egui::DragValue::new(&mut e.yamaha.port).range(1..=65535));
                    ui.end_row();
                }
                ConsoleKind::M32 => {
                    ui.label("Adresse IP de la console");
                    ui.text_edit_singleline(&mut e.m32.host);
                    ui.end_row();
                    ui.label("Port");
                    ui.add(egui::DragValue::new(&mut e.m32.port).range(1..=65535));
                    ui.end_row();
                }
            }
            ui.label("Fenêtres du PC");
            ui.checkbox(&mut e.free_windows, "La page peut afficher n'importe quelle fenêtre ouverte");
            ui.end_row();
        });
        ui.add_space(10.0);
        if ui.button("Enregistrer").clicked() {
            let cfg = self.edit.clone();
            let before = self.hub.cfg();
            match cfg.save(&self.hub.config_path) {
                Ok(()) => {
                    let restart = before.http_port != cfg.http_port || before.console != cfg.console;
                    self.hub.replace_config(cfg);
                    journal::info("Réglages enregistrés");
                    self.saved_note = if restart {
                        "Enregistré. Le port et le choix de la console prendront effet au prochain lancement.".into()
                    } else {
                        "Enregistré : la console se reconnecte avec la nouvelle adresse.".into()
                    };
                }
                Err(err) => {
                    journal::error(&format!("Réglages non enregistrés : {err}"));
                    self.saved_note = format!("Échec : {err}");
                }
            }
        }
        if !self.saved_note.is_empty() {
            ui.label(RichText::new(&self.saved_note).color(theme::TEXT_DIM));
        }
        ui.add_space(6.0);
        ui.label(RichText::new(format!("Fichier de réglages : {}", self.hub.config_path.display())).color(theme::TEXT_DIM).small());
    }
}

impl eframe::App for OldStarUi {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // État des clients et journal : un coup d'œil toutes les demi-secondes suffit.
        ui.ctx().request_repaint_after(Duration::from_millis(500));
        egui::Panel::top("haut").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("Old").size(26.0).family(theme::logo_family()));
                ui.label(RichText::new("Star").size(26.0).family(theme::logo_family()).color(theme::ACCENT));
                ui.label(RichText::new(env!("CARGO_PKG_VERSION")).color(theme::TEXT_DIM));
                ui.add_space(16.0);
                ui.selectable_value(&mut self.tab, Tab::Connexion, "Connexion");
                ui.selectable_value(&mut self.tab, Tab::Journal, "Journal");
                ui.selectable_value(&mut self.tab, Tab::Parametres, "Paramètres");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Quitter").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
            ui.add_space(4.0);
        });
        egui::CentralPanel::default().show(ui, |ui| match self.tab {
            Tab::Connexion => self.connexion(ui),
            Tab::Journal => self.journal(ui),
            Tab::Parametres => self.parametres(ui),
        });
    }
}

/// Ouvre une adresse dans le navigateur par défaut.
pub fn open_url(url: &str) {
    #[cfg(windows)]
    let r = {
        use std::os::windows::process::CommandExt;
        // Sans fenêtre de console (CREATE_NO_WINDOW).
        std::process::Command::new("cmd").args(["/C", "start", "", url]).creation_flags(0x0800_0000).spawn()
    };
    #[cfg(target_os = "macos")]
    let r = std::process::Command::new("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let r = std::process::Command::new("xdg-open").arg(url).spawn();
    if let Err(e) = r {
        journal::warn(&format!("Navigateur introuvable : {e}"));
    }
}
