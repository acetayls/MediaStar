//! Accueil de MediaStar, composé comme son icône : quatre tuiles en carré sur un fond dégradé,
//! chacune avec son bandeau de couleur et son symbole (l'étoile du centre en moins). Une tuile
//! lance son module dans son propre processus (ce même exe relancé avec `--jingle`, `--cue`, `--old` ou `--sweep`) ;
//! l'accueil reste ouvert pour en lancer un autre.

use eframe::egui;
use egui::{pos2, vec2, Color32, CornerRadius, Pos2, Rect, Sense, Shape, Stroke};
use serde::{Deserialize, Serialize};
use star_core::i18n::{self, tr, Lang};
use star_ui::{app_bar, theme};
use std::process::{Child, Command, Stdio};

/// Version de MediaStar (indépendante de celles des modules).
const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Dossier de réglages de MediaStar (celui de l'accueil ; chaque module garde le sien).
static APP: star_core::app::App = star_core::app::App::new("MediaStar");

/// Réglages de l'accueil retenus d'un lancement à l'autre.
#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    language: Lang,
}

impl Default for Settings {
    /// Interface en anglais par défaut.
    fn default() -> Self {
        Self { language: Lang::En }
    }
}

fn settings_path() -> Option<std::path::PathBuf> {
    APP.config_file("accueil.json")
}

// Couleurs de l'icône (assets/logo/mediastar-icone-app.svg).
const FOND_HAUT: Color32 = Color32::from_rgb(0x2d, 0x2d, 0x34);
const FOND_BAS: Color32 = Color32::from_rgb(0x16, 0x16, 0x1a);
const TUILE: Color32 = Color32::from_rgb(0x2c, 0x2c, 0x33);
const TUILE_SURVOL: Color32 = Color32::from_rgb(0x35, 0x35, 0x3d);
const ENCRE: Color32 = Color32::from_rgb(0x20, 0x20, 0x26);
const BLEU: Color32 = Color32::from_rgb(0x4a, 0x8f, 0xdb);
const VERT: Color32 = Color32::from_rgb(0x4f, 0xae, 0x58);
const VIOLET: Color32 = Color32::from_rgb(0x9b, 0x6b, 0xd3);
const TURQUOISE: Color32 = Color32::from_rgb(0x3f, 0xb8, 0xaf);

pub fn run() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("MediaStar")
            .with_inner_size([640.0, 650.0])
            .with_min_inner_size([520.0, 550.0])
            .with_icon(
                eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/mediastar-fenetre-256.png"))
                    .expect("icône PNG valide"),
            ),
        centered: true,
        ..Default::default()
    };
    let _ = eframe::run_native("MediaStar", options, Box::new(|cc| Ok(Box::new(Home::new(cc)))));
}

#[derive(Clone, Copy)]
enum Glyph {
    Casque,
    Lecture,
    Fader,
    Reponse,
}

struct Module {
    name: &'static str,
    phrase_fr: &'static str,
    phrase_en: &'static str,
    /// `None` : pas encore disponible (« bientôt »).
    version: Option<&'static str>,
    color: Color32,
    glyph: Glyph,
    /// Argument de relance ; `None` : pas encore disponible.
    arg: Option<&'static str>,
}

const MODULES: [Module; 4] = [
    Module {
        name: "JingleStar",
        phrase_fr: "Jingles et playlists sur plusieurs sorties audio",
        phrase_en: "Jingles and playlists on several audio outputs",
        version: Some(jinglestar::VERSION),
        color: BLEU,
        glyph: Glyph::Casque,
        arg: Some("--jingle"),
    },
    Module {
        name: "CueStar",
        phrase_fr: "Diffusion vidéo : lecteur, diaporama, cuelist",
        phrase_en: "Video playback: player, slideshow, cuelist",
        version: Some(cuestar::VERSION),
        color: VERT,
        glyph: Glyph::Lecture,
        arg: Some("--cue"),
    },
    Module {
        name: "OldStar",
        phrase_fr: "Télécommande web pour « vieille » tablette : console, applis, fenêtres",
        phrase_en: "Web remote for an \"old\" tablet: console, apps, windows",
        version: Some(oldstar::VERSION),
        color: VIOLET,
        glyph: Glyph::Fader,
        arg: Some("--old"),
    },
    Module {
        name: "SweepStar",
        phrase_fr: "Mesure acoustique : délai, réponse, spectre",
        phrase_en: "Acoustic measurement: delay, response, spectrum",
        version: Some(sweepstar::VERSION),
        color: TURQUOISE,
        glyph: Glyph::Reponse,
        arg: Some("--sweep"),
    },
];

