//! Aperçu navigable de la régie : un petit lecteur muet où l'on se déplace dans un média sans
//! toucher à la sortie, et la bande de vignettes de l'éditeur d'élément.
//!
//! Il tourne dans un processus à part (`--preview`) avec sa propre libmpv, comme les vignettes :
//! un fichier qui fait planter le décodeur ne touche ni la régie ni la sortie à l'antenne.
//! mpv y rend l'image en logiciel (API de rendu « sw ») dans une petite image RGBA.
//!
//! Dialogue : commandes JSON sur l'entrée standard ; sur la sortie standard, des messages
//! « type (1 octet) + longueur (4 octets) + contenu » : événement JSON (0) ou image (1).

use crate::output::mpv::{self, Handle, Mpv, RenderParam};
use serde::{Deserialize, Serialize};
use std::ffi::{c_int, c_void, CString};
use std::io::{BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Largeur de l'image de l'aperçu.
pub const LIVE_W: u32 = 640;
/// Largeur des vignettes de la frise.
pub const STRIP_W: u32 = 160;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PreviewCmd {
    /// Ouvre un média, en pause sur l'image à `at` secondes.
    /// `seq` : numéro de la demande, rendu dans les positions une fois la demande servie.
    Open {
        path: String,
        at: f64,
        #[serde(default)]
        seq: u64,
    },
    Close,
    /// `exact` faux : saut à l'image clé la plus proche (fluide pendant un glisser).
    Seek {
        seconds: f64,
        exact: bool,
        #[serde(default)]
        seq: u64,
    },
    Play,
    Pause,
    /// Image suivante ou précédente (en pause). Les pas demandés à la suite sont faits un par
    /// un, chacun depuis l'image du précédent : aucun ne se perd.
    Step {
        back: bool,
        #[serde(default)]
        seq: u64,
    },
    /// Une image toutes les durée / `count` secondes, pour la frise de l'éditeur.
    Strip {
        path: String,
        count: usize,
    },
    /// Images exactes aux instants `times` (s), pour la frise zoomée : une demande remplace la
    /// précédente pas encore servie. Servie après la frise entière si elle est en cours.
    Frames {
        path: String,
        times: Vec<f64>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PreviewEvent {
    Ready { mpv: String },
    Missing { detail: String },
    Opened {
        path: String,
        duration: f64,
        image: bool,
        /// Images par seconde (0 : inconnue).
        #[serde(default)]
        fps: f64,
        /// Rotation demandée par le fichier (métadonnées), que l'aperçu applique au dessin.
        #[serde(default)]
        rotate: u16,
    },
    /// `seq` : dernière demande servie (saut, pas, ouverture).
    Position {
        time: f64,
        paused: bool,
        #[serde(default)]
        seq: u64,
    },
    StripInfo { path: String, duration: f64, count: usize },
    /// Les images de détail qui suivent (`FRAME_AT`) sont celles de ce fichier.
    FramesFor { path: String },
    Error { message: String },
}

/// Étiquette d'une image de détail : `FRAME_AT | millisecondes` de son instant.
const FRAME_AT: u32 = 0x8000_0000;

/// Image envoyée : 0 = aperçu, 1 + i = vignette n° i de la frise, `FRAME_AT | ms` = image de
/// détail de la frise zoomée.
struct FrameMsg {
    tag: u32,
    w: u32,
    h: u32,
    rgba: Vec<u8>,
}

fn send_msg(out: &mut impl Write, kind: u8, payload: &[u8]) -> std::io::Result<()> {
    out.write_all(&[kind])?;
    out.write_all(&(payload.len() as u32).to_le_bytes())?;
    out.write_all(payload)?;
    out.flush()
}

fn send_event(ev: &PreviewEvent) {
    if let Ok(s) = serde_json::to_vec(ev) {
        let _ = send_msg(&mut std::io::stdout().lock(), 0, &s);
    }
}

fn send_frame(f: &FrameMsg) {
    let mut buf = Vec::with_capacity(12 + f.rgba.len());
    buf.extend_from_slice(&f.tag.to_le_bytes());
    buf.extend_from_slice(&f.w.to_le_bytes());
    buf.extend_from_slice(&f.h.to_le_bytes());
    buf.extend_from_slice(&f.rgba);
    let _ = send_msg(&mut std::io::stdout().lock(), 1, &buf);
}

// --- Processus `--preview` -------------------------------------------------------------------

const RENDER_PARAM_SW_SIZE: c_int = 17;
const RENDER_PARAM_SW_FORMAT: c_int = 18;
const RENDER_PARAM_SW_STRIDE: c_int = 19;
const RENDER_PARAM_SW_POINTER: c_int = 20;

unsafe extern "C" fn on_update(data: *mut c_void) {
    let flag = unsafe { &*(data as *const AtomicBool) };
    flag.store(true, Ordering::Release);
}

/// Un lecteur mpv muet avec son contexte de rendu logiciel.
struct SwPlayer {
    rc: Handle,
    api: mpv::RenderApi,
    dirty: &'static AtomicBool,
    // Détruit après le contexte de rendu (ordre des champs).
    m: Mpv,
}

impl SwPlayer {
    fn new() -> Result<Self, String> {
        let m = Mpv::create()?;
        for (k, v) in [
            ("terminal", "no"),
            ("vo", "libmpv"),
            ("aid", "no"),
            ("sid", "no"),
            ("audio", "no"),
            ("hwdec", "no"),
            // Image brute (le rendu logiciel de mpv ne sait pas la tourner) : la rotation du
            // fichier et celle du cue sont appliquées au dessin, comme mpv le fait sur la sortie.
            ("video-rotate", "no"),
            ("idle", "yes"),
            ("keep-open", "always"),
            ("pause", "yes"),
            ("hr-seek", "yes"),
            ("image-display-duration", "inf"),
            ("osd-level", "0"),
            ("input-default-bindings", "no"),
            ("demuxer-max-bytes", "32MiB"),
        ] {
            let _ = m.set_option(k, v);
        }
        m.initialize()?;
        let api = m.render_api().ok_or("API de rendu de mpv absente (libmpv trop ancienne)")?;
        let sw = CString::new("sw").unwrap_or_default();
        let mut params = [RenderParam { kind: mpv::RENDER_PARAM_API_TYPE, data: sw.as_ptr() as *mut c_void }, RenderParam::END];
        let mut rc: Handle = std::ptr::null_mut();
        let r = unsafe { (api.create)(&mut rc, m.raw(), params.as_mut_ptr()) };
        if r < 0 || rc.is_null() {
            return Err(format!("rendu logiciel de mpv indisponible (code {r})"));
        }
        let dirty: &'static AtomicBool = Box::leak(Box::new(AtomicBool::new(false)));
        unsafe { (api.set_update_callback)(rc, Some(on_update), dirty as *const AtomicBool as *mut c_void) };
        Ok(Self { rc, api, dirty, m })
    }

    /// Taille de l'image affichée (après rotation et rapport d'aspect).
    fn video_size(&self) -> Option<(u32, u32)> {
        let get = |k| self.m.get_property(k).and_then(|s| s.parse::<f64>().ok()).filter(|v| *v > 0.0);
        Some((get("video-out-params/dw").or_else(|| get("dwidth"))? as u32, get("video-out-params/dh").or_else(|| get("dheight"))? as u32))
    }

    /// Rend l'image en cours si mpv en a une nouvelle, à `width` points de large.
    fn render_if_new(&self, width: u32) -> Option<(u32, u32, Vec<u8>)> {
        if !self.dirty.swap(false, Ordering::AcqRel) {
            return None;
        }
        let flags = unsafe { (self.api.update)(self.rc) };
        if flags & mpv::RENDER_UPDATE_FRAME == 0 {
            return None;
        }
        self.render(width)
    }

    fn render(&self, width: u32) -> Option<(u32, u32, Vec<u8>)> {
        let (vw, vh) = self.video_size()?;
        let w = width.min(vw).max(2) & !1;
        let h = ((vh as f64 * w as f64 / vw as f64).round() as u32).max(2) & !1;
        let mut size: [c_int; 2] = [w as c_int, h as c_int];
        let fmt = CString::new("rgb0").unwrap_or_default();
        let mut stride: usize = w as usize * 4;
        let mut buf = vec![0u8; stride * h as usize];
        let mut params = [
            RenderParam { kind: RENDER_PARAM_SW_SIZE, data: size.as_mut_ptr() as *mut c_void },
            RenderParam { kind: RENDER_PARAM_SW_FORMAT, data: fmt.as_ptr() as *mut c_void },
            RenderParam { kind: RENDER_PARAM_SW_STRIDE, data: &mut stride as *mut usize as *mut c_void },
            RenderParam { kind: RENDER_PARAM_SW_POINTER, data: buf.as_mut_ptr() as *mut c_void },
            RenderParam::END,
        ];
        if unsafe { (self.api.render)(self.rc, params.as_mut_ptr()) } < 0 {
            return None;
        }
        for px in buf.chunks_exact_mut(4) {
            px[3] = 255;
        }
        Some((w, h, buf))
    }

    fn get_f64(&self, k: &str) -> Option<f64> {
        self.m.get_property(k).and_then(|s| s.parse().ok())
    }

    /// Vide la file d'événements : fichier chargé, échec, image en place après un saut.
    fn drain(&self) -> (bool, bool, bool) {
        let (mut loaded, mut failed, mut restart) = (false, false, false);
        while let Some(ev) = self.m.next_event() {
            match ev.event_id {
                mpv::EVENT_FILE_LOADED => loaded = true,
                mpv::EVENT_PLAYBACK_RESTART => restart = true,
                mpv::EVENT_END_FILE => {
                    let end = unsafe { &*(ev.data as *const mpv::RawEndFile) };
                    failed |= end.reason == mpv::END_FILE_ERROR;
                }
                _ => {}
            }
        }
        (loaded, failed, restart)
    }
}

/// Demande en cours dans le processus d'aperçu : servie quand mpv a l'image demandée.
struct Waiting {
    seq: u64,
    since: Instant,
    /// Vrai : servie à la prochaine image rendue (pas en avant) ; faux : à la fin du saut.
    on_frame: bool,
}

impl Drop for SwPlayer {
    fn drop(&mut self) {
        unsafe { (self.api.free)(self.rc) };
    }
}

/// Frise en cours de fabrication.
struct StripJob {
    path: String,
    count: usize,
    /// Instants demandés (frise zoomée) ; vide : `count` vignettes réparties sur la durée.
    times: Vec<f64>,
    next: usize,
    duration: f64,
    /// Attente de l'image demandée (saut lancé à cet instant).
    waiting: Option<Instant>,
    loaded: bool,
}

/// Mode `--preview` : sert l'aperçu jusqu'à la fermeture de l'entrée standard.
pub fn run() {
    let live = match SwPlayer::new() {
        Ok(p) => p,
        Err(e) => {
            send_event(&PreviewEvent::Missing { detail: e });
            return;
        }
    };
    let strip = SwPlayer::new().ok();
    send_event(&PreviewEvent::Ready { mpv: live.m.get_property("mpv-version").unwrap_or_default() });

    let (tx, rx): (Sender<Option<PreviewCmd>>, Receiver<Option<PreviewCmd>>) = channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if let Ok(c) = serde_json::from_str(&line) {
                if tx.send(Some(c)).is_err() {
                    return;
                }
            }
        }
        let _ = tx.send(None);
    });

    let mut current: Option<String> = None;
    let mut job: Option<StripJob> = None;
    // Demande d'images de détail arrivée pendant la frise entière, et fichier ouvert sur le
    // lecteur de la frise (gardé ouvert pour servir le détail sans le rouvrir).
    let mut queued: Option<StripJob> = None;
    let mut strip_open: Option<(String, f64)> = None;
    let mut last_pos = Instant::now();
    let mut sent_pos: Option<(f64, bool, u64)> = None;
    // Demandes servies : les positions envoyées portent le numéro de la dernière, et la régie
    // ignore celles d'avant sa dernière demande (plus de tête de lecture qui revient en arrière).
    let mut seq_done: u64 = 0;
    let mut waiting: Option<Waiting> = None;
    let mut steps: std::collections::VecDeque<(bool, u64)> = std::collections::VecDeque::new();
    loop {
        match rx.recv_timeout(Duration::from_millis(8)) {
            Ok(None) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(Some(cmd)) => match cmd {
                PreviewCmd::Open { path, at, seq } => {
                    let _ = live.m.set_property("pause", "yes");
                    let _ = live.m.set_property("start", &format!("{:.3}", at.max(0.0)));
                    if let Err(e) = live.m.command(&["loadfile", &path, "replace"]) {
                        send_event(&PreviewEvent::Error { message: e });
                    }
                    current = Some(path);
                    sent_pos = None;
                    steps.clear();
                    waiting = Some(Waiting { seq, since: Instant::now(), on_frame: false });
                }
                PreviewCmd::Close => {
                    let _ = live.m.command(&["stop"]);
                    current = None;
                    steps.clear();
                    waiting = None;
                }
                PreviewCmd::Seek { seconds, exact, seq } => {
                    steps.clear();
                    let _ = live.m.command(&["seek", &format!("{:.3}", seconds.max(0.0)), if exact { "absolute+exact" } else { "absolute+keyframes" }]);
                    waiting = Some(Waiting { seq, since: Instant::now(), on_frame: false });
                }
                PreviewCmd::Play => {
                    steps.clear();
                    let _ = live.m.set_property("pause", "no");
                }
                PreviewCmd::Pause => {
                    let _ = live.m.set_property("pause", "yes");
                }
                PreviewCmd::Step { back, seq } => {
                    steps.push_back((back, seq));
                }
                PreviewCmd::Strip { path, count } => {
                    queued = None;
                    strip_open = None;
                    if let Some(s) = &strip {
                        let _ = s.m.set_property("start", "0");
                        if s.m.command(&["loadfile", &path, "replace"]).is_ok() {
                            job = Some(StripJob { path, count: count.clamp(1, 200), times: Vec::new(), next: 0, duration: 0.0, waiting: None, loaded: false });
                        }
                    }
                }
                PreviewCmd::Frames { path, mut times } => {
                    times.truncate(200);
                    let j = StripJob { path, count: times.len(), times, next: 0, duration: 0.0, waiting: None, loaded: false };
                    if j.times.is_empty() {
                        if job.as_ref().is_some_and(|j| !j.times.is_empty()) {
                            job = None;
                        }
                        queued = None;
                    } else if job.as_ref().is_some_and(|j| j.times.is_empty()) {
                        queued = Some(j);
                    } else if let Some(s) = &strip {
                        job = start_frames(s, j, &mut strip_open);
                    }
                }
            },
            Err(RecvTimeoutError::Timeout) => {}
        }

        // Aperçu : nouvel état du fichier, position, image.
        let (loaded, failed, restart) = live.drain();
        if loaded {
            let image = live.m.get_property("current-tracks/video/image").as_deref() == Some("yes");
            let fps = live.get_f64("container-fps").or_else(|| live.get_f64("estimated-vf-fps")).filter(|f| f.is_finite() && *f > 1.0).unwrap_or(0.0);
            let rotate = live.get_f64("current-tracks/video/demux-rotation").map(|r| (r.round() as i64).rem_euclid(360) as u16 / 90 * 90).unwrap_or(0);
            send_event(&PreviewEvent::Opened { path: current.clone().unwrap_or_default(), duration: live.get_f64("duration").unwrap_or(0.0), image, fps, rotate });
        }
        if failed {
            send_event(&PreviewEvent::Error { message: format!("aperçu impossible ({})", current.clone().unwrap_or_default()) });
        }
        let frame = live.render_if_new(LIVE_W);
        let new_frame = frame.is_some();
        if let Some((w, h, rgba)) = frame {
            send_frame(&FrameMsg { tag: 0, w, h, rgba });
        }
        // Demande servie : image en place après le saut, ou image suivante rendue ; au pire
        // au bout de 2 s (fichier sans image, saut refusé).
        if let Some(w) = &waiting {
            let served = failed || (if w.on_frame { new_frame } else { restart }) || w.since.elapsed() > Duration::from_secs(2);
            if served {
                seq_done = seq_done.max(w.seq);
                waiting = None;
                sent_pos = None;
                last_pos = Instant::now() - Duration::from_millis(40);
            }
        }
        // Pas suivant, une fois le précédent servi.
        if waiting.is_none() && current.is_some() {
            if let Some((back, seq)) = steps.pop_front() {
                let _ = live.m.set_property("pause", "yes");
                let _ = live.m.command(&[if back { "frame-back-step" } else { "frame-step" }]);
                waiting = Some(Waiting { seq, since: Instant::now(), on_frame: !back });
            }
        }
        if current.is_some() && last_pos.elapsed() >= Duration::from_millis(40) {
            last_pos = Instant::now();
            let t = live.get_f64("time-pos").unwrap_or(0.0);
            let paused = live.m.get_property("pause").as_deref() != Some("no");
            if sent_pos.is_none_or(|(st, sp, sq)| (t - st).abs() > 0.001 || paused != sp || sq != seq_done) {
                sent_pos = Some((t, paused, seq_done));
                send_event(&PreviewEvent::Position { time: t, paused, seq: seq_done });
            }
        }

        // Frise : une vignette à la fois, sans gêner l'aperçu.
        if let (Some(s), Some(j)) = (&strip, &mut job) {
            let (loaded, failed, _) = s.drain();
            if failed {
                job = None;
                continue;
            }
            if loaded {
                j.loaded = true;
                j.duration = s.get_f64("duration").unwrap_or(0.0);
                strip_open = Some((j.path.clone(), j.duration));
                if j.times.is_empty() {
                    send_event(&PreviewEvent::StripInfo { path: j.path.clone(), duration: j.duration, count: j.count });
                }
            }
            if !j.loaded {
                continue;
            }
            match j.waiting {
                None => {
                    // Vignette n° i : l'image exacte à i × durée / nombre, là où la frise la
                    // dessine (son bord gauche tombe sur cet instant).
                    let t = match j.times.get(j.next) {
                        Some(t) => t.clamp(0.0, j.duration.max(0.0)),
                        None => j.next as f64 * j.duration / j.count as f64,
                    };
                    s.dirty.store(false, Ordering::Release);
                    let _ = s.m.command(&["seek", &format!("{t:.3}"), "absolute+exact"]);
                    j.waiting = Some(Instant::now());
                }
                Some(since) => {
                    let frame = s.render_if_new(STRIP_W);
                    let late = since.elapsed() > Duration::from_secs(3);
                    if frame.is_some() || late {
                        if let Some((w, h, rgba)) = frame.or_else(|| if late { s.render(STRIP_W) } else { None }) {
                            let tag = match j.times.get(j.next) {
                                Some(t) => FRAME_AT | ((t * 1000.0).round().clamp(0.0, (FRAME_AT - 1) as f64) as u32),
                                None => 1 + j.next as u32,
                            };
                            send_frame(&FrameMsg { tag, w, h, rgba });
                        }
                        j.next += 1;
                        j.waiting = None;
                        if j.next >= j.count || j.duration <= 0.0 {
                            // Le fichier reste ouvert : les images de détail en ont besoin.
                            job = queued.take().and_then(|q| start_frames(s, q, &mut strip_open));
                        }
                    }
                }
            }
        }
    }
}

