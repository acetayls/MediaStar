//! Signal joué par le callback de sortie : générateurs de star-measure, sweep, fichiers de
//! la playlist ; niveau en rampe, arrêt. Aucune allocation ni libération dans `process` :
//! les sources remplacées repartent par la file `garbage` pour être libérées ailleurs.
//!
//! Sorties : un signal mono va sur toutes les sorties choisies ; un fichier stéréo envoie la
//! gauche sur la 1re, 3e… sortie choisie et la droite sur la 2e, 4e… (mono si une seule).

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;

use crossbeam_channel::{Receiver, Sender};
use star_measure::gen::{Generator, PeriodicPink, PinkNoise, Sine, WhiteNoise};

/// Taille du tampon de travail (les callbacks plus longs sont découpés).
pub const MAX_BLOCK: usize = 4096;
/// Durée des rampes de niveau (s).
const RAMP_SECS: f32 = 0.02;
/// Constante de temps du glissement de fréquence du sinus (s).
const GLIDE_SECS: f64 = 0.03;

/// Fichier décodé (stéréo planaire, au taux du périphérique).
pub struct Track {
    pub left: Vec<f32>,
    pub right: Vec<f32>,
}

/// Source sonore.
pub enum Source {
    Pink(PinkNoise),
    White(WhiteNoise),
    Sine(Sine),
    PeriodicPink(PeriodicPink),
    /// Joué une fois, à son propre niveau (le sweep a ses fondus), puis silence.
    Sweep { data: Arc<[f32]>, pos: usize },
    /// Morceau de la playlist, au niveau de la playlist ; en boucle ou une fois.
    File { track: Arc<Track>, pos: usize, looping: bool },
}

impl Source {
    fn is_sweep(&self) -> bool {
        matches!(self, Source::Sweep { .. })
    }

    fn is_file(&self) -> bool {
        matches!(self, Source::File { .. })
    }

    /// Remplit `l` (et `r` pour un fichier stéréo, sinon copie de `l`) ; rend `false` quand
    /// un sweep ou un fichier vient de finir.
    fn fill(&mut self, l: &mut [f32], r: &mut [f32]) -> bool {
        let mut alive = true;
        match self {
            Source::Pink(g) => g.fill(l),
            Source::White(g) => g.fill(l),
            Source::Sine(g) => g.fill(l),
            Source::PeriodicPink(g) => g.fill(l),
            Source::Sweep { data, pos } => {
                let n = l.len().min(data.len() - *pos);
                l[..n].copy_from_slice(&data[*pos..*pos + n]);
                l[n..].fill(0.0);
                *pos += n;
                alive = *pos < data.len();
            }
            Source::File { track, pos, looping } => {
                let len = track.left.len();
                let mut i = 0;
                while i < l.len() {
                    if *pos >= len {
                        if *looping && len > 0 {
                            *pos = 0;
                        } else {
                            l[i..].fill(0.0);
                            r[i..].fill(0.0);
                            return false;
                        }
                    }
                    let n = (l.len() - i).min(len - *pos);
                    l[i..i + n].copy_from_slice(&track.left[*pos..*pos + n]);
                    r[i..i + n].copy_from_slice(&track.right[*pos..*pos + n]);
                    *pos += n;
                    i += n;
                }
                return true;
            }
        }
        r.copy_from_slice(l);
        alive
    }
}

pub enum OutCmd {
    /// Nouvelle source, en rampe depuis le silence (sauf un sweep, joué tel quel). Si un
    /// signal sonne déjà, il descend d'abord au silence : pas de clic au changement.
    Play(Box<Source>),
    /// Nouvelle fréquence du sinus en cours, en glissant (phase continue).
    SineHz(f64),
    /// Saut dans le fichier en cours (image), avec un fondu de part et d'autre.
    Seek(usize),
    /// Rampe jusqu'au silence.
    Stop,
}

/// État partagé avec l'interface (lu et écrit sans verrou).
pub struct OutShared {
    /// Gain des générateurs (linéaire, bits d'un f32).
    pub gain: AtomicU32,
    /// Gain de la playlist (linéaire, bits d'un f32).
    pub file_gain: AtomicU32,
    /// Sorties qui reçoivent le signal (bit n = canal n).
    pub channel_mask: AtomicU64,
    /// Crête de la sortie depuis la dernière lecture (bits d'un f32).
    pub peak: AtomicU32,
    /// Un signal est en cours (rampe comprise).
    pub playing: AtomicBool,
    /// Un fichier de la playlist est en cours.
    pub playing_file: AtomicBool,
    /// Incrémenté à chaque fin de sweep.
    pub sweeps_done: AtomicU64,
    /// Incrémenté à chaque fin de fichier (hors boucle et hors arrêt).
    pub files_done: AtomicU64,
    /// Position dans le fichier en cours (images).
    pub file_pos: AtomicU64,
    /// Le fichier en cours repart au début à la fin.
    pub loop_file: AtomicBool,
}

