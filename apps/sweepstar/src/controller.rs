//! État de SweepStar hors interface : préférences, périphériques, signal, mesures et traces.

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::Arc;

use crossbeam_channel::{bounded, Receiver};
use serde::{Deserialize, Serialize};
use star_core::app::App;
use star_core::i18n::{tr, Lang};
use star_measure::gen::{PeriodicPink, PinkNoise, Sine, WhiteNoise};
use star_measure::sweep::{Sweep, SweepParams};

use crate::audio::engine::{AudioEngine, DeviceInfo, OpenRequest, MAX_INPUTS, NO_CHANNEL};
use crate::audio::decode::{decode, is_audio_file};
use crate::audio::signal::{OutCmd, Source, Track};
use crate::measure::{MeasureJob, Measurement, RtaFrame, TransferFrame, Worker, WorkerEvent};
use crate::traces::{self, RtaSnapshot, Trace, TraceData, TraceKind, TransferSnapshot};
use star_measure::transfer::{Averaging, TransferConfig};

pub const APP: App = App::new("SweepStar");

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AudioPrefs {
    pub host: Option<String>,
    pub output: Option<String>,
    pub input: Option<String>,
    pub buffer_size: Option<u32>,
    /// Sorties qui reçoivent le signal (à partir de 0).
    pub out_channels: Vec<usize>,
    /// Entrée de la boucle de référence (à partir de 0), ou aucune.
    pub ref_channel: Option<usize>,
    pub mic_channel: usize,
}

impl Default for AudioPrefs {
    fn default() -> Self {
        Self { host: None, output: None, input: None, buffer_size: None, out_channels: vec![0], ref_channel: None, mic_channel: 0 }
    }
}

impl AudioPrefs {
    fn channel_mask(&self) -> u64 {
        self.out_channels.iter().filter(|&&c| c < 64).fold(0, |m, &c| m | (1 << c))
    }

    fn request(&self) -> OpenRequest {
        OpenRequest { host: self.host.clone(), output: self.output.clone(), input: self.input.clone(), buffer_size: self.buffer_size }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SignalKind {
    #[default]
    Pink,
    White,
    PeriodicPink,
    Sine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RtaWeighting {
    A,
    C,
    #[default]
    Z,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub lang: Lang,
    pub audio: AudioPrefs,
    /// Niveau des générateurs et du sweep (dBFS crête).
    pub level_db: f32,
    pub signal: SignalKind,
    pub sine_hz: f64,
    pub sweep_secs: f64,
    pub f1: f64,
    pub f2: f64,
    /// Silence enregistré après le sweep (s).
    pub tail_secs: f64,
    /// Température de l'air (°C), pour la conversion ms → m.
    pub temperature_c: f64,
    /// Lissage en 1/n d'octave (0 : aucun).
    pub smoothing: u32,
    pub rta_fraction: u32,
    pub rta_weighting: RtaWeighting,
    /// Moyenne exponentielle du RTA (poids du nouveau spectre).
    pub rta_speed: f64,
    /// RTA en courbe continue (bande glissante) plutôt qu'en barres.
    pub rta_curve: bool,
    /// Lissage du spectre du RTA en 1/n d'octave (0 : aucun).
    pub rta_smoothing: u32,
    /// Dossier des mesures enregistrées (`None` : Documents/SweepStar/Mesures).
    pub measures_dir: Option<PathBuf>,
    /// Calibration du micro : dB SPL = dBFS + cette valeur (`None` : pas calibré).
    pub mic_cal_db: Option<f64>,
    /// Fonction de transfert : taille de la FFT, blocs moyennés (0 : moyenne infinie),
    /// lissage (1/n d'octave), retard de la référence (ms).
    pub tf_fft: usize,
    pub tf_avg: u32,
    pub tf_smoothing: u32,
    pub tf_delay_ms: f64,
    /// Suivi automatique du délai (recherche toutes les secondes).
    pub tf_delay_track: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            lang: Lang::Fr,
            audio: AudioPrefs::default(),
            level_db: -20.0,
            signal: SignalKind::Pink,
            sine_hz: 1000.0,
            sweep_secs: 3.0,
            f1: 20.0,
            f2: 20_000.0,
            tail_secs: 1.0,
            temperature_c: 20.0,
            smoothing: 6,
            rta_fraction: 3,
            rta_weighting: RtaWeighting::Z,
            rta_speed: 0.25,
            rta_curve: true,
            rta_smoothing: 0,
            measures_dir: None,
            mic_cal_db: None,
            tf_fft: 16_384,
            tf_avg: 16,
            tf_smoothing: 12,
            tf_delay_ms: 0.0,
            tf_delay_track: false,
        }
    }
}

impl Prefs {
    /// Remet dans leurs bornes les valeurs d'un fichier modifié à la main.
    fn sanitize(&mut self) {
        let fix = |v: &mut f64, d: f64, lo: f64, hi: f64| {
            if !v.is_finite() {
                *v = d;
            }
            *v = v.clamp(lo, hi);
        };
        fix(&mut self.sweep_secs, 3.0, 1.0, 16.0);
        fix(&mut self.f1, 20.0, 5.0, 1000.0);
        fix(&mut self.f2, 20_000.0, 1000.0, 24_000.0);
        fix(&mut self.tail_secs, 1.0, 0.1, 10.0);
        fix(&mut self.temperature_c, 20.0, -30.0, 50.0);
        fix(&mut self.sine_hz, 1000.0, 10.0, 20_000.0);
        fix(&mut self.rta_speed, 0.25, 0.01, 1.0);
        fix(&mut self.tf_delay_ms, 0.0, 0.0, 1000.0);
        if ![4096, 8192, 16_384, 32_768, 65_536].contains(&self.tf_fft) {
            self.tf_fft = 16_384;
        }
        self.tf_avg = self.tf_avg.min(128);
        if self.mic_cal_db.is_some_and(|c| !c.is_finite()) {
            self.mic_cal_db = None;
        }
        if !self.level_db.is_finite() {
            self.level_db = -20.0;
        }
        self.level_db = self.level_db.clamp(-80.0, 0.0);
    }
}

/// Session restaurée au lancement : la playlist et ses réglages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub playlist: Vec<PathBuf>,
    /// Morceaux joués en boucle (chemins).
    pub looped: Vec<PathBuf>,
    /// Niveau de la playlist (dB, 0 = niveau du fichier).
    pub playlist_db: f32,
    /// Mesures affichées (fichiers).
    pub visible_traces: Vec<PathBuf>,
}