/// Lance une demande d'images de détail sur le lecteur de la frise (le fichier n'est rouvert
/// que s'il a changé).
fn start_frames(s: &SwPlayer, mut j: StripJob, open: &mut Option<(String, f64)>) -> Option<StripJob> {
    send_event(&PreviewEvent::FramesFor { path: j.path.clone() });
    match open {
        Some((p, d)) if *p == j.path => {
            j.loaded = true;
            j.duration = *d;
        }
        _ => {
            *open = None;
            let _ = s.m.set_property("start", "0");
            s.m.command(&["loadfile", &j.path, "replace"]).ok()?;
        }
    }
    Some(j)
}

// --- Côté régie -------------------------------------------------------------------------------

/// Image reçue (RGBA), avec un numéro qui change à chaque nouvelle image.
#[derive(Default)]
struct Pixels {
    w: usize,
    h: usize,
    rgba: Vec<u8>,
    serial: u64,
}

#[derive(Default)]
struct Shared {
    live: Pixels,
    strip: Vec<Option<Pixels>>,
    strip_serial: u64,
    /// Fichier des images de détail qui arrivent, et images arrivées (fichier, ms, pixels).
    detail_path: Option<String>,
    detail: Vec<(String, u32, Pixels)>,
    events: Vec<PreviewEvent>,
}