struct Home {
    /// Modules lancés, gardés pour récupérer leur fin (pas de processus zombies sous Unix).
    children: Vec<Child>,
    error: Option<String>,
    settings: Settings,
}

impl Home {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply(&cc.egui_ctx);
        let settings: Settings = settings_path().and_then(|p| star_core::prefs::load(&p)).unwrap_or_default();
        i18n::set_lang(settings.language);
        Self { children: Vec::new(), error: None, settings }
    }

    fn launch(&mut self, name: &str, arg: &str) {
        let result = std::env::current_exe().and_then(|exe| {
            let mut cmd = Command::new(exe);
            cmd.arg(arg).stdin(Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW : pas de console (build de dev).
            }
            cmd.spawn()
        });
        match result {
            Ok(child) => {
                self.children.push(child);
                self.error = None;
            }
            Err(e) => self.error = Some(format!("{} {name} : {e}", tr("Impossible de lancer", "Could not start"))),
        }
    }
}

impl eframe::App for Home {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.children.retain_mut(|c| matches!(c.try_wait(), Ok(None)));
        let mut clicked = None;
        // Barre commune à la suite : logo et nom ; choix de la langue à droite.
        app_bar::show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                app_bar::identity(ui, app_bar::Logo::Media, "MediaStar", false);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let before = self.settings.language;
                    ui.selectable_value(&mut self.settings.language, Lang::En, "English");
                    ui.selectable_value(&mut self.settings.language, Lang::Fr, "Français");
                    if self.settings.language != before {
                        i18n::set_lang(self.settings.language);
                        if let Some(p) = settings_path() {
                            if let Err(e) = star_core::prefs::save(&p, &self.settings) {
                                self.error = Some(format!("{} : {e}", tr("Réglages non enregistrés", "Settings not saved")));
                            }
                        }
                    }
                });
            });
        });
        egui::CentralPanel::default().frame(egui::Frame::NONE).show(ui, |ui| {
            let full = ui.max_rect();
            gradient(ui.painter(), full);

            // Grille 2×2 : marges et écart dans les proportions de l'icône.
            let footer = 30.0;
            let margin = 28.0;
            let area = Rect::from_min_max(full.min + vec2(margin, margin), full.max - vec2(margin, margin + footer));
            // Pixels entiers : bandeau, fond et liseré de survol tombent exactement au même endroit.
            let area = Rect::from_min_max(area.min.round(), area.max.round());
            let gap = (area.width().min(area.height()) * 0.045).max(14.0).round();
            let tile = vec2(((area.width() - gap) / 2.0).floor(), ((area.height() - gap) / 2.0).floor());
            for (i, m) in MODULES.iter().enumerate() {
                let (col, row) = ((i % 2) as f32, (i / 2) as f32);
                let rect = Rect::from_min_size(area.min + vec2(col * (tile.x + gap), row * (tile.y + gap)), tile);
                if draw_tile(ui, rect, m) {
                    clicked = m.arg.map(|a| (m.name, a));
                }
            }

            // Pied : erreur éventuelle, puis version de MediaStar et version de build, discrètement.
            let painter = ui.painter();
            let y = full.max.y - footer / 2.0 - 4.0;
            if let Some(e) = &self.error {
                painter.text(pos2(full.center().x, area.max.y + 4.0), egui::Align2::CENTER_TOP, e, font(13.0), theme::STOP);
            }
            painter.text(
                pos2(full.center().x, y),
                egui::Align2::CENTER_CENTER,
                format!("MediaStar v{VERSION} · {} · {}", env!("STAR_GIT"), env!("STAR_BUILD_DATE")),
                font(11.0),
                theme::TEXT_DIM,
            );
        });
        if let Some((name, arg)) = clicked {
            self.launch(name, arg);
        }
        // Relevé régulier de la fin des modules lancés.
        if !self.children.is_empty() {
            ctx.request_repaint_after(std::time::Duration::from_secs(2));
        }
    }
}

