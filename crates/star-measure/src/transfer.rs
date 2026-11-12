//! Fonction de transfert bi-canal en direct (référence → mesure), par blocs FFT.
//!
//! Pour chaque bloc fenêtré : Pxx = |X|², Pyy = |Y|², Pxy = X*·Y. Les moyennes Gxx, Gyy, Gxy
//! donnent l'estimateur H1 = Gxy / Gxx (insensible au bruit sur la mesure) et la cohérence
//! γ² = |Gxy|² / (Gxx·Gyy) : 1 quand la mesure est entièrement expliquée par la référence,
//! moins quand du bruit, de la réverbération tardive ou une non-linéarité s'en mêlent.
//!
//! La référence est retardée par un tampon de `delay` échantillons pour l'aligner sur la
//! mesure (temps de vol jusqu'au micro), sinon la cohérence s'effondre dans l'aigu.
//! Tous les tampons sont alloués à la construction : `push` n'alloue pas (une fois les
//! files remplies jusqu'à leur taille de croisière).

use std::collections::VecDeque;
use std::sync::Arc;

use num_complex::Complex64;
use realfft::RealToComplex;

use crate::fft::{Fft, Window};

/// Moyennage des spectres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Averaging {
    /// Moyenne glissante des N derniers blocs.
    Linear(usize),
    /// Moyenne exponentielle : poids du nouveau bloc (0 à 1).
    Exponential(f64),
    /// Moyenne de tous les blocs depuis la dernière remise à zéro.
    Infinite,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransferConfig {
    pub sample_rate: f64,
    pub fft_len: usize,
    /// Recouvrement des blocs (0 à 0,95).
    pub overlap: f64,
    pub window: Window,
    pub averaging: Averaging,
    /// Retard appliqué à la référence (échantillons).
    pub delay: usize,
}

impl Default for TransferConfig {
    fn default() -> Self {
        Self { sample_rate: 48_000.0, fft_len: 16_384, overlap: 0.75, window: Window::Hann, averaging: Averaging::Linear(16), delay: 0 }
    }
}

/// Résultat moyenné.
#[derive(Debug, Clone)]
pub struct Transfer {
    pub bin_hz: f64,
    pub h: Vec<Complex64>,
    pub coherence: Vec<f64>,
    pub gxx: Vec<f64>,
    pub gyy: Vec<f64>,
}

impl Transfer {
    pub fn frequencies(&self) -> Vec<f64> {
        (0..self.h.len()).map(|k| k as f64 * self.bin_hz).collect()
    }

    pub fn magnitude_db(&self) -> Vec<f64> {
        self.h.iter().map(|c| crate::db20(c.norm())).collect()
    }

    /// Phase en degrés, dans ]−180, 180].
    pub fn phase_deg(&self) -> Vec<f64> {
        self.h.iter().map(|c| c.arg().to_degrees()).collect()
    }
}

pub struct TransferEngine {
    cfg: TransferConfig,
    plan: Arc<dyn RealToComplex<f64>>,
    window: Vec<f64>,
    hop: usize,
    ref_fifo: VecDeque<f64>,
    meas_fifo: VecDeque<f64>,
    input: Vec<f64>,
    spec_x: Vec<Complex64>,
    spec_y: Vec<Complex64>,
    gxx: Vec<f64>,
    gyy: Vec<f64>,
    gxy: Vec<Complex64>,
    /// Historique des blocs pour la moyenne glissante (N × cases, à plat).
    ring_xx: Vec<f64>,
    ring_yy: Vec<f64>,
    ring_xy: Vec<Complex64>,
    ring_pos: usize,
    blocks: usize,
}

impl TransferEngine {
    pub fn new(cfg: TransferConfig, fft: &mut Fft) -> Self {
        assert!(cfg.fft_len >= 16 && cfg.fft_len.is_multiple_of(2), "taille de FFT invalide");
        let plan = fft.forward_plan(cfg.fft_len);
        let bins = cfg.fft_len / 2 + 1;
        let hop = ((cfg.fft_len as f64 * (1.0 - cfg.overlap.clamp(0.0, 0.95))).round() as usize).max(1);
        let ring = if let Averaging::Linear(n) = cfg.averaging { n.max(1) * bins } else { 0 };
        let mut ref_fifo = VecDeque::with_capacity(cfg.fft_len + cfg.delay + 8192);
        ref_fifo.extend(std::iter::repeat_n(0.0, cfg.delay));
        Self {
            plan: plan.clone(),
            window: cfg.window.coefficients(cfg.fft_len),
            hop,
            ref_fifo,
            meas_fifo: VecDeque::with_capacity(cfg.fft_len + 8192),
            input: vec![0.0; cfg.fft_len],
            spec_x: plan.make_output_vec(),
            spec_y: plan.make_output_vec(),
            gxx: vec![0.0; bins],
            gyy: vec![0.0; bins],
            gxy: vec![Complex64::new(0.0, 0.0); bins],
            ring_xx: vec![0.0; ring],
            ring_yy: vec![0.0; ring],
            ring_xy: vec![Complex64::new(0.0, 0.0); ring],
            ring_pos: 0,
            blocks: 0,
            cfg,
        }
    }