/// État de l'aperçu tel que la régie le voit.
#[derive(Default, Clone)]
pub struct PreviewState {
    pub available: bool,
    pub missing: Option<String>,
    /// Média ouvert dans l'aperçu, sa durée et sa nature.
    pub path: Option<PathBuf>,
    pub duration: f64,
    pub image: bool,
    /// Images par seconde du média (0 : inconnue).
    pub fps: f64,
    /// Rotation demandée par le fichier (métadonnées) : mpv l'applique sur la sortie, l'aperçu
    /// la dessine lui-même.
    pub rotate: u16,
    pub time: f64,
    pub paused: bool,
    pub error: Option<String>,
    /// Frise prête pour ce média : (chemin, durée, nombre de vignettes).
    pub strip: Option<(PathBuf, f64, usize)>,
}

/// L'aperçu côté régie : lance le processus, lui envoie les commandes, garde la dernière image.
pub struct Preview {
    child: Option<std::process::Child>,
    stdin: Option<std::process::ChildStdin>,
    shared: Arc<Mutex<Shared>>,
    pub state: PreviewState,
    live_tex: Option<(egui::TextureHandle, u64)>,
    strip_tex: Vec<Option<egui::TextureHandle>>,
    strip_seen: u64,
    /// Images de détail de la frise zoomée : fichier, images reçues (instant en ms), dernière
    /// demande envoyée.
    detail_for: Option<PathBuf>,
    detail_tex: std::collections::HashMap<u32, egui::TextureHandle>,
    detail_asked: Vec<u32>,
    started: bool,
    /// Dernière demande de déplacement envoyée (pour ne pas inonder le processus).
    last_seek: Option<(f64, bool)>,
    /// Position approchée pendant un glisser, à rendre exacte au relâché.
    pending_exact: Option<f64>,
    /// Numéro de la dernière demande envoyée (saut, pas, ouverture) : les positions plus
    /// anciennes sont ignorées.
    seq: u64,
}