fn font(size: f32) -> egui::FontId {
    egui::FontId::proportional(size)
}

/// Fond dégradé du haut vers le bas, comme celui de l'icône.
fn gradient(painter: &egui::Painter, rect: Rect) {
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.left_top(), FOND_HAUT);
    mesh.colored_vertex(rect.right_top(), FOND_HAUT);
    mesh.colored_vertex(rect.right_bottom(), FOND_BAS);
    mesh.colored_vertex(rect.left_bottom(), FOND_BAS);
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(Shape::mesh(mesh));
}

/// Une tuile : bandeau de couleur avec le nom, symbole, phrase et version. Vrai si cliquée.
fn draw_tile(ui: &mut egui::Ui, rect: Rect, m: &Module) -> bool {
    let enabled = m.arg.is_some() && m.version.is_some();
    let id = egui::Id::unique(("accueil", m.name));
    let resp = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
    let hovered = enabled && resp.hovered();
    // Module pas encore disponible : tuile estompée.
    let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.45) };
    let color = fade(m.color);
    let painter = ui.painter_at(rect);

    // Tuile et bandeau (rayon et hauteur du bandeau dans les proportions de l'icône : 50 et 52 sur 316).
    let radius = (rect.width().min(rect.height()) * 0.12).min(28.0).round();
    let corner = CornerRadius::same(radius as u8);
    let band = (rect.height() * 0.16).clamp(30.0, 44.0);
    painter.rect_filled(rect, corner, if hovered { TUILE_SURVOL } else { TUILE });
    let band_rect = Rect::from_min_size(rect.min, vec2(rect.width(), band));
    painter.rect_filled(
        band_rect,
        CornerRadius { nw: radius as u8, ne: radius as u8, sw: 0, se: 0 },
        color,
    );
    if hovered {
        // Même rectangle et mêmes arrondis que la tuile et son bandeau : le liseré part du haut du bandeau.
        painter.rect_stroke(rect, corner, Stroke::new(2.0, m.color), egui::StrokeKind::Inside);
    }
    painter.text(
        band_rect.center(),
        egui::Align2::CENTER_CENTER,
        m.name,
        egui::FontId::new(band * 0.55, theme::logo_family()),
        if enabled { ENCRE } else { ENCRE.gamma_multiply(0.8) },
    );

    // Symbole, puis phrase et version en bas de la tuile.
    let text_h = 52.0;
    let body = Rect::from_min_max(pos2(rect.min.x, band_rect.max.y), pos2(rect.max.x, rect.max.y - text_h));
    let size = (body.height() * 0.78).min(body.width() * 0.5);
    let glyph_rect = Rect::from_center_size(body.center(), vec2(size, size));
    draw_glyph(&painter, glyph_rect, m.glyph, color);

    let galley = painter.layout(tr(m.phrase_fr, m.phrase_en).to_owned(), font(13.0), fade(theme::TEXT), rect.width() - 28.0);
    let phrase_pos = pos2(rect.center().x - galley.size().x / 2.0, rect.max.y - text_h);
    painter.galley(phrase_pos, galley, fade(theme::TEXT));
    painter.text(
        pos2(rect.center().x, rect.max.y - 12.0),
        egui::Align2::CENTER_BOTTOM,
        m.version.map_or_else(|| tr("bientôt", "coming soon").to_owned(), |v| format!("v{v}")),
        font(12.0),
        if enabled { theme::TEXT_DIM } else { m.color.gamma_multiply(0.8) },
    );

    let resp = if enabled { resp.on_hover_cursor(egui::CursorIcon::PointingHand) } else { resp.on_hover_text(tr("Pas encore disponible", "Not available yet")) };
    resp.clicked()
}