impl OutShared {
    pub fn new(gain: f32, channel_mask: u64) -> Self {
        Self {
            gain: AtomicU32::new(gain.to_bits()),
            file_gain: AtomicU32::new(gain.to_bits()),
            channel_mask: AtomicU64::new(channel_mask),
            peak: AtomicU32::new(0),
            playing: AtomicBool::new(false),
            playing_file: AtomicBool::new(false),
            sweeps_done: AtomicU64::new(0),
            files_done: AtomicU64::new(0),
            file_pos: AtomicU64::new(0),
            loop_file: AtomicBool::new(false),
        }
    }

    pub fn set_gain(&self, g: f32) {
        self.gain.store(g.to_bits(), Ordering::Relaxed);
    }

    pub fn set_file_gain(&self, g: f32) {
        self.file_gain.store(g.to_bits(), Ordering::Relaxed);
    }

    pub fn take_peak(&self) -> f32 {
        f32::from_bits(self.peak.swap(0, Ordering::Relaxed))
    }
}

/// Ce qui attend que l'enveloppe soit descendue au silence.
enum Pending {
    Source(Box<Source>),
    Seek(usize),
}

pub struct OutputState {
    /// `None` : silence.
    source: Option<Box<Source>>,
    /// Changement en attente de la fin du fondu de sortie.
    pending: Option<Pending>,
    /// Enveloppe de la rampe (0 à 1).
    gain: f32,
    /// Arrêt demandé : l'enveloppe descend à 0, puis la source est retirée.
    stopping: bool,
    step: f32,
    left: Vec<f32>,
    right: Vec<f32>,
    commands: Receiver<OutCmd>,
    garbage: Sender<Box<Source>>,
    shared: Arc<OutShared>,
}

impl OutputState {
    pub fn new(sample_rate: u32, commands: Receiver<OutCmd>, garbage: Sender<Box<Source>>, shared: Arc<OutShared>) -> Self {
        Self {
            source: None,
            pending: None,
            gain: 0.0,
            stopping: false,
            step: 1.0 / (RAMP_SECS * sample_rate.max(1) as f32),
            left: vec![0.0; MAX_BLOCK],
            right: vec![0.0; MAX_BLOCK],
            commands,
            garbage,
            shared,
        }
    }

    fn replace(&mut self, new: Option<Box<Source>>) {
        if let Some(old) = std::mem::replace(&mut self.source, new) {
            // File pleine : rare, on libère sur place plutôt que de bloquer.
            let _ = self.garbage.try_send(old);
        }
    }

    fn set_pending(&mut self, p: Option<Pending>) {
        if let Some(Pending::Source(old)) = std::mem::replace(&mut self.pending, p) {
            let _ = self.garbage.try_send(old);
        }
    }

    /// Un signal audible sonne (hors sweep) : un changement doit attendre son fondu.
    fn audible(&self) -> bool {
        self.gain > 0.0 && self.source.as_ref().is_some_and(|s| !s.is_sweep())
    }