impl Default for Preview {
    fn default() -> Self {
        Self {
            child: None,
            stdin: None,
            shared: Arc::default(),
            state: PreviewState::default(),
            live_tex: None,
            strip_tex: Vec::new(),
            strip_seen: 0,
            detail_for: None,
            detail_tex: Default::default(),
            detail_asked: Vec::new(),
            started: false,
            last_seek: None,
            pending_exact: None,
            seq: 0,
        }
    }
}

fn read_msg(r: &mut impl Read) -> Option<(u8, Vec<u8>)> {
    let mut head = [0u8; 5];
    r.read_exact(&mut head).ok()?;
    let len = u32::from_le_bytes([head[1], head[2], head[3], head[4]]) as usize;
    if len > 64 << 20 {
        return None;
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).ok()?;
    Some((head[0], buf))
}

impl Preview {
    /// Lance le processus d'aperçu à la première utilisation.
    fn ensure(&mut self, ctx: &egui::Context) {
        if self.started {
            return;
        }
        self.started = true;
        let Ok(exe) = std::env::current_exe() else { return };
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("--preview").stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000);
        }
        let Ok(mut child) = cmd.spawn() else {
            self.state.missing = Some("processus d'aperçu impossible à lancer".into());
            return;
        };
        self.stdin = child.stdin.take();
        if let Some(mut stdout) = child.stdout.take() {
            let shared = self.shared.clone();
            let ctx = ctx.clone();
            std::thread::Builder::new()
                .name("aperçu".into())
                .spawn(move || {
                    while let Some((kind, buf)) = read_msg(&mut stdout) {
                        let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                        if kind == 0 {
                            if let Ok(ev) = serde_json::from_slice::<PreviewEvent>(&buf) {
                                if let PreviewEvent::FramesFor { path } = &ev {
                                    s.detail_path = Some(path.clone());
                                }
                                s.events.push(ev);
                            }
                        } else if buf.len() >= 12 {
                            let u = |i: usize| u32::from_le_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]) as usize;
                            let (tag, w, h) = (u(0), u(4), u(8));
                            if buf.len() >= 12 + w * h * 4 {
                                let px = Pixels { w, h, rgba: buf[12..12 + w * h * 4].to_vec(), serial: 0 };
                                if tag == 0 {
                                    let serial = s.live.serial + 1;
                                    s.live = Pixels { serial, ..px };
                                } else if tag as u32 & FRAME_AT != 0 {
                                    if let Some(p) = s.detail_path.clone() {
                                        s.detail.push((p, tag as u32 & !FRAME_AT, px));
                                    }
                                } else {
                                    let i = tag - 1;
                                    if s.strip.len() <= i {
                                        s.strip.resize_with(i + 1, || None);
                                    }
                                    s.strip[i] = Some(px);
                                    s.strip_serial += 1;
                                }
                            }
                        }
                        drop(s);
                        ctx.request_repaint();
                    }
                    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                    s.events.push(PreviewEvent::Missing { detail: "le processus d'aperçu s'est arrêté".into() });
                    ctx.request_repaint();
                })
                .ok();
        }
        self.child = Some(child);
    }

    fn send(&mut self, ctx: &egui::Context, c: PreviewCmd) {
        self.ensure(ctx);
        let Some(stdin) = &mut self.stdin else { return };
        if let Ok(s) = serde_json::to_string(&c) {
            if writeln!(stdin, "{s}").and_then(|_| stdin.flush()).is_err() {
                self.stdin = None;
            }
        }
    }

    /// À appeler à chaque image de la régie : lit les événements du processus.
    pub fn poll(&mut self) {
        let events = std::mem::take(&mut self.shared.lock().unwrap_or_else(|e| e.into_inner()).events);
        for ev in events {
            match ev {
                PreviewEvent::Ready { .. } => {
                    self.state.available = true;
                    self.state.missing = None;
                }
                PreviewEvent::Missing { detail } => {
                    self.state.available = false;
                    self.state.missing = Some(detail);
                }
                PreviewEvent::Opened { path, duration, image, fps, rotate } => {
                    if self.state.path.as_deref() == Some(Path::new(&path)) {
                        self.state.duration = duration;
                        self.state.image = image;
                        self.state.fps = fps;
                        self.state.rotate = rotate;
                        self.state.error = None;
                    }
                }
                PreviewEvent::Position { time, paused, seq } => {
                    // Position d'avant la dernière demande : la tête de lecture ne saute pas
                    // en arrière pendant qu'on cherche une image.
                    if seq >= self.seq {
                        self.state.time = time;
                    }
                    self.state.paused = paused;
                }
                PreviewEvent::StripInfo { path, duration, count } => self.state.strip = Some((PathBuf::from(path), duration, count)),
                PreviewEvent::FramesFor { .. } => {}
                PreviewEvent::Error { message } => self.state.error = Some(message),
            }
        }
    }

    /// Ouvre `path` dans l'aperçu (sans effet s'il y est déjà).
    pub fn open(&mut self, ctx: &egui::Context, path: &Path, at: f64) {
        if self.state.path.as_deref() == Some(path) {
            return;
        }
        self.state.path = Some(path.to_path_buf());
        self.state.duration = 0.0;
        self.state.fps = 0.0;
        self.state.rotate = 0;
        self.state.time = at;
        self.state.image = false;
        self.state.error = None;
        self.last_seek = None;
        self.live_tex = None;
        self.seq += 1;
        let seq = self.seq;
        self.send(ctx, PreviewCmd::Open { path: path.to_string_lossy().into_owned(), at, seq });
    }

    pub fn seek(&mut self, ctx: &egui::Context, seconds: f64, exact: bool) {
        self.pending_exact = (!exact).then_some(seconds);
        if self.last_seek == Some((seconds, exact)) {
            return;
        }
        self.last_seek = Some((seconds, exact));
        self.state.time = seconds;
        self.seq += 1;
        let seq = self.seq;
        self.send(ctx, PreviewCmd::Seek { seconds, exact, seq });
    }

    /// À chaque image : un glisser terminé ramène l'aperçu sur l'image exacte.
    pub fn settle(&mut self, ctx: &egui::Context) {
        if self.pending_exact.is_some() && !ctx.input(|i| i.pointer.any_down()) {
            if let Some(t) = self.pending_exact.take() {
                self.seek(ctx, t, true);
            }
        }
    }

    pub fn play(&mut self, ctx: &egui::Context, on: bool) {
        self.last_seek = None;
        self.send(ctx, if on { PreviewCmd::Play } else { PreviewCmd::Pause });
    }

    pub fn step(&mut self, ctx: &egui::Context, back: bool) {
        self.last_seek = None;
        self.pending_exact = None;
        // Affichage tout de suite à l'image visée ; la position exacte suit.
        if self.state.fps > 0.0 {
            let d = 1.0 / self.state.fps;
            self.state.time = (self.state.time + if back { -d } else { d }).clamp(0.0, self.state.duration.max(0.0));
        }
        self.seq += 1;
        let seq = self.seq;
        self.send(ctx, PreviewCmd::Step { back, seq });
    }

    /// Vrai si l'aperçu montre `path` et en connaît la durée.
    pub fn shows(&self, path: &Path) -> bool {
        self.state.path.as_deref() == Some(path) && (self.state.duration > 0.0 || self.state.image)
    }

    /// Demande la frise de `path` : `count` vignettes, la n° i étant l'image à i × durée / `count`.
    pub fn request_strip(&mut self, ctx: &egui::Context, path: &Path, count: usize) {
        if self.state.strip.as_ref().is_some_and(|s| s.0 == path) {
            return;
        }
        self.state.strip = Some((path.to_path_buf(), 0.0, count));
        {
            let mut s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
            s.strip.clear();
            s.strip_serial += 1;
        }
        self.strip_tex.clear();
        self.send(ctx, PreviewCmd::Strip { path: path.to_string_lossy().into_owned(), count });
    }

    /// Image de l'aperçu, pour le média ouvert.
    pub fn texture(&mut self, ctx: &egui::Context) -> Option<&egui::TextureHandle> {
        let s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        let fresh = s.live.serial > 0 && self.live_tex.as_ref().is_none_or(|(_, n)| *n != s.live.serial);
        if fresh && s.live.w > 0 {
            let img = egui::ColorImage::from_rgba_unmultiplied([s.live.w, s.live.h], &s.live.rgba);
            let serial = s.live.serial;
            drop(s);
            match &mut self.live_tex {
                Some((t, n)) => {
                    t.set(img, egui::TextureOptions::LINEAR);
                    *n = serial;
                }
                None => self.live_tex = Some((ctx.load_texture("apercu", img, egui::TextureOptions::LINEAR), serial)),
            }
        }
        self.live_tex.as_ref().map(|(t, _)| t)
    }

    /// Demande les images exactes de `path` aux instants `times` (frise zoomée) ; seules celles
    /// qui manquent partent, et une demande identique à la précédente n'est pas renvoyée.
    pub fn request_frames(&mut self, ctx: &egui::Context, path: &Path, times: &[f64]) {
        if self.detail_for.as_deref() != Some(path) {
            self.detail_for = Some(path.to_path_buf());
            self.detail_tex.clear();
            self.detail_asked.clear();
        }
        let missing: Vec<u32> = times.iter().map(|t| ms(*t)).filter(|m| !self.detail_tex.contains_key(m)).collect();
        if missing == self.detail_asked {
            return;
        }
        self.detail_asked = missing.clone();
        self.send(ctx, PreviewCmd::Frames { path: path.to_string_lossy().into_owned(), times: missing.iter().map(|m| *m as f64 / 1000.0).collect() });
    }

    /// Images de détail de `path` reçues jusqu'ici, par instant (ms, voir `ms`).
    pub fn detail_textures(&mut self, ctx: &egui::Context, path: &Path) -> &std::collections::HashMap<u32, egui::TextureHandle> {
        let new = std::mem::take(&mut self.shared.lock().unwrap_or_else(|e| e.into_inner()).detail);
        if self.detail_for.as_deref() != Some(path) {
            self.detail_for = Some(path.to_path_buf());
            self.detail_tex.clear();
            self.detail_asked.clear();
        }
        let want = path.to_string_lossy();
        for (p, at, px) in new {
            if p == want {
                // Mémoire bornée : au-delà, on repart de zéro (les images visibles reviennent).
                if self.detail_tex.len() >= 400 {
                    self.detail_tex.clear();
                    self.detail_asked.clear();
                }
                let img = egui::ColorImage::from_rgba_unmultiplied([px.w, px.h], &px.rgba);
                self.detail_tex.insert(at, ctx.load_texture(format!("detail-{at}"), img, egui::TextureOptions::LINEAR));
            }
        }
        &self.detail_tex
    }

    /// Vignettes de la frise reçues jusqu'ici (None : pas encore arrivée).
    pub fn strip_textures(&mut self, ctx: &egui::Context) -> &[Option<egui::TextureHandle>] {
        let s = self.shared.lock().unwrap_or_else(|e| e.into_inner());
        if s.strip_serial != self.strip_seen {
            self.strip_seen = s.strip_serial;
            self.strip_tex.resize_with(s.strip.len(), || None);
            for (i, px) in s.strip.iter().enumerate() {
                if let (Some(px), None) = (px, &self.strip_tex[i]) {
                    let img = egui::ColorImage::from_rgba_unmultiplied([px.w, px.h], &px.rgba);
                    self.strip_tex[i] = Some(ctx.load_texture(format!("frise-{i}"), img, egui::TextureOptions::LINEAR));
                }
            }
        }
        &self.strip_tex
    }
}

/// Instant en millisecondes, clé des images de détail.
pub fn ms(t: f64) -> u32 {
    (t.max(0.0) * 1000.0).round().min((FRAME_AT - 1) as f64) as u32
}

impl Drop for Preview {
    fn drop(&mut self) {
        self.stdin = None;
        if let Some(mut c) = self.child.take() {
            // Attente bornée : la régie ne se fige pas sur un aperçu qui tarde à disparaître.
            let _ = c.kill();
            for _ in 0..40 {
                if !matches!(c.try_wait(), Ok(None)) {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }
    }
}