impl Default for Session {
    fn default() -> Self {
        Self { playlist: Vec::new(), looped: Vec::new(), playlist_db: -20.0, visible_traces: Vec::new() }
    }
}


/// Couleurs des traces, dans l'ordre.
pub const TRACE_COLORS: [[u8; 3]; 24] = [
    [0x4a, 0x8f, 0xdb], [0x4f, 0xae, 0x58], [0xe2, 0x7a, 0x30], [0x9b, 0x6b, 0xd3], [0x3f, 0xb8, 0xaf], [0xd9, 0x53, 0x4f],
    [0xe2, 0xb2, 0x32], [0xd8, 0x6a, 0xb0], [0x8f, 0xc9, 0x3a], [0x2f, 0x6c, 0xb5], [0xf0, 0x97, 0x6b], [0x6a, 0xd0, 0xf0],
    [0xb5, 0x3a, 0x6e], [0x2e, 0x8b, 0x57], [0xc8, 0xa0, 0xf0], [0xa0, 0x6b, 0x2c], [0xf5, 0xe0, 0x6e], [0x5c, 0x5c, 0xc8],
    [0xff, 0x9f, 0xc8], [0x8a, 0x8a, 0x90], [0x1f, 0xa8, 0x8a], [0xe0, 0x45, 0x20], [0xb0, 0xd8, 0xb0], [0xff, 0xff, 0xff],
];

pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