    pub fn config(&self) -> &TransferConfig {
        &self.cfg
    }

    /// Nombre de blocs dans la moyenne courante.
    pub fn blocks(&self) -> usize {
        self.blocks
    }

    pub fn delay(&self) -> usize {
        self.cfg.delay
    }

    /// Change le retard de la référence et remet les moyennes à zéro.
    pub fn set_delay(&mut self, delay: usize) {
        let old = self.cfg.delay;
        if delay > old {
            for _ in 0..delay - old {
                self.ref_fifo.push_front(0.0);
            }
        } else {
            let drop = (old - delay).min(self.ref_fifo.len());
            self.ref_fifo.drain(..drop);
        }
        self.cfg.delay = delay;
        self.reset();
    }

    /// Remet les moyennes à zéro (les signaux en attente sont gardés).
    pub fn reset(&mut self) {
        self.gxx.iter_mut().for_each(|v| *v = 0.0);
        self.gyy.iter_mut().for_each(|v| *v = 0.0);
        self.gxy.iter_mut().for_each(|v| *v = Complex64::new(0.0, 0.0));
        self.ring_pos = 0;
        self.blocks = 0;
    }

    /// Ajoute des échantillons des deux canaux et traite les blocs complets.
    pub fn push(&mut self, reference: &[f32], measured: &[f32]) {
        self.ref_fifo.extend(reference.iter().map(|&v| v as f64));
        self.meas_fifo.extend(measured.iter().map(|&v| v as f64));
        let n = self.cfg.fft_len;
        while self.ref_fifo.len() >= n && self.meas_fifo.len() >= n {
            self.process_block();
            self.ref_fifo.drain(..self.hop);
            self.meas_fifo.drain(..self.hop);
        }
    }

    fn process_block(&mut self) {
        for ((d, &s), &w) in self.input.iter_mut().zip(self.ref_fifo.iter()).zip(&self.window) {
            *d = s * w;
        }
        self.plan.process(&mut self.input, &mut self.spec_x).expect("tailles cohérentes");
        for ((d, &s), &w) in self.input.iter_mut().zip(self.meas_fifo.iter()).zip(&self.window) {
            *d = s * w;
        }
        self.plan.process(&mut self.input, &mut self.spec_y).expect("tailles cohérentes");

        let bins = self.gxx.len();
        match self.cfg.averaging {
            Averaging::Linear(count) => {
                let count = count.max(1);
                let base = self.ring_pos * bins;
                let full = self.blocks == count;
                for k in 0..bins {
                    let (x, y) = (self.spec_x[k], self.spec_y[k]);
                    let (pxx, pyy, pxy) = (x.norm_sqr(), y.norm_sqr(), x.conj() * y);
                    if full {
                        self.gxx[k] -= self.ring_xx[base + k];
                        self.gyy[k] -= self.ring_yy[base + k];
                        self.gxy[k] -= self.ring_xy[base + k];
                    }
                    self.ring_xx[base + k] = pxx;
                    self.ring_yy[base + k] = pyy;
                    self.ring_xy[base + k] = pxy;
                    self.gxx[k] += pxx;
                    self.gyy[k] += pyy;
                    self.gxy[k] += pxy;
                }
                self.ring_pos = (self.ring_pos + 1) % count;
                if !full {
                    self.blocks += 1;
                }
            }
            Averaging::Exponential(alpha) => {
                let a = if self.blocks == 0 { 1.0 } else { alpha.clamp(1e-6, 1.0) };
                for k in 0..bins {
                    let (x, y) = (self.spec_x[k], self.spec_y[k]);
                    self.gxx[k] += a * (x.norm_sqr() - self.gxx[k]);
                    self.gyy[k] += a * (y.norm_sqr() - self.gyy[k]);
                    let g = self.gxy[k];
                    self.gxy[k] = g + (x.conj() * y - g) * a;
                }
                self.blocks += 1;
            }
            Averaging::Infinite => {
                for k in 0..bins {
                    let (x, y) = (self.spec_x[k], self.spec_y[k]);
                    self.gxx[k] += x.norm_sqr();
                    self.gyy[k] += y.norm_sqr();
                    self.gxy[k] += x.conj() * y;
                }
                self.blocks += 1;
            }
        }
    }