/// Symboles de l'icône, dessinés dans une grille de 100 × 100 ramenée à `rect`.
fn draw_glyph(painter: &egui::Painter, rect: Rect, glyph: Glyph, color: Color32) {
    let s = rect.width() / 100.0;
    let p = |x: f32, y: f32| rect.min + vec2(x * s, y * s);
    // Trait à bouts ronds (egui ne fait pas de bouts ronds) : ligne brisée et disques aux extrémités.
    let line = |pts: Vec<Pos2>, w: f32| {
        let w = w * s;
        if let (Some(a), Some(b)) = (pts.first().copied(), pts.last().copied()) {
            painter.circle_filled(a, w / 2.0, color);
            painter.circle_filled(b, w / 2.0, color);
        }
        for q in pts.iter().skip(1).take(pts.len().saturating_sub(2)) {
            painter.circle_filled(*q, w / 2.0, color);
        }
        painter.add(Shape::line(pts, Stroke::new(w, color)));
    };
    let arc = |cx: f32, cy: f32, r: f32, a0: f32, a1: f32| -> Vec<Pos2> {
        let n = 24;
        (0..=n).map(|i| {
            let a = a0 + (a1 - a0) * i as f32 / n as f32;
            p(cx + r * a.cos(), cy + r * a.sin())
        }).collect()
    };
    let rrect = |x: f32, y: f32, w: f32, h: f32, r: f32| {
        painter.rect_filled(Rect::from_min_size(p(x, y), vec2(w * s, h * s)), CornerRadius::same((r * s) as u8), color);
    };
    use std::f32::consts::PI;
    match glyph {
        Glyph::Casque => {
            let mut pts = vec![p(16.5, 62.0)];
            pts.extend(arc(50.0, 52.0, 33.5, PI, 2.0 * PI));
            pts.push(p(83.5, 62.0));
            line(pts, 13.0);
            rrect(10.0, 50.0, 25.0, 38.0, 9.0);
            rrect(65.0, 50.0, 25.0, 38.0, 9.0);
        }
        Glyph::Lecture => {
            let tri = vec![p(31.0, 19.0), p(85.0, 50.0), p(31.0, 81.0)];
            painter.add(Shape::convex_polygon(tri.clone(), color, Stroke::NONE));
            let mut closed = tri.clone();
            closed.push(tri[0]);
            line(closed, 12.0);
        }
        Glyph::Fader => {
            line(vec![p(20.0, 12.0), p(20.0, 88.0)], 8.0);
            rrect(7.0, 32.0, 26.0, 16.0, 3.5);
            painter.circle_filled(p(70.0, 80.0), 6.0 * s, color);
            for r in [15.0, 27.0, 39.0] {
                line(arc(70.0, 80.0, r, 1.25 * PI, 1.75 * PI), 10.0);
            }
        }
        Glyph::Reponse => {
            line(REPONSE.iter().map(|&(x, y)| p(x, y)).collect(), 10.0);
        }
    }
}

/// Réponse impulsionnelle du symbole de SweepStar (points repris de l'icône, un sur trois).
const REPONSE: [(f32, f32); 59] = [
    (8.0, 64.0), (20.0, 64.0), (26.0, 10.0), (32.0, 86.0), (32.4, 42.7), (32.8, 43.6), (33.9, 48.1), (35.0, 54.3),
    (36.1, 60.9), (37.3, 67.2), (38.4, 72.2), (39.5, 75.5), (40.7, 76.7), (41.8, 76.1), (42.9, 73.9), (44.1, 70.6),
    (45.2, 66.8), (46.3, 63.1), (47.5, 60.0), (48.6, 57.9), (49.7, 56.8), (50.9, 56.9), (52.0, 58.0), (53.1, 59.7),
    (54.3, 61.8), (55.4, 63.9), (56.5, 65.8), (57.7, 67.2), (58.8, 68.0), (59.9, 68.1), (61.1, 67.7), (62.2, 66.8),
    (63.3, 65.6), (64.5, 64.4), (65.6, 63.2), (66.7, 62.4), (67.8, 61.8), (69.0, 61.6), (70.1, 61.8), (71.2, 62.3),
    (72.4, 62.9), (73.5, 63.6), (74.6, 64.3), (75.8, 64.8), (76.9, 65.2), (78.0, 65.3), (79.2, 65.3), (80.3, 65.1),
    (81.4, 64.7), (82.6, 64.3), (83.7, 64.0), (84.8, 63.6), (86.0, 63.4), (87.1, 63.2), (88.2, 63.2), (89.4, 63.3),
    (90.5, 63.5), (91.6, 63.7), (92.0, 63.8),
];