pub struct Controller {
    pub prefs: Prefs,
    engine: AudioEngine,
    worker: Worker,
    pub device: Option<DeviceInfo>,
    pub device_error: Option<String>,
    opening: Option<Receiver<Result<DeviceInfo, String>>>,
    /// Mesure en cours : avancement (0 à 1).
    pub measuring: Option<f32>,
    /// Dernière mesure (pas encore figée).
    pub current: Option<Arc<Measurement>>,
    pub traces: Vec<Trace>,
    pub status: Option<String>,
    /// Niveaux affichés (dBFS, avec retombée) : entrées, sortie.
    pub in_meters: [f32; MAX_INPUTS],
    pub out_meter: f32,
    /// Écritures perdues du tampon d'entrée depuis le lancement.
    pub overruns: u64,
    measure_count: usize,
    pub session: Session,
    /// Morceau sélectionné (joué, ou en cours de chargement).
    pub track: Option<usize>,
    /// Durée du morceau en cours (images).
    pub track_frames: usize,
    loading: Option<(usize, Receiver<Result<Track, String>>)>,
    files_done_seen: u64,
    /// Réglages envoyés au fil de mesure pour la fonction de transfert (`None` : arrêtée).
    tf_sent: Option<TransferConfig>,
    /// Recherche du délai en cours.
    pub finding_delay: bool,
    /// Dernière recherche du délai (suivi automatique).
    last_find: Option<std::time::Instant>,
    /// Réécritures de fichiers de mesure en attente (couleur changée au sélecteur).
    dirty: Vec<(usize, std::time::Instant)>,
}

impl Controller {
    pub fn new(wake: impl Fn() + Send + 'static) -> Self {
        let mut prefs: Prefs = APP.config_file("preferences.json").and_then(|p| star_core::prefs::load(&p)).unwrap_or_default();
        prefs.sanitize();
        star_core::i18n::set_lang(prefs.lang);
        let engine = AudioEngine::new(db_to_gain(prefs.level_db), prefs.audio.channel_mask());
        let worker = Worker::new(engine.ring.clone(), engine.out_cmd.clone(), wake);
        let mut c = Self {
            prefs,
            engine,
            worker,
            device: None,
            device_error: None,
            opening: None,
            measuring: None,
            current: None,
            traces: Vec::new(),
            status: None,
            in_meters: [-120.0; MAX_INPUTS],
            out_meter: -120.0,
            overruns: 0,
            measure_count: 0,
            session: APP.session_path().and_then(|p| star_core::prefs::load(&p)).unwrap_or_default(),
            track: None,
            track_frames: 0,
            loading: None,
            files_done_seen: 0,
            tf_sent: None,
            finding_delay: false,
            last_find: None,
            dirty: Vec::new(),
        };
        if !c.session.playlist_db.is_finite() {
            c.session.playlist_db = -20.0;
        }
        c.engine.out.set_file_gain(db_to_gain(c.session.playlist_db));
        c.apply_channels();
        c.open_device();
        c.load_library();
        c
    }

    pub fn save_prefs(&self) {
        if let Some(p) = APP.config_file("preferences.json") {
            if let Err(e) = star_core::prefs::save(&p, &self.prefs) {
                star_core::log::warn(&format!("préférences non enregistrées : {e}"));
            }
        }
    }

    pub fn save_session(&self) {
        if let Some(p) = APP.session_path() {
            if let Err(e) = star_core::prefs::save(&p, &self.session) {
                star_core::log::warn(&format!("session non enregistrée : {e}"));
            }
        }
    }

    // ------------------------------------------------------------------ playlist

    /// Ajoute des fichiers audio (les dossiers sont parcourus, sans leurs sous-dossiers).
    pub fn add_files(&mut self, paths: Vec<PathBuf>) {
        let mut added = 0;
        for p in paths {
            if p.is_dir() {
                let mut files: Vec<PathBuf> = std::fs::read_dir(&p).into_iter().flatten().flatten().map(|e| e.path()).filter(|f| is_audio_file(f)).collect();
                files.sort();
                added += files.len();
                self.session.playlist.extend(files);
            } else if is_audio_file(&p) {
                self.session.playlist.push(p);
                added += 1;
            }
        }
        if added == 0 {
            self.status = Some(tr("aucun fichier audio reconnu", "no audio file recognised").into());
        }
        self.save_session();
    }

    pub fn remove_track(&mut self, i: usize) {
        if i >= self.session.playlist.len() {
            return;
        }
        if self.track == Some(i) {
            self.stop_all();
            self.track = None;
        } else if let Some(t) = self.track.filter(|&t| t > i) {
            self.track = Some(t - 1);
        }
        let path = self.session.playlist.remove(i);
        if !self.session.playlist.contains(&path) {
            self.session.looped.retain(|p| *p != path);
        }
        self.save_session();
    }

    pub fn clear_playlist(&mut self) {
        if self.file_playing() {
            self.stop_all();
        }
        self.track = None;
        self.session.playlist.clear();
        self.session.looped.clear();
        self.save_session();
    }

