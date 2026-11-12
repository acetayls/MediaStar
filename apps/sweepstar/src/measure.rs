//! Chaîne de mesure : capture des paires (référence, micro) pendant le sweep, puis
//! déconvolution et réponse en fréquence. Le fil de fond `mesure` vide le tampon d'entrée,
//! enregistre pendant une mesure et prépare le spectre du RTA le reste du temps.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, Sender};
use star_core::i18n::tr;
use star_measure::fft::{Fft, Window};
use star_measure::ir::{etc_db, windowed};
use star_measure::sweep::{deconvolve_with_reference, Sweep, DEFAULT_REGULARIZATION};
use star_measure::transfer::{Transfer, TransferConfig, TransferEngine};
use star_measure::Complex64;

use crate::audio::ring::Ring;
use crate::audio::signal::{OutCmd, Source};

/// Taille de la FFT de la réponse en fréquence (1,4 s à 48 kHz, 0,73 Hz par case).
pub const RESPONSE_FFT: usize = 65_536;
/// Avant le pic gardé dans la fenêtre de la réponse en fréquence.
const PRE_PEAK_S: f64 = 0.005;
/// Réponse impulsionnelle gardée pour l'affichage et l'export : 10 ms avant t = 0, 1 s après.
const IR_PRE_S: f64 = 0.010;
const IR_POST_S: f64 = 1.0;
/// Niveau minimal de la boucle de référence (dBFS crête).
const REF_MIN_DB: f32 = -50.0;
/// Taille de la FFT du RTA.
pub const RTA_FFT: usize = 8192;
/// Signal gardé pour la recherche du délai de la fonction de transfert (2,7 s à 48 kHz).
const DELAY_BUF: usize = 1 << 17;
/// Intervalle entre deux résultats de la fonction de transfert.
const TRANSFER_EVERY: Duration = Duration::from_millis(80);
/// Sans données d'entrée pendant ce délai, la mesure échoue.
const INPUT_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Division par la boucle de référence : délai et couleur de la carte annulés.
    Reference,
    /// Filtre inverse : le délai comprend la latence de la carte son.
    InverseFilter,
}

/// Résultat d'une mesure par sweep.
#[derive(Debug, Clone)]
pub struct Measurement {
    pub sample_rate: f64,
    pub method: Method,
    /// Délai du pic de l'IR (ms).
    pub delay_ms: f64,
    /// Réponse en fréquence, délai du pic retiré (case k à k·bin_hz).
    pub h: Vec<Complex64>,
    pub bin_hz: f64,
    /// Réponse impulsionnelle, `ir_pre` échantillons avant t = 0.
    pub ir: Vec<f32>,
    pub ir_pre: usize,
    /// ETC (dB, 0 au maximum), mêmes instants que `ir`.
    pub etc: Vec<f32>,
    /// Crêtes captées (dBFS).
    pub mic_peak_db: f32,
    pub ref_peak_db: Option<f32>,
}

impl Measurement {
    /// Instant (ms) de l'échantillon `i` de `ir`.
    pub fn ir_time_ms(&self, i: usize) -> f64 {
        (i as f64 - self.ir_pre as f64) * 1000.0 / self.sample_rate
    }
}

fn peak_db(x: &[f32]) -> f32 {
    let p = x.iter().fold(0.0f32, |m, v| m.max(v.abs()));
    if p > 0.0 {
        20.0 * p.log10()
    } else {
        -200.0
    }
}

