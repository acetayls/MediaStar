//! Mires six25 (https://six25.fr/mire-de-test/) : les vidéos d'origine, jouées telles quelles.
//!
//! Les fichiers ne sont pas livrés avec CueStar : chacun est téléchargé depuis media.six25.fr au
//! premier clic (curl, présent sur Windows 10+, macOS et la plupart des Linux), rangé dans le
//! dossier de cache de CueStar, puis joué en boucle sur la sortie, avec son son.

use crate::link::OutputLink;
use crate::protocol::{Clip, Command, Transition};
use egui::{vec2, RichText};
use star_ui::theme::*;
use std::path::PathBuf;
use std::process::{Child, Command as Process, Stdio};

const PAGE: &str = "https://six25.fr/mire-de-test/";

struct Mire {
    label: &'static str,
    detail: &'static str,
    file: &'static str,
    /// Taille annoncée, en octets (progression du téléchargement et contrôle du fichier).
    size: u64,
    /// Image fixe : affichée sans boucle.
    still: bool,
}

const MIRES: [Mire; 4] = [
    Mire { label: "six25 1080p25", detail: "MP4 H.264, 39 Mo", file: "Mire_Progressive_25p.mp4", size: 39_268_743, still: false },
    Mire { label: "six25 1080p60", detail: "MP4 H.264, 81 Mo", file: "Mire_Progressive_60p.mp4", size: 81_260_400, still: false },
    Mire { label: "six25 1080i50", detail: "MOV ProRes 422, 715 Mo", file: "Mire_Interlaced_50i.mov", size: 714_802_104, still: false },
    Mire { label: "six25 image fixe", detail: "BMP 1080p25, 6 Mo", file: "Mire_Progressive_25p.bmp", size: 6_220_856, still: true },
];

fn url(m: &Mire) -> String {
    format!("https://media.six25.fr/Mire/{}", m.file)
}

fn dir() -> Option<PathBuf> {
    crate::project::APP.cache_file("mires-six25")
}

fn path(m: &Mire) -> Option<PathBuf> {
    dir().map(|d| d.join(m.file))
}

/// Fichier complet sur le disque.
fn ready(m: &Mire) -> bool {
    path(m).and_then(|p| std::fs::metadata(p).ok()).map(|md| md.len() > 0 && (m.size == 0 || md.len() == m.size)).unwrap_or(false)
}

struct Download {
    index: usize,
    child: Child,
    part: PathBuf,
    /// Lancer la mire dès la fin du téléchargement.
    play: bool,
}

#[derive(Default)]
pub struct Six25 {
    download: Option<Download>,
    /// Mire six25 à l'écran (index dans `MIRES`).
    pub playing: Option<usize>,
    error: Option<String>,
}