    /// Déplace un morceau (glisser dans la liste).
    pub fn move_track(&mut self, from: usize, to: usize) {
        let n = self.session.playlist.len();
        if from >= n || to >= n || from == to {
            return;
        }
        let item = self.session.playlist.remove(from);
        self.session.playlist.insert(to, item);
        self.track = self.track.map(|t| {
            if t == from {
                to
            } else if from < t && t <= to {
                t - 1
            } else if to <= t && t < from {
                t + 1
            } else {
                t
            }
        });
        self.save_session();
    }

    /// Décode le morceau `i` en fond, puis le joue.
    pub fn play_track(&mut self, i: usize) {
        let Some(path) = self.session.playlist.get(i).cloned() else { return };
        if self.measuring.is_some() {
            return;
        }
        let rate = self.sample_rate() as u32;
        let (tx, rx) = bounded(1);
        let spawned = std::thread::Builder::new().name("decodage".into()).spawn(move || {
            let res = decode(&path, rate)
                .map(|d| {
                    if let Some(w) = &d.warning {
                        star_core::log::warn(&format!("{} : {w}", path.display()));
                    }
                    Track { left: d.left, right: d.right }
                })
                .map_err(|e| format!("{} : {e}", path.display()));
            let _ = tx.send(res);
        });
        if spawned.is_ok() {
            self.track = Some(i);
            self.loading = Some((i, rx));
        }
    }

    pub fn next_track(&mut self) {
        let n = self.session.playlist.len();
        if n > 0 {
            let i = self.track.map_or(0, |t| (t + 1).min(n - 1));
            self.play_track(i);
        }
    }

    pub fn prev_track(&mut self) {
        if !self.session.playlist.is_empty() {
            let i = self.track.map_or(0, |t| t.saturating_sub(1));
            self.play_track(i);
        }
    }

    pub fn is_loading_track(&self) -> bool {
        self.loading.is_some()
    }

    /// Un morceau de la playlist sonne.
    pub fn file_playing(&self) -> bool {
        self.engine.out.playing_file.load(Ordering::Relaxed)
    }

    /// Position et durée du morceau en cours (s).
    pub fn track_time(&self) -> (f64, f64) {
        let fs = self.sample_rate();
        (self.engine.out.file_pos.load(Ordering::Relaxed) as f64 / fs, self.track_frames as f64 / fs)
    }

    pub fn set_playlist_level(&mut self, db: f32) {
        self.session.playlist_db = db.clamp(-60.0, 0.0);
        self.engine.out.set_file_gain(db_to_gain(self.session.playlist_db));
    }

    /// Le morceau `i` est joué en boucle.
    pub fn is_looped(&self, i: usize) -> bool {
        self.session.playlist.get(i).is_some_and(|p| self.session.looped.contains(p))
    }

    /// Met le morceau `i` en boucle ou l'en retire ; s'il joue, la boucle change aussitôt.
    pub fn toggle_loop(&mut self, i: usize) {
        let Some(path) = self.session.playlist.get(i).cloned() else { return };
        let on = !self.session.looped.contains(&path);
        if on {
            self.session.looped.push(path);
        } else {
            self.session.looped.retain(|p| *p != path);
        }
        if self.track == Some(i) {
            self.engine.out.loop_file.store(on, Ordering::Relaxed);
        }
        self.save_session();
    }

    /// Saut à `secs` dans le morceau en cours.
    pub fn seek(&mut self, secs: f64) {
        if self.file_playing() {
            let frame = (secs.max(0.0) * self.sample_rate()) as usize;
            self.engine.send(OutCmd::Seek(frame.min(self.track_frames)));
        }
    }

    fn poll_playlist(&mut self) {
        if let Some((i, rx)) = &self.loading {
            if let Ok(res) = rx.try_recv() {
                let i = *i;
                self.loading = None;
                match res {
                    // Le morceau ne joue que s'il est toujours celui demandé.
                    Ok(t) if self.track == Some(i) && self.measuring.is_none() => {
                        self.track_frames = t.left.len();
                        let looping = self.is_looped(i);
                        self.engine.out.loop_file.store(looping, Ordering::Relaxed);
                        self.engine.out.set_file_gain(db_to_gain(self.session.playlist_db));
                        self.engine.send(OutCmd::Play(Box::new(Source::File { track: Arc::new(t), pos: 0, looping })));
                        self.status = self.session.playlist.get(i).and_then(|p| p.file_name()).map(|n| format!("♪ {}", n.to_string_lossy()));
                    }
                    Ok(_) => {}
                    Err(e) => self.status = Some(format!("{} : {e}", tr("lecture impossible", "cannot play"))),
                }
            }
        }
        // Fin d'un morceau (sans boucle) : le suivant, s'il y en a un.
        let done = self.engine.out.files_done.load(Ordering::Relaxed);
        if done != self.files_done_seen {
            self.files_done_seen = done;
            if let Some(t) = self.track {
                if t + 1 < self.session.playlist.len() {
                    self.play_track(t + 1);
                }
            }
        }
    }