    /// Remplit `data` (entrelacé, `channels` canaux).
    pub fn process(&mut self, data: &mut [f32], channels: usize) {
        while let Ok(cmd) = self.commands.try_recv() {
            match cmd {
                OutCmd::Play(s) => {
                    if !s.is_sweep() && self.audible() {
                        // Fondu de sortie, puis la nouvelle source monte depuis le silence.
                        self.set_pending(Some(Pending::Source(s)));
                        self.stopping = true;
                        continue;
                    }
                    self.set_pending(None);
                    self.stopping = false;
                    let was_sweep = self.source.as_ref().is_some_and(|s| s.is_sweep());
                    if s.is_sweep() {
                        self.gain = 1.0;
                    } else if was_sweep || self.source.is_none() {
                        self.gain = 0.0;
                    }
                    self.replace(Some(s));
                }
                OutCmd::SineHz(hz) => {
                    if let Some(Source::Sine(g)) = self.source.as_deref_mut() {
                        g.glide_to(hz, GLIDE_SECS);
                    }
                    if let Some(Pending::Source(p)) = &mut self.pending {
                        if let Source::Sine(g) = &mut **p {
                            g.set_frequency(hz);
                        }
                    }
                }
                OutCmd::Seek(pos) => {
                    let is_file = self.source.as_ref().is_some_and(|s| s.is_file());
                    if !is_file || matches!(self.pending, Some(Pending::Source(_))) {
                        continue;
                    }
                    if self.audible() {
                        self.pending = Some(Pending::Seek(pos));
                        self.stopping = true;
                    } else if let Some(Source::File { pos: p, track, .. }) = self.source.as_deref_mut() {
                        *p = pos.min(track.left.len());
                    }
                }
                OutCmd::Stop => {
                    self.set_pending(None);
                    self.stopping = true;
                }
            }
        }
        let channels = channels.max(1);
        let mask = self.shared.channel_mask.load(Ordering::Relaxed);
        let chosen = mask.count_ones();
        let mut peak = 0.0f32;
        for frames in data.chunks_mut(MAX_BLOCK * channels) {
            let n = frames.len() / channels;
            // La source a fini pendant le fondu : la suivante démarre tout de suite.
            if self.source.is_none() {
                match self.pending.take() {
                    Some(Pending::Source(s)) => {
                        self.source = Some(s);
                        self.stopping = false;
                        self.gain = 0.0;
                    }
                    Some(Pending::Seek(_)) | None => {}
                }
            }
            let Some(src) = self.source.as_mut() else {
                frames.fill(0.0);
                continue;
            };
            if let Source::File { looping, .. } = &mut **src {
                *looping = self.shared.loop_file.load(Ordering::Relaxed);
            }
            let (l, r) = (&mut self.left[..n], &mut self.right[..n]);
            let alive = src.fill(l, r);
            let (sweep, file) = (src.is_sweep(), src.is_file());
            if let Source::File { pos, .. } = &**src {
                self.shared.file_pos.store(*pos as u64, Ordering::Relaxed);
            }
            // Enveloppe de 0 à 1 en RAMP_SECS, quel que soit le niveau ; le sweep a ses fondus.
            let level = if sweep {
                1.0
            } else if file {
                f32::from_bits(self.shared.file_gain.load(Ordering::Relaxed))
            } else {
                f32::from_bits(self.shared.gain.load(Ordering::Relaxed))
            };
            for (f, (&a, &b)) in frames.chunks_mut(channels).zip(l.iter().zip(r.iter())) {
                let target = if self.stopping { 0.0 } else { 1.0 };
                if self.gain < target {
                    self.gain = (self.gain + self.step).min(target);
                } else if self.gain > target {
                    self.gain = (self.gain - self.step).max(target);
                }
                let k = self.gain * level;
                let (a, b) = (a * k, b * k);
                let mono = 0.5 * (a + b);
                peak = peak.max(a.abs()).max(b.abs());
                // Rang de la sortie parmi les sorties choisies : gauche, droite, gauche…
                let mut rank = 0;
                for (c, o) in f.iter_mut().enumerate() {
                    *o = if c < 64 && mask & (1 << c) != 0 {
                        rank += 1;
                        if chosen == 1 {
                            mono
                        } else if rank % 2 == 1 {
                            a
                        } else {
                            b
                        }
                    } else {
                        0.0
                    };
                }
            }
            if !alive {
                if sweep {
                    self.shared.sweeps_done.fetch_add(1, Ordering::Relaxed);
                } else if file && !self.stopping {
                    self.shared.files_done.fetch_add(1, Ordering::Relaxed);
                }
                self.replace(None);
                self.gain = 0.0;
            } else if self.stopping && self.gain == 0.0 {
                match self.pending.take() {
                    Some(Pending::Source(s)) => {
                        self.replace(Some(s));
                        self.stopping = false;
                    }
                    Some(Pending::Seek(p)) => {
                        if let Some(Source::File { pos, track, .. }) = self.source.as_deref_mut() {
                            *pos = p.min(track.left.len());
                        }
                        self.stopping = false;
                    }
                    None => self.replace(None),
                }
            }
        }
        let file = self.source.as_ref().is_some_and(|s| s.is_file());
        self.shared.playing.store(self.source.is_some(), Ordering::Relaxed);
        self.shared.playing_file.store(file, Ordering::Relaxed);
        let prev = f32::from_bits(self.shared.peak.load(Ordering::Relaxed));
        if peak > prev {
            self.shared.peak.store(peak.to_bits(), Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossbeam_channel::bounded;

    fn state(gain: f32) -> (OutputState, Sender<OutCmd>, Arc<OutShared>) {
        let (tx, rx) = bounded(8);
        let (gtx, _grx) = bounded(8);
        let shared = Arc::new(OutShared::new(gain, 0b01));
        (OutputState::new(48_000, rx, gtx, shared.clone()), tx, shared)
    }

    #[test]
    fn rampe_et_arret() {
        let (mut s, tx, shared) = state(0.1);
        tx.send(OutCmd::Play(Box::new(Source::Sine(Sine::new(1000.0, 48_000.0, 1.0))))).unwrap();
        let mut buf = vec![0.0f32; 2 * 4800];
        s.process(&mut buf, 2);
        // Deuxième canal hors du masque : muet.
        assert!(buf.iter().skip(1).step_by(2).all(|&v| v == 0.0));
        // Montée progressive : les premières millisecondes sont plus faibles.
        let early = buf[..96].iter().step_by(2).fold(0.0f32, |m, v| m.max(v.abs()));
        let late = buf[4000..].iter().step_by(2).fold(0.0f32, |m, v| m.max(v.abs()));
        assert!(early < 0.01 && (late - 0.1).abs() < 0.002, "{early} {late}");
        assert!(shared.playing.load(Ordering::Relaxed));
        tx.send(OutCmd::Stop).unwrap();
        s.process(&mut buf, 2);
        assert!(buf[buf.len() - 200..].iter().all(|&v| v == 0.0));
        assert!(!shared.playing.load(Ordering::Relaxed));
    }

    /// Fichier stéréo : gauche sur la 1re sortie choisie, droite sur la 2e ; fin signalée.
    #[test]
    fn fichier_stereo() {
        let (tx, rx) = bounded(8);
        let (gtx, _grx) = bounded(8);
        let shared = Arc::new(OutShared::new(1.0, 0b101));
        let mut s = OutputState::new(48_000, rx, gtx, shared.clone());
        let track = Arc::new(Track { left: vec![0.5; 4800], right: vec![-0.25; 4800] });
        tx.send(OutCmd::Play(Box::new(Source::File { track, pos: 0, looping: false }))).unwrap();
        let mut buf = vec![0.0f32; 3 * 6000];
        s.process(&mut buf, 3);
        assert_eq!(&buf[3 * 2000..3 * 2001], &[0.5, 0.0, -0.25]);
        assert!(buf[3 * 5000..].iter().all(|&v| v == 0.0));
        assert_eq!(shared.files_done.load(Ordering::Relaxed), 1);
        assert!(!shared.playing.load(Ordering::Relaxed));
    }

    /// Changement de signal ou saut dans un fichier : fondu, jamais de saut brutal.
    #[test]
    fn changement_sans_clic() {
        let (mut s, tx, _shared) = state(1.0);
        tx.send(OutCmd::Play(Box::new(Source::Sine(Sine::new(1000.0, 48_000.0, 1.0))))).unwrap();
        let mut buf = vec![0.0f32; 9600];
        s.process(&mut buf, 1);
        tx.send(OutCmd::Play(Box::new(Source::Sine(Sine::new(3000.0, 48_000.0, 1.0))))).unwrap();
        let mut b2 = vec![0.0f32; 9600];
        s.process(&mut b2, 1);
        tx.send(OutCmd::SineHz(5000.0)).unwrap();
        let mut b3 = vec![0.0f32; 9600];
        s.process(&mut b3, 1);
        let all: Vec<f32> = buf.iter().chain(&b2).chain(&b3).copied().collect();
        let pente = 2.0 * std::f32::consts::PI * 5000.0 / 48_000.0;
        assert!(all.windows(2).all(|w| (w[1] - w[0]).abs() <= pente * 1.05));
        // La nouvelle source a bien pris le relais.
        assert!(b3[9000..].iter().fold(0.0f32, |m, v| m.max(v.abs())) > 0.99);

        // Saut dans un fichier constant : fondu de sortie puis de retour, au même niveau.
        let ramp: Vec<f32> = (0..48_000).map(|i| i as f32 / 48_000.0).collect();
        let track = Arc::new(Track { left: ramp.clone(), right: ramp });
        tx.send(OutCmd::Play(Box::new(Source::File { track, pos: 0, looping: false }))).unwrap();
        let mut f = vec![0.0f32; 4800];
        s.process(&mut f, 1);
        s.process(&mut f, 1);
        tx.send(OutCmd::Seek(40_000)).unwrap();
        s.process(&mut f, 1);
        s.process(&mut f, 1);
        assert!(f[4700] > 0.8, "{}", f[4700]);
    }

    #[test]
    fn sweep_joue_une_fois() {
        let (mut s, tx, shared) = state(0.01);
        let data: Arc<[f32]> = (0..1000).map(|i| i as f32 / 1000.0).collect();
        tx.send(OutCmd::Play(Box::new(Source::Sweep { data: data.clone(), pos: 0 }))).unwrap();
        let mut buf = vec![0.0f32; 1500];
        s.process(&mut buf, 1);
        // Joué tel quel, sans rampe ni gain des générateurs.
        assert_eq!(&buf[..1000], &data[..]);
        assert!(buf[1000..].iter().all(|&v| v == 0.0));
        assert_eq!(shared.sweeps_done.load(Ordering::Relaxed), 1);
    }
}