/// Déconvolue une capture. `reference` : boucle captée (ou vide pour le filtre inverse).
pub fn analyze(reference: Option<&[f32]>, mic: &[f32], sweep: &Sweep, fft: &mut Fft) -> Result<Measurement, String> {
    let fs = sweep.params().sample_rate;
    let mic_peak_db = peak_db(mic);
    if mic_peak_db < -100.0 {
        return Err(tr("aucun signal sur l'entrée micro", "no signal on the microphone input").into());
    }
    let ref_peak_db = reference.map(peak_db);
    let (ir, method) = match reference {
        Some(r) => {
            if ref_peak_db.unwrap_or(-200.0) < REF_MIN_DB {
                return Err(tr(
                    "pas de signal sur la boucle de référence : vérifiez le câble de boucle et le canal choisi dans les Préférences",
                    "no signal on the reference loop: check the loopback cable and the channel chosen in Preferences",
                )
                .into());
            }
            let p = sweep.params();
            (deconvolve_with_reference(mic, r, fs, (p.f1, p.f2), DEFAULT_REGULARIZATION, fft), Method::Reference)
        }
        None => (sweep.deconvolve(mic, fft), Method::InverseFilter),
    };

    // Réponse en fréquence : fenêtre autour du pic, délai du pic retiré.
    let peak = ir.zero as f64 + ir.peak_samples();
    let peak_i = peak.round().max(0.0) as usize;
    let pre = ((PRE_PEAK_S * fs) as usize).min(peak_i);
    let n = RESPONSE_FFT;
    let seg = windowed(&ir.samples, peak_i, pre, n - pre);
    let start = peak_i - pre;
    let pos = peak - start as f64;
    let mut h = fft.forward(&seg, n);
    for (k, c) in h.iter_mut().enumerate() {
        *c *= Complex64::from_polar(1.0, 2.0 * std::f64::consts::PI * k as f64 * pos / n as f64);
    }

    // IR gardée pour l'affichage et l'export.
    let ir_pre = ((IR_PRE_S * fs) as usize).min(ir.zero);
    let len = ((IR_POST_S * fs) as usize + ir_pre).min(ir.samples.len() - (ir.zero - ir_pre));
    let ir_part: Vec<f64> = ir.samples[ir.zero - ir_pre..ir.zero - ir_pre + len].to_vec();
    let etc = etc_db(&ir_part, fft);

    Ok(Measurement {
        sample_rate: fs,
        method,
        delay_ms: ir.delay_ms(),
        h,
        bin_hz: fs / n as f64,
        ir: ir_part.iter().map(|&v| v as f32).collect(),
        ir_pre,
        etc: etc.iter().map(|&v| v as f32).collect(),
        mic_peak_db,
        ref_peak_db,
    })
}

/// Capture en cours : paires (référence, micro) jusqu'à `needed` images.
pub struct Capture {
    pub reference: Vec<f32>,
    pub mic: Vec<f32>,
    needed: usize,
}

impl Capture {
    pub fn new(needed: usize) -> Self {
        Self { reference: Vec::with_capacity(needed), mic: Vec::with_capacity(needed), needed }
    }

    /// Ajoute des paires entrelacées ; rend `true` quand la capture est complète.
    pub fn push_pairs(&mut self, pairs: &[f32]) -> bool {
        for &[r, m] in pairs.as_chunks::<2>().0 {
            if self.mic.len() >= self.needed {
                break;
            }
            self.reference.push(r);
            self.mic.push(m);
        }
        self.done()
    }

    pub fn done(&self) -> bool {
        self.mic.len() >= self.needed
    }

    pub fn progress(&self) -> f32 {
        self.mic.len() as f32 / self.needed.max(1) as f32
    }
}

pub struct MeasureJob {
    pub sweep: Arc<Sweep>,
    pub use_reference: bool,
    /// Silence enregistré après le sweep (s), pour la traîne de la salle.
    pub tail_s: f64,
}

enum Cmd {
    Measure(MeasureJob),
    Cancel,
    SampleRate(f64),
    /// Fonction de transfert en direct : réglages (le taux est celui du périphérique), ou arrêt.
    Transfer(Option<TransferConfig>),
    /// Retard de la référence (échantillons) ; remet la moyenne à zéro.
    TransferDelay(usize),
    TransferReset,
    /// Cherche le retard du micro sur la référence (GCC-PHAT).
    FindDelay,
}

pub enum WorkerEvent {
    Progress(f32),
    /// Retard trouvé (échantillons, fractionnaire), ou la raison de l'échec.
    DelayFound(Result<f64, String>),
    Done(Result<Measurement, String>),
}