    /// Fonction de transfert moyennée, ou `None` avant le premier bloc.
    pub fn result(&self) -> Option<Transfer> {
        if self.blocks == 0 {
            return None;
        }
        let scale = match self.cfg.averaging {
            Averaging::Exponential(_) => 1.0,
            _ => 1.0 / self.blocks as f64,
        };
        let mut h = Vec::with_capacity(self.gxx.len());
        let mut coherence = Vec::with_capacity(self.gxx.len());
        for k in 0..self.gxx.len() {
            let (gxx, gyy, gxy) = (self.gxx[k].max(0.0), self.gyy[k].max(0.0), self.gxy[k]);
            if gxx > f64::MIN_POSITIVE {
                h.push(gxy / gxx);
            } else {
                h.push(Complex64::new(0.0, 0.0));
            }
            let den = gxx * gyy;
            coherence.push(if den > f64::MIN_POSITIVE { (gxy.norm_sqr() / den).min(1.0) } else { 0.0 });
        }
        Some(Transfer {
            bin_hz: self.cfg.sample_rate / self.cfg.fft_len as f64,
            h,
            coherence,
            gxx: self.gxx.iter().map(|v| v * scale).collect(),
            gyy: self.gyy.iter().map(|v| v * scale).collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{Generator, PinkNoise, WhiteNoise};
    use crate::test_util::Biquad;

    fn mean_in(values: &[f64], bin_hz: f64, lo: f64, hi: f64) -> f64 {
        let (a, b) = ((lo / bin_hz).ceil() as usize, (hi / bin_hz).floor() as usize);
        values[a..=b].iter().sum::<f64>() / (b - a + 1) as f64
    }

    /// Bruit rose à travers un biquad et un retard de 37 échantillons, plus un bruit blanc
    /// non corrélé : H juste là où le signal domine, cohérence basse là où le bruit domine.
    #[test]
    fn transfert_avec_bruit() {
        let fs = 48_000.0;
        let bq = Biquad::peaking(fs, 1000.0, 1.0, 6.0);
        let mut pink = PinkNoise::new(1.0, 11);
        let mut x = vec![0.0f32; 48_000 * 6];
        pink.fill(&mut x);
        let delay = 37;
        let filtered = bq.filter(&crate::to_f64(&x));
        let mut n = vec![0.0f32; x.len()];
        WhiteNoise::new(0.15, 99).fill(&mut n);
        let y: Vec<f32> = (0..x.len()).map(|i| if i >= delay { filtered[i - delay] as f32 } else { 0.0 } + n[i]).collect();

        for averaging in [Averaging::Infinite, Averaging::Linear(64), Averaging::Exponential(0.05)] {
            let mut fft = Fft::new();
            let cfg = TransferConfig { sample_rate: fs, fft_len: 8192, overlap: 0.5, window: Window::Hann, averaging, delay };
            let mut e = TransferEngine::new(cfg, &mut fft);
            for (a, b) in x.chunks(512).zip(y.chunks(512)) {
                e.push(a, b);
            }
            let t = e.result().unwrap();
            let freqs = t.frequencies();
            let mag = t.magnitude_db();
            let mut worst: f64 = 0.0;
            let mut sum = 0.0;
            let mut count = 0;
            for (k, &f) in freqs.iter().enumerate() {
                if (80.0..=1500.0).contains(&f) {
                    let err = mag[k] - crate::db20(bq.magnitude(f));
                    worst = worst.max(err.abs());
                    sum += err.abs();
                    count += 1;
                }
            }
            let g_lo = mean_in(&t.coherence, t.bin_hz, 100.0, 300.0);
            let g_hi = mean_in(&t.coherence, t.bin_hz, 12_000.0, 20_000.0);
            let phase_1k = t.phase_deg()[(1000.0 / t.bin_hz).round() as usize];
            println!("{averaging:?} : {} blocs, écart |H| moyen {:.3} dB (max {worst:.3}) sur 80–1500 Hz, γ² {g_lo:.3} (100–300 Hz) / {g_hi:.3} (12–20 kHz), phase à 1 kHz {phase_1k:.1}°", e.blocks(), sum / count as f64);
            assert!(sum / (count as f64) < 0.3);
            assert!(g_lo > 0.9);
            assert!(g_hi < 0.6);
            // Retard compensé : phase de la cloche seule (≈ 0° au centre).
            assert!(phase_1k.abs() < 10.0);
        }
    }

    /// Sans bruit, H est le biquad lui-même à 0,1 dB près et γ² ≈ 1.
    #[test]
    fn transfert_propre() {
        let fs = 48_000.0;
        let bq = Biquad::high_shelf(fs, 4000.0, 0.707, -6.0);
        let mut x = vec![0.0f32; 48_000 * 4];
        PinkNoise::new(1.0, 5).fill(&mut x);
        let y: Vec<f32> = bq.filter(&crate::to_f64(&x)).iter().map(|&v| v as f32).collect();
        let mut fft = Fft::new();
        let mut e = TransferEngine::new(TransferConfig { sample_rate: fs, fft_len: 16_384, ..Default::default() }, &mut fft);
        e.push(&x, &y);
        let t = e.result().unwrap();
        let mag = t.magnitude_db();
        for (k, f) in t.frequencies().into_iter().enumerate() {
            if (50.0..=15_000.0).contains(&f) {
                assert!((mag[k] - crate::db20(bq.magnitude(f))).abs() < 0.1, "{f} Hz");
                assert!(t.coherence[k] > 0.99, "{f} Hz : γ² {}", t.coherence[k]);
            }
        }
        // Changer le retard remet la moyenne à zéro.
        e.set_delay(10);
        assert_eq!(e.blocks(), 0);
        assert!(e.result().is_none());
    }
}