impl Six25 {
    fn start_download(&mut self, index: usize) {
        let m = &MIRES[index];
        let Some(dest) = path(m) else {
            self.error = Some("dossier de cache introuvable".into());
            return;
        };
        if let Some(d) = dest.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        let part = dest.with_extension("part");
        let _ = std::fs::remove_file(&part);
        let mut cmd = Process::new("curl");
        cmd.args(["-L", "-f", "-s", "-S", "-o"]).arg(&part).arg(url(m)).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW : pas de console qui s'ouvre.
        }
        match cmd.spawn() {
            Ok(child) => {
                self.error = None;
                self.download = Some(Download { index, child, part, play: true });
                star_core::log::write(&format!("mires six25 : téléchargement de {}", url(m)));
            }
            Err(e) => self.error = Some(format!("curl introuvable ({e}) : télécharge la mire depuis {PAGE} et place-la dans {}", dest.display())),
        }
    }

    /// Suit le téléchargement en cours ; lance la mire quand il se termine.
    fn poll(&mut self, out: &mut OutputLink, pattern: &mut Option<crate::protocol::PatternKind>) {
        let Some(d) = &mut self.download else { return };
        let status = match d.child.try_wait() {
            Ok(None) => return,
            Ok(Some(s)) => Ok(s),
            Err(e) => Err(e.to_string()),
        };
        let d = self.download.take().unwrap();
        let mut stderr = String::new();
        if let Some(mut e) = d.child.stderr {
            use std::io::Read;
            let _ = e.read_to_string(&mut stderr);
        }
        let m = &MIRES[d.index];
        let dest = path(m).unwrap_or_default();
        match status {
            Ok(s) if s.success() && std::fs::rename(&d.part, &dest).is_ok() => {
                star_core::log::write(&format!("mires six25 : {} téléchargée", m.file));
                if d.play {
                    self.play(d.index, out, pattern);
                }
            }
            Ok(_) | Err(_) => {
                let _ = std::fs::remove_file(&d.part);
                let why = match status {
                    Err(e) => e,
                    Ok(_) => stderr.trim().to_string(),
                };
                let msg = format!("téléchargement de {} impossible : {why}", m.file);
                star_core::log::warn(&format!("mires six25 : {msg}"));
                self.error = Some(msg);
            }
        }
    }

    fn play(&mut self, index: usize, out: &mut OutputLink, pattern: &mut Option<crate::protocol::PatternKind>) {
        let m = &MIRES[index];
        let Some(p) = path(m) else { return };
        *pattern = None;
        self.playing = Some(index);
        out.send(Command::Load {
            path: p.to_string_lossy().into_owned(),
            paused: false,
            looping: !m.still,
            hold: true,
            transition: Transition::default(),
            clip: Clip::default(),
            signal_end: false,
        });
    }

    pub fn stop(&mut self, out: &mut OutputLink) {
        if self.playing.take().is_some() {
            out.send(Command::Stop);
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, out: &mut OutputLink, pattern: &mut Option<crate::protocol::PatternKind>) {
        self.poll(out, pattern);
        // Une mire générée choisie entre-temps remplace la mire six25.
        if pattern.is_some() {
            self.playing = None;
        }
        ui.horizontal_wrapped(|ui| {
            for (i, m) in MIRES.iter().enumerate() {
                let sel = self.playing == Some(i);
                let loading = self.download.as_ref().is_some_and(|d| d.index == i);
                let have = ready(m);
                let text = if loading {
                    let got = std::fs::metadata(&self.download.as_ref().unwrap().part).map(|md| md.len()).unwrap_or(0);
                    if m.size > 0 {
                        format!("{} : {} %", m.label, got * 100 / m.size)
                    } else {
                        format!("{} : {} Mo", m.label, got / 1_000_000)
                    }
                } else {
                    m.label.to_string()
                };
                let b = egui::Button::new(RichText::new(text).color(if sel { PLAYING_INK } else if have { TEXT } else { TEXT_DIM }))
                    .fill(if sel { ACCENT } else { PANEL_HI })
                    .min_size(vec2(150.0, 30.0));
                let hover = if have { format!("{} : jouée en boucle avec son son", m.detail) } else { format!("{} : téléchargée au premier clic depuis media.six25.fr", m.detail) };
                if ui.add_enabled(self.download.is_none() || loading, b).on_hover_text(hover).clicked() && !loading {
                    if sel {
                        self.stop(out);
                    } else if have {
                        self.play(i, out, pattern);
                    } else {
                        self.start_download(i);
                    }
                }
            }
        });
        if self.download.is_some() {
            ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
            if ui.button("Annuler le téléchargement").clicked() {
                if let Some(mut d) = self.download.take() {
                    let _ = d.child.kill();
                    let _ = d.child.wait();
                    let _ = std::fs::remove_file(&d.part);
                }
            }
        }
        if let Some(e) = &self.error {
            ui.label(RichText::new(e).color(METER_MID));
        }
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("Mire de test six25 (barres SMPTE HD, flèches de recadrage, safe zones EBU, TC, son 440 Hz, bips 1 kHz d'une image, cercles 1/2 à 1/5), d'après").color(TEXT_DIM));
            ui.hyperlink_to("six25.fr", PAGE);
        });
    }
}