    /// (Ré)ouvre les périphériques des préférences.
    pub fn open_device(&mut self) {
        self.stop_all();
        self.opening = Some(self.engine.request_open(self.prefs.audio.request()));
    }

    pub fn is_opening(&self) -> bool {
        self.opening.is_some()
    }

    /// Canaux de sortie, de référence et du micro (sans rouvrir le périphérique).
    pub fn apply_channels(&mut self) {
        let a = &self.prefs.audio;
        self.engine.out.channel_mask.store(a.channel_mask(), std::sync::atomic::Ordering::Relaxed);
        self.engine.input.ref_channel.store(a.ref_channel.unwrap_or(NO_CHANNEL), std::sync::atomic::Ordering::Relaxed);
        self.engine.input.mic_channel.store(a.mic_channel, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn sample_rate(&self) -> f64 {
        self.device.as_ref().map_or(48_000.0, |d| d.sample_rate as f64)
    }

    /// À chaque image : réponses des périphériques, de la mesure, vu-mètres.
    pub fn poll(&mut self, dt: f32) {
        if let Some(rx) = &self.opening {
            if let Ok(res) = rx.try_recv() {
                self.opening = None;
                match res {
                    Ok(info) => {
                        star_core::log::write(&format!("audio : {} · {} → {:?} · {} Hz", info.host, info.output, info.input, info.sample_rate));
                        self.worker.set_sample_rate(info.sample_rate as f64);
                        self.device_error = if info.input.is_none() { Some(tr("aucune entrée audio : la mesure est impossible", "no audio input: measuring is impossible").into()) } else { None };
                        self.device = Some(info);
                    }
                    Err(e) => {
                        star_core::log::warn(&format!("audio : {e}"));
                        self.device = None;
                        self.device_error = Some(e);
                    }
                }
            }
        }
        while let Ok(e) = self.engine.stream_errors.try_recv() {
            self.device_error = Some(e);
        }
        while let Ok(ev) = self.worker.events.try_recv() {
            match ev {
                WorkerEvent::Progress(p) => self.measuring = Some(p),
                WorkerEvent::DelayFound(res) => {
                    self.finding_delay = false;
                    let tracking = self.prefs.tf_delay_track;
                    match res {
                        Ok(samples) if samples >= 0.0 => {
                            let ms = samples * 1000.0 / self.sample_rate();
                            // En suivi, on ne change que pour un écart d'au moins un échantillon
                            // (chaque changement remet la moyenne à zéro).
                            let step = 1000.0 / self.sample_rate();
                            if !tracking || (ms - self.prefs.tf_delay_ms).abs() >= step {
                                self.set_tf_delay_ms(ms);
                                self.status = Some(format!("{} {ms:.2} ms", if tracking { tr("Délai suivi :", "Delay tracked:") } else { tr("Délai trouvé :", "Delay found:") }));
                            }
                        }
                        Err(_) | Ok(_) if tracking => {}
                        Ok(_) => self.status = Some(tr("le micro est en avance sur la référence : vérifiez les canaux", "the mic is ahead of the reference: check the channels").into()),
                        Err(e) => self.status = Some(e),
                    }
                }
                WorkerEvent::Done(res) => {
                    self.measuring = None;
                    match res {
                        Ok(m) => {
                            self.measure_count += 1;
                            self.status = Some(format!("{} {}", tr("Mesure", "Measurement"), self.measure_count));
                            self.current = Some(Arc::new(m));
                        }
                        Err(e) => self.status = Some(e),
                    }
                }
            }
        }
        self.poll_playlist();
        // Fichiers de mesure à réécrire, une fois le réglage stable.
        let due: Vec<usize> = self.dirty.iter().filter(|(_, t)| t.elapsed().as_millis() > 400).map(|(i, _)| *i).collect();
        if !due.is_empty() {
            self.dirty.retain(|(i, _)| !due.contains(i));
            for i in due {
                self.save_trace(i);
            }
        }
        self.overruns += self.engine.ring.take_overruns();
        // Vu-mètres : crête instantanée, retombée de 20 dB/s.
        let fall = 20.0 * dt;
        let db = |p: f32| if p > 0.0 { 20.0 * p.log10() } else { -120.0 };
        for (c, m) in self.in_meters.iter_mut().enumerate() {
            let v = db(self.engine.input.take_peak(c));
            *m = if v > *m { v } else { (*m - fall).max(-120.0) };
        }
        let v = db(self.engine.out.take_peak());
        self.out_meter = if v > self.out_meter { v } else { (self.out_meter - fall).max(-120.0) };
    }

    // ------------------------------------------------------------------ transfert

    fn transfer_config(&self) -> TransferConfig {
        let p = &self.prefs;
        TransferConfig {
            sample_rate: self.sample_rate(),
            fft_len: p.tf_fft,
            averaging: if p.tf_avg == 0 { Averaging::Infinite } else { Averaging::Linear(p.tf_avg as usize) },
            delay: self.tf_delay_samples(),
            ..Default::default()
        }
    }

    fn tf_delay_samples(&self) -> usize {
        (self.prefs.tf_delay_ms * self.sample_rate() / 1000.0).round().max(0.0) as usize
    }

    /// Lance ou arrête la fonction de transfert (appelé à chaque image avec la vue affichée) ;
    /// les réglages changés sont renvoyés au fil de mesure.
    pub fn run_transfer(&mut self, on: bool) {
        // Suivi automatique du délai : une recherche par seconde.
        if on && self.prefs.tf_delay_track && !self.finding_delay && self.last_find.is_none_or(|t| t.elapsed().as_secs_f32() >= 1.0) {
            self.find_delay();
        }
        let want = on.then(|| self.transfer_config());
        if want != self.tf_sent {
            let delay_only = matches!((&want, &self.tf_sent), (Some(a), Some(b)) if TransferConfig { delay: b.delay, ..*a } == *b);
            if delay_only {
                self.worker.set_transfer_delay(want.as_ref().map_or(0, |c| c.delay));
            } else {
                self.worker.set_transfer(want);
            }
            self.tf_sent = want;
        }
    }

    pub fn transfer_frame(&self) -> TransferFrame {
        self.worker.transfer.lock().map(|f| f.clone()).unwrap_or_default()
    }

    pub fn set_tf_delay_ms(&mut self, ms: f64) {
        self.prefs.tf_delay_ms = ms.clamp(0.0, 1000.0);
    }

    pub fn reset_transfer(&mut self) {
        self.worker.reset_transfer();
    }

    pub fn find_delay(&mut self) {
        self.finding_delay = true;
        self.last_find = Some(std::time::Instant::now());
        self.worker.find_delay();
    }

    /// Fige la fonction de transfert en cours.
    pub fn capture_transfer(&mut self, snap: TransferSnapshot) {
        let t = Trace::new(self.next_name(TraceKind::Transfer), self.next_color(), TraceData::Transfer(Arc::new(snap)));
        self.add_trace(t);
    }

    /// Réécrit plus tard le fichier de la mesure `i` (réglage en cours de changement).
    pub fn touch_trace(&mut self, i: usize) {
        self.dirty.retain(|(j, _)| *j != i);
        self.dirty.push((i, std::time::Instant::now()));
    }

    /// Dernier spectre du micro pour le RTA.
    pub fn rta_frame(&self) -> RtaFrame {
        self.worker.rta.lock().map(|f| f.clone()).unwrap_or_default()
    }

    /// Le signal (générateur ou sweep) sonne.
    pub fn playing(&self) -> bool {
        self.engine.out.playing.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_level(&mut self, db: f32) {
        self.prefs.level_db = db.clamp(-80.0, 0.0);
        self.engine.out.set_gain(db_to_gain(self.prefs.level_db));
    }

    /// Lance le générateur choisi.
    pub fn play_signal(&mut self) {
        if self.measuring.is_some() {
            return;
        }
        let fs = self.sample_rate();
        let seed = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(1);
        let src = match self.prefs.signal {
            SignalKind::Pink => Source::Pink(PinkNoise::new(1.0, seed)),
            SignalKind::White => Source::White(WhiteNoise::new(1.0, seed)),
            SignalKind::Sine => Source::Sine(Sine::new(self.prefs.sine_hz, fs, 1.0)),
            SignalKind::PeriodicPink => Source::PeriodicPink(PeriodicPink::new(65_536, fs, 20.0, 20_000.0, 1.0, seed, &mut star_measure::fft::Fft::new())),
        };
        self.engine.out.set_gain(db_to_gain(self.prefs.level_db));
        self.engine.send(OutCmd::Play(Box::new(src)));
    }

    /// Nouvelle fréquence du sinus : le sinus en cours y glisse, sans redémarrer.
    pub fn set_sine_hz(&mut self, hz: f64) {
        self.prefs.sine_hz = hz;
        if self.prefs.signal == SignalKind::Sine {
            self.engine.send(OutCmd::SineHz(hz));
        }
    }

    /// Échap : arrête signal et mesure.
    pub fn stop_all(&mut self) {
        self.engine.send(OutCmd::Stop);
        self.loading = None;
        if self.measuring.is_some() {
            self.worker.cancel();
        }
    }

    /// Lance une mesure par sweep.
    pub fn measure(&mut self) {
        if self.measuring.is_some() {
            return;
        }
        let Some(dev) = &self.device else {
            self.status = Some(tr("aucun périphérique audio ouvert", "no audio device open").into());
            return;
        };
        if dev.input.is_none() {
            self.status = Some(tr("aucune entrée audio : la mesure est impossible", "no audio input: measuring is impossible").into());
            return;
        }
        let fs = dev.sample_rate as f64;
        let p = &self.prefs;
        let sweep = Sweep::new(SweepParams {
            f1: p.f1,
            f2: p.f2.min(fs / 2.0 * 0.99),
            duration: p.sweep_secs,
            sample_rate: fs,
            fade_in: 0.05,
            fade_out: 0.005,
            amplitude: db_to_gain(p.level_db) as f64,
        });
        self.measuring = Some(0.0);
        self.status = Some(tr("Mesure en cours… (Échap pour arrêter)", "Measuring… (Esc to stop)").into());
        self.worker.measure(MeasureJob { sweep: Arc::new(sweep), use_reference: p.audio.ref_channel.is_some(), tail_s: p.tail_secs });
    }

    // ------------------------------------------------------------------ mesures enregistrées

    pub fn measures_dir(&self) -> Option<PathBuf> {
        self.prefs.measures_dir.clone().or_else(traces::default_dir)
    }

    /// Relit la liste des mesures du dossier ; celles de la session redeviennent visibles.
    pub fn load_library(&mut self) {
        let Some(dir) = self.measures_dir() else { return };
        let (mut list, errors) = traces::scan(&dir);
        for e in &errors {
            star_core::log::warn(&format!("mesure illisible : {e}"));
        }
        for t in &mut list {
            if t.path.as_ref().is_some_and(|p| self.session.visible_traces.contains(p)) {
                t.visible = t.load().is_ok();
            }
        }
        // Les mesures pas encore écrites (écriture impossible) restent dans la liste.
        list.extend(self.traces.drain(..).filter(|t| t.path.is_none()));
        self.traces = list;
        if !errors.is_empty() {
            self.status = Some(format!("{} : {}", tr("mesures illisibles", "unreadable measurements"), errors.len()));
        }
    }

    /// Écrit tout de suite les fichiers de mesure en attente.
    pub fn flush_traces(&mut self) {
        let pending: Vec<usize> = self.dirty.drain(..).map(|(j, _)| j).collect();
        for j in pending {
            self.save_trace(j);
        }
    }

    pub fn set_measures_dir(&mut self, dir: PathBuf) {
        self.flush_traces();
        self.prefs.measures_dir = Some(dir);
        self.save_prefs();
        self.traces.retain(|t| t.path.is_none());
        self.load_library();
    }

    fn remember_visible(&mut self) {
        self.session.visible_traces = self.traces.iter().filter(|t| t.visible).filter_map(|t| t.path.clone()).collect();
        self.save_session();
    }

    /// Ajoute une mesure à la liste et l'écrit tout de suite dans le dossier.
    fn add_trace(&mut self, mut t: Trace) {
        match self.measures_dir() {
            Some(dir) => {
                let p = dir.join(traces::file_name(&t.name, t.created));
                match t.write_to(&p) {
                    Ok(()) => t.path = Some(p),
                    Err(e) => self.status = Some(format!("{} : {e}", tr("mesure non enregistrée", "measurement not saved"))),
                }
            }
            None => self.status = Some(tr("pas de dossier pour les mesures", "no folder for measurements").into()),
        }
        self.traces.push(t);
        self.remember_visible();
    }

    fn next_name(&self, kind: TraceKind) -> String {
        let n = self.traces.iter().filter(|t| t.kind == kind).count() + 1;
        match kind {
            TraceKind::Sweep => format!("{} {n}", tr("Mesure", "Measurement")),
            TraceKind::Rta => format!("RTA {n}"),
            TraceKind::Transfer => format!("{} {n}", tr("Transfert", "Transfer")),
        }
    }

    fn next_color(&self) -> [u8; 3] {
        TRACE_COLORS[self.traces.len() % (TRACE_COLORS.len() - 1)]
    }

    /// Fige la dernière mesure : elle rejoint la liste et le dossier.
    pub fn freeze(&mut self) {
        let Some(m) = self.current.take() else { return };
        let t = Trace::new(self.next_name(TraceKind::Sweep), self.next_color(), TraceData::Sweep(m));
        self.add_trace(t);
    }

    /// Fige le spectre du RTA.
    pub fn capture_rta(&mut self, snap: RtaSnapshot) {
        let t = Trace::new(self.next_name(TraceKind::Rta), self.next_color(), TraceData::Rta(Arc::new(snap)));
        self.add_trace(t);
    }

    /// Affiche ou masque une mesure (chargée du disque à l'affichage).
    pub fn set_visible(&mut self, i: usize, on: bool) {
        let Some(t) = self.traces.get_mut(i) else { return };
        if on {
            if let Err(e) = t.load() {
                self.status = Some(format!("{} : {e}", tr("mesure illisible", "unreadable measurement")));
                return;
            }
        } else {
            t.unload();
        }
        t.visible = on;
        self.remember_visible();
    }

    /// Réécrit le fichier d'une mesure renommée ou recolorée.
    pub fn save_trace(&mut self, i: usize) {
        let Some(t) = self.traces.get_mut(i) else { return };
        let res = t.save();
        if !t.visible {
            t.unload();
        }
        if let Err(e) = res {
            self.status = Some(format!("{} : {e}", tr("mesure non enregistrée", "measurement not saved")));
        }
    }

    /// Nouvelle couleur pour la mesure `i` (fichier réécrit un peu plus tard).
    pub fn set_color(&mut self, i: usize, c: [u8; 3]) {
        let Some(t) = self.traces.get_mut(i) else { return };
        if t.color != c {
            t.color = c;
            self.touch_trace(i);
        }
    }

    /// Supprime une mesure et son fichier.
    pub fn delete_trace(&mut self, i: usize) {
        if i >= self.traces.len() {
            return;
        }
        self.flush_traces();
        let t = self.traces.remove(i);
        if let Some(p) = &t.path {
            if let Err(e) = std::fs::remove_file(p) {
                self.status = Some(format!("{} : {e}", tr("fichier non supprimé", "file not deleted")));
            }
        }
        self.remember_visible();
    }

    /// Copie des fichiers de mesure dans le dossier et les affiche.
    pub fn import_traces(&mut self, files: Vec<PathBuf>) {
        let Some(dir) = self.measures_dir() else { return };
        let mut errors = 0;
        for f in files {
            let res = traces::open(&f).and_then(|mut t| {
                let target = dir.join(f.file_name().unwrap_or_default());
                if f != target {
                    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
                    if target.exists() {
                        return Err(tr("une mesure de ce nom existe déjà dans le dossier", "a measurement with this name already exists in the folder").into());
                    }
                    std::fs::copy(&f, &target).map_err(|e| e.to_string())?;
                }
                t.path = Some(target);
                t.load()?;
                t.visible = true;
                Ok(t)
            });
            match res {
                Ok(t) => {
                    self.traces.retain(|o| o.path != t.path);
                    self.traces.push(t);
                }
                Err(e) => {
                    errors += 1;
                    star_core::log::warn(&format!("import de {} : {e}", f.display()));
                    self.status = Some(format!("{} : {e}", tr("import impossible", "import failed")));
                }
            }
        }
        if errors == 0 {
            self.status = None;
        }
        self.remember_visible();
    }
}