/// Spectre du micro pour le RTA : puissance par case (somme = valeur quadratique moyenne).
#[derive(Debug, Clone, Default)]
pub struct RtaFrame {
    pub power: Vec<f64>,
    pub bin_hz: f64,
    /// Augmente à chaque nouveau spectre.
    pub serial: u64,
}

/// Dernier résultat de la fonction de transfert en direct.
#[derive(Clone, Default)]
pub struct TransferFrame {
    pub data: Option<Arc<Transfer>>,
    /// Blocs dans la moyenne.
    pub blocks: usize,
    pub sample_rate: f64,
    /// Augmente à chaque nouveau résultat.
    pub serial: u64,
}

pub struct Worker {
    tx: Sender<Cmd>,
    pub events: Receiver<WorkerEvent>,
    pub rta: Arc<Mutex<RtaFrame>>,
    pub transfer: Arc<Mutex<TransferFrame>>,
}

impl Worker {
    pub fn new(ring: Arc<Ring>, out_cmd: Sender<OutCmd>, wake: impl Fn() + Send + 'static) -> Self {
        let (tx, rx) = unbounded::<Cmd>();
        let (ev_tx, ev_rx) = unbounded::<WorkerEvent>();
        let rta = Arc::new(Mutex::new(RtaFrame::default()));
        let transfer = Arc::new(Mutex::new(TransferFrame::default()));
        let shared = Shared { rta: rta.clone(), transfer: transfer.clone() };
        thread::Builder::new()
            .name("mesure".into())
            .spawn(move || run(rx, ev_tx, ring, out_cmd, shared, wake))
            .expect("thread mesure");
        Self { tx, events: ev_rx, rta, transfer }
    }

    pub fn set_transfer(&self, cfg: Option<TransferConfig>) {
        let _ = self.tx.send(Cmd::Transfer(cfg));
    }

    pub fn set_transfer_delay(&self, samples: usize) {
        let _ = self.tx.send(Cmd::TransferDelay(samples));
    }

    pub fn reset_transfer(&self) {
        let _ = self.tx.send(Cmd::TransferReset);
    }

    pub fn find_delay(&self) {
        let _ = self.tx.send(Cmd::FindDelay);
    }

    pub fn measure(&self, job: MeasureJob) {
        let _ = self.tx.send(Cmd::Measure(job));
    }

    pub fn cancel(&self) {
        let _ = self.tx.send(Cmd::Cancel);
    }

    pub fn set_sample_rate(&self, fs: f64) {
        let _ = self.tx.send(Cmd::SampleRate(fs));
    }
}

struct Shared {
    rta: Arc<Mutex<RtaFrame>>,
    transfer: Arc<Mutex<TransferFrame>>,
}

/// Fonction de transfert en cours dans le fil de mesure.
struct LiveTransfer {
    engine: TransferEngine,
    /// Derniers échantillons des deux canaux, pour la recherche du délai.
    ref_hist: VecDeque<f32>,
    mic_hist: VecDeque<f32>,
    last: Instant,
    serial: u64,
}

fn run(rx: Receiver<Cmd>, events: Sender<WorkerEvent>, ring: Arc<Ring>, out_cmd: Sender<OutCmd>, shared: Shared, wake: impl Fn()) {
    let rta = shared.rta;
    let mut live: Option<LiveTransfer> = None;
    let mut ref_blk: Vec<f32> = Vec::new();
    let mut mic_blk: Vec<f32> = Vec::new();
    let mut fft = Fft::new();
    let mut pending: Vec<f32> = Vec::new();
    let mut job: Option<(MeasureJob, Capture)> = None;
    let mut last_data = Instant::now();
    let mut last_progress = Instant::now();
    let mut fs = 48_000.0;
    let mut rta_buf: Vec<f32> = Vec::new();
    let mut serial = 0u64;
    loop {
        match rx.recv_timeout(Duration::from_millis(5)) {
            Ok(Cmd::Measure(j)) => {
                ring.clear();
                let needed = j.sweep.len() + (j.tail_s * j.sweep.params().sample_rate) as usize;
                let data: Arc<[f32]> = j.sweep.signal().into();
                let _ = out_cmd.try_send(OutCmd::Play(Box::new(Source::Sweep { data, pos: 0 })));
                job = Some((j, Capture::new(needed)));
                last_data = Instant::now();
            }
            Ok(Cmd::Cancel) => {
                if job.take().is_some() {
                    let _ = events.send(WorkerEvent::Done(Err(tr("mesure arrêtée", "measurement stopped").into())));
                    wake();
                }
            }
            Ok(Cmd::SampleRate(r)) => {
                fs = r;
                // Nouveau taux : la fonction de transfert repart avec ses réglages.
                if let Some(l) = &mut live {
                    let cfg = TransferConfig { sample_rate: fs, ..*l.engine.config() };
                    l.engine = TransferEngine::new(cfg, &mut fft);
                }
            }
            Ok(Cmd::Transfer(cfg)) => match cfg {
                Some(c) => {
                    let c = TransferConfig { sample_rate: fs, ..c };
                    let keep = live.as_ref().is_some_and(|l| *l.engine.config() == c);
                    if !keep {
                        live = Some(LiveTransfer {
                            engine: TransferEngine::new(c, &mut fft),
                            ref_hist: VecDeque::with_capacity(DELAY_BUF + 8192),
                            mic_hist: VecDeque::with_capacity(DELAY_BUF + 8192),
                            last: Instant::now(),
                            serial: live.as_ref().map_or(0, |l| l.serial),
                        });
                    }
                }
                None => {
                    live = None;
                    if let Ok(mut f) = shared.transfer.lock() {
                        f.data = None;
                        f.blocks = 0;
                    }
                }
            },
            Ok(Cmd::TransferDelay(d)) => {
                if let Some(l) = &mut live {
                    l.engine.set_delay(d);
                }
            }
            Ok(Cmd::TransferReset) => {
                if let Some(l) = &mut live {
                    l.engine.reset();
                }
            }
            Ok(Cmd::FindDelay) => {
                let res = match &live {
                    Some(l) if l.ref_hist.len() >= DELAY_BUF / 2 => {
                        let (r, m): (Vec<f32>, Vec<f32>) = (l.ref_hist.iter().copied().collect(), l.mic_hist.iter().copied().collect());
                        if r.iter().all(|v| v.abs() < 1e-6) {
                            Err(tr("pas de signal sur la boucle de référence", "no signal on the reference loop").into())
                        } else {
                            star_measure::delay::gcc_phat(&r, &m, Some((fs * 0.9) as usize), &mut fft)
                                .ok_or_else(|| tr("délai introuvable : jouez un bruit rose", "delay not found: play pink noise").into())
                        }
                    }
                    Some(_) => Err(tr("pas encore assez de signal, réessayez dans une seconde", "not enough signal yet, try again in a second").into()),
                    None => Err(tr("la fonction de transfert n'est pas lancée", "the transfer function is not running").into()),
                };
                let _ = events.send(WorkerEvent::DelayFound(res));
                wake();
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {}
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => return,
        }
        pending.clear();
        if ring.pop_into(&mut pending) > 0 {
            last_data = Instant::now();
        }
        if let Some((j, cap)) = &mut job {
            if cap.push_pairs(&pending) {
                let (j, cap) = job.take().expect("mesure en cours");
                let reference = j.use_reference.then_some(cap.reference.as_slice());
                let res = analyze(reference, &cap.mic, &j.sweep, &mut fft);
                let _ = events.send(WorkerEvent::Done(res));
                wake();
            } else if last_data.elapsed() > INPUT_TIMEOUT {
                job = None;
                let _ = out_cmd.try_send(OutCmd::Stop);
                let _ = events.send(WorkerEvent::Done(Err(tr("aucune donnée de l'entrée audio : vérifiez l'entrée dans les Préférences", "no data from the audio input: check the input in Preferences").into())));
                wake();
            } else if last_progress.elapsed() > Duration::from_millis(50) {
                last_progress = Instant::now();
                let _ = events.send(WorkerEvent::Progress(cap.progress()));
                let _ = j;
                wake();
            }
            continue;
        }
        // Fonction de transfert : (référence, micro), résultat toutes les 80 ms.
        if let Some(l) = &mut live {
            ref_blk.clear();
            mic_blk.clear();
            for p in pending.as_chunks::<2>().0 {
                ref_blk.push(p[0]);
                mic_blk.push(p[1]);
            }
            if !ref_blk.is_empty() {
                l.engine.push(&ref_blk, &mic_blk);
                l.ref_hist.extend(&ref_blk);
                l.mic_hist.extend(&mic_blk);
                let extra = l.ref_hist.len().saturating_sub(DELAY_BUF);
                l.ref_hist.drain(..extra);
                l.mic_hist.drain(..extra);
            }
            if l.last.elapsed() >= TRANSFER_EVERY {
                l.last = Instant::now();
                if let Some(t) = l.engine.result() {
                    l.serial += 1;
                    if let Ok(mut f) = shared.transfer.lock() {
                        *f = TransferFrame { data: Some(Arc::new(t)), blocks: l.engine.blocks(), sample_rate: fs, serial: l.serial };
                    }
                    wake();
                }
            }
        }
        // RTA : micro seul, un spectre par demi-bloc.
        rta_buf.extend(pending.as_chunks::<2>().0.iter().map(|p| p[1]));
        if rta_buf.len() >= RTA_FFT {
            let start = rta_buf.len() - RTA_FFT;
            let power = star_measure::rta::welch_power(&rta_buf[start..], RTA_FFT, 0.0, Window::Hann, &mut fft);
            rta_buf.drain(..rta_buf.len() - RTA_FFT / 2);
            serial += 1;
            if let Ok(mut f) = rta.lock() {
                *f = RtaFrame { power, bin_hz: fs / RTA_FFT as f64, serial };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::signal::{OutShared, OutputState};
    use crossbeam_channel::bounded;
    use star_measure::sweep::SweepParams;

    /// Fonction de transfert en direct : la recherche du délai trouve le retard du micro, puis,
    /// une fois la référence alignée, H vaut −6 dB avec une cohérence proche de 1.
    #[test]
    fn transfert_en_direct() {
        let ring = Arc::new(Ring::new(1 << 18));
        let (otx, _orx) = bounded(8);
        let w = Worker::new(ring.clone(), otx, || {});
        w.set_sample_rate(48_000.0);
        w.set_transfer(Some(TransferConfig { fft_len: 4096, ..Default::default() }));
        let mut g = star_measure::gen::PinkNoise::new(0.5, 7);
        let lag = 240usize;
        let mut hist = vec![0.0f32; lag];
        let feed = |hist: &mut Vec<f32>, g: &mut star_measure::gen::PinkNoise, secs: f64| {
            let n = (secs * 48_000.0) as usize;
            for _ in 0..n / 1024 {
                let mut x = vec![0.0f32; 1024];
                star_measure::gen::Generator::fill(g, &mut x);
                hist.extend_from_slice(&x);
                let base = hist.len() - 1024;
                let pairs: Vec<f32> = (0..1024).flat_map(|i| [hist[base + i], 0.5 * hist[base + i - lag]]).collect();
                while !ring.push(&pairs) {
                    std::thread::sleep(Duration::from_millis(1));
                }
            }
        };
        feed(&mut hist, &mut g, 2.0);
        std::thread::sleep(Duration::from_millis(200));
        w.find_delay();
        let found = loop {
            match w.events.recv_timeout(Duration::from_secs(5)).expect("délai") {
                WorkerEvent::DelayFound(r) => break r.unwrap(),
                _ => continue,
            }
        };
        assert!((found - lag as f64).abs() < 0.5, "{found}");
        w.set_transfer_delay(lag);
        feed(&mut hist, &mut g, 1.0);
        std::thread::sleep(Duration::from_millis(300));
        let f = w.transfer.lock().unwrap().clone();
        let t = f.data.expect("résultat");
        let k = (1000.0 / t.bin_hz) as usize;
        assert!((star_measure::db20(t.h[k].norm()) + 6.02).abs() < 0.2, "{}", star_measure::db20(t.h[k].norm()));
        assert!(t.coherence[k] > 0.95, "{}", t.coherence[k]);
    }

    /// Chaîne complète sur un périphérique simulé : le callback de sortie joue le sweep, la
    /// « carte » renvoie la boucle avec 17 échantillons de latence et le micro à −6 dB avec
    /// 17 + 480 échantillons (10 ms de temps de vol) ; les paires passent par le tampon
    /// circulaire comme dans le callback d'entrée.
    #[test]
    fn chaine_simulee() {
        let fs = 48_000.0;
        let sweep = Arc::new(Sweep::new(SweepParams { duration: 1.0, amplitude: 0.1, ..Default::default() }));
        let (tx, rx) = bounded(8);
        let (gtx, _grx) = bounded(8);
        let shared = Arc::new(OutShared::new(0.1, 1));
        let mut out = OutputState::new(48_000, rx, gtx, shared.clone());
        tx.send(OutCmd::Play(Box::new(Source::Sweep { data: sweep.signal().into(), pos: 0 }))).unwrap();

        let ring = Ring::new(1 << 16);
        let (lat, flight) = (17usize, 480usize);
        let mut played: Vec<f32> = Vec::new();
        let mut cap = Capture::new(sweep.len() + 24_000);
        let mut pending = Vec::new();
        let block = 256;
        let mut buf = vec![0.0f32; block];
        while !cap.done() {
            out.process(&mut buf, 1);
            played.extend_from_slice(&buf);
            let n0 = played.len() - block;
            let mut pairs = Vec::with_capacity(2 * block);
            for i in n0..played.len() {
                let at = |d: usize| if i >= d { played[i - d] } else { 0.0 };
                pairs.push(at(lat));
                pairs.push(0.5 * at(lat + flight));
            }
            assert!(ring.push(&pairs));
            pending.clear();
            ring.pop_into(&mut pending);
            cap.push_pairs(&pending);
        }
        assert_eq!(shared.sweeps_done.load(std::sync::atomic::Ordering::Relaxed), 1);

        let mut fft = Fft::new();
        let m = analyze(Some(&cap.reference), &cap.mic, &sweep, &mut fft).unwrap();
        println!("boucle : délai {:.4} ms (attendu 10 ms), micro {:.1} dBFS", m.delay_ms, m.mic_peak_db);
        assert_eq!(m.method, Method::Reference);
        assert!((m.delay_ms - 10.0).abs() < 0.01);
        let k = (1000.0 / m.bin_hz) as usize;
        let g = star_measure::db20(m.h[k].norm());
        assert!((g + 6.02).abs() < 0.1, "gain à 1 kHz {g}");
        // Délai retiré : phase nulle.
        assert!(m.h[k].arg().abs() < 0.05);

        let m = analyze(None, &cap.mic, &sweep, &mut fft).unwrap();
        let attendu = (lat + flight) as f64 * 1000.0 / fs;
        println!("filtre inverse : délai {:.4} ms (attendu {attendu:.4} ms)", m.delay_ms);
        assert_eq!(m.method, Method::InverseFilter);
        assert!((m.delay_ms - attendu).abs() < 0.01);
        assert!((star_measure::db20(m.h[k].norm()) + 6.02).abs() < 0.1);
        assert_eq!(m.ir.len(), m.etc.len());

        // Boucle muette : erreur claire.
        let silence = vec![0.0f32; cap.mic.len()];
        assert!(analyze(Some(&silence), &cap.mic, &sweep, &mut fft).is_err());
    }
}
