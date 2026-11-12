//! Réponse impulsionnelle : partie linéaire, harmoniques, ETC, fenêtrage du direct.

use std::f64::consts::PI;

use num_complex::Complex64;

use crate::delay::peak_position;
use crate::fft::{next_pow2, Fft};
use crate::sweep::Sweep;

/// Recherche du pic principal : on remonte au plus de 5 ms avant t = 0.
const PEAK_GUARD_S: f64 = 0.005;

/// Bord montant devant chaque réponse extraite (au plus 2 ms).
const MAX_PRE_S: f64 = 0.002;

/// Réponse impulsionnelle déconvoluée. `samples[zero]` correspond à t = 0 (sortie du sweep
/// sans délai) ; ce qui précède contient les harmoniques d'un sweep.
#[derive(Debug, Clone)]
pub struct Ir {
    pub samples: Vec<f64>,
    pub zero: usize,
    pub sample_rate: f64,
}

/// Réponse d'une harmonique (ordre 1 = réponse linéaire).
#[derive(Debug, Clone)]
pub struct Harmonic {
    pub order: usize,
    pub samples: Vec<f64>,
    /// Position (fractionnaire) de t = 0 dans `samples`.
    pub zero: f64,
}

impl Ir {
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Partie causale, à partir de t = 0.
    pub fn causal(&self) -> &[f64] {
        &self.samples[self.zero..]
    }

    /// Position du pic principal, en échantillons après t = 0 (interpolation parabolique).
    pub fn peak_samples(&self) -> f64 {
        let guard = (PEAK_GUARD_S * self.sample_rate) as usize;
        let start = self.zero.saturating_sub(guard);
        peak_position(&self.samples[start..]).map_or(0.0, |p| p + start as f64 - self.zero as f64)
    }

    /// Délai du pic principal (ms).
    pub fn delay_ms(&self) -> f64 {
        self.peak_samples() * 1000.0 / self.sample_rate
    }

    /// `len` échantillons à partir de `pre` avant t = 0 (circulaire, sans fenêtre).
    pub fn linear(&self, pre: usize, len: usize) -> Vec<f64> {
        let n = self.samples.len();
        let start = (self.zero + n - pre % n) % n;
        (0..len).map(|i| self.samples[(start + i) % n]).collect()
    }

    /// Sépare la réponse linéaire et les harmoniques 2 à `max_order` d'un sweep synchronisé.
    ///
    /// L'harmonique `k` est centrée `L·ln k` avant t = 0 et s'étend jusqu'au début de la
    /// fenêtre de l'harmonique `k − 1` ; chaque fenêtre monte en demi-Hann sur un dixième de
    /// l'écart à l'harmonique suivante (2 ms au plus) et redescend sur son dernier dixième.
    /// La réponse linéaire va de t = 0 jusqu'à la fin de la partie causale.
    pub fn harmonics(&self, sweep: &Sweep, max_order: usize) -> Vec<Harmonic> {
        let fs = self.sample_rate;
        let l = sweep.rate();
        let z = self.zero as f64;
        let center = |k: usize| z - l * (k as f64).ln() * fs;
        let pre = |k: usize| (0.1 * l * ((k + 1) as f64 / k as f64).ln() * fs).min(MAX_PRE_S * fs);
        let mut out = Vec::new();
        for k in 1..=max_order.max(1) {
            let left = (center(k) - pre(k)).floor();
            if left < 0.0 {
                break;
            }
            let start = left as usize;
            let end = if k == 1 { self.samples.len() } else { (center(k - 1) - pre(k - 1)).floor().max(left) as usize };
            let mut samples = self.samples[start..end].to_vec();
            let rise = (pre(k) as usize).min(samples.len());
            for (i, v) in samples.iter_mut().take(rise).enumerate() {
                *v *= 0.5 - 0.5 * (PI * i as f64 / rise as f64).cos();
            }
            if k > 1 {
                let n = samples.len();
                let fall = n / 10;
                for i in 0..fall {
                    samples[n - 1 - i] *= 0.5 - 0.5 * (PI * i as f64 / fall as f64).cos();
                }
            }
            out.push(Harmonic { order: k, samples, zero: center(k) - start as f64 });
        }
        out
    }
}

/// Courbe énergie-temps (ETC) en dB, 0 dB au maximum : enveloppe de Hilbert au carré.
pub fn etc_db(samples: &[f64], fft: &mut Fft) -> Vec<f64> {
    if samples.is_empty() {
        return Vec::new();
    }
    let n = next_pow2(samples.len());
    let mut spec = fft.forward(samples, n);
    // Transformée de Hilbert : −j sur les fréquences positives, 0 au continu et à Nyquist.
    spec[0] = Complex64::new(0.0, 0.0);
    let last = spec.len() - 1;
    spec[last] = Complex64::new(0.0, 0.0);
    for c in spec.iter_mut().take(last).skip(1) {
        *c *= Complex64::new(0.0, -1.0);
    }
    let hilbert = fft.inverse(&spec, n);
    let energy: Vec<f64> = samples.iter().zip(&hilbert).map(|(a, b)| a * a + b * b).collect();
    let max = energy.iter().cloned().fold(0.0, f64::max);
    if max <= 0.0 {
        return vec![crate::FLOOR_DB; samples.len()];
    }
    energy.iter().map(|&e| crate::db10(e / max)).collect()
}

/// Réponse fenêtrée autour du direct (indice `peak`) : montée demi-Hann sur `pre`
/// échantillons, plateau, descente demi-Hann sur le dernier quart de `post`.
pub fn windowed(samples: &[f64], peak: usize, pre: usize, post: usize) -> Vec<f64> {
    let start = peak.saturating_sub(pre);
    let end = (peak + post).min(samples.len());
    let rise = peak - start;
    let mut out = samples[start..end].to_vec();
    for (i, v) in out.iter_mut().take(rise).enumerate() {
        *v *= 0.5 - 0.5 * (PI * i as f64 / rise as f64).cos();
    }
    let n = out.len();
    let fall = (post / 4).min(n - rise);
    for i in 0..fall {
        out[n - 1 - i] *= 0.5 - 0.5 * (PI * i as f64 / fall as f64).cos();
    }
    out
}

/// Réponse en fréquence d'une réponse impulsionnelle, sur `n` points.
pub fn frequency_response(samples: &[f64], n: usize, fft: &mut Fft) -> Vec<Complex64> {
    fft.forward(samples, n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sweep::SweepParams;
    use crate::to_f64;

    /// Distorsion cubique : l'harmonique 3 apparaît L·ln 3 avant la réponse linéaire,
    /// au niveau attendu (−a/4 pour y = x + a·x³ avec un sweep d'amplitude 1), sans H2.
    #[test]
    fn harmonique_3_isolee() {
        let fs = 48_000.0;
        // f2 = 6 kHz : 3·f2 reste sous Nyquist, pas de repliement.
        let s = Sweep::new(SweepParams { f1: 20.0, f2: 6000.0, duration: 2.0, sample_rate: fs, fade_in: 0.05, fade_out: 0.005, amplitude: 1.0 });
        let a = 0.1;
        let mut y: Vec<f32> = to_f64(s.signal()).iter().map(|&x| (x + a * x * x * x) as f32).collect();
        y.extend(std::iter::repeat_n(0.0, 4800));
        let mut fft = Fft::new();
        let ir = s.deconvolve(&y, &mut fft);
        let h = ir.harmonics(&s, 3);
        assert_eq!(h.len(), 3);

        let peak = |x: &[f64]| x.iter().cloned().fold(0.0, |m: f64, v| if v.abs() > m.abs() { v } else { m });
        let p1 = peak(&h[0].samples);
        let p2 = peak(&h[1].samples);
        let p3 = peak(&h[2].samples);
        let pos3 = peak_position(&h[2].samples).unwrap();
        let ratio = p3 / p1;
        println!("H1 = {p1:.4}, H2 = {p2:.2e}, H3 = {p3:.5} (rapport {ratio:.5}, attendu ≈ {:.5}), position H3 {pos3:.2} pour {:.2}", -a / 4.0 / (1.0 + 0.75 * a), h[2].zero);
        // Pic d'une impulsion limitée à [f1, f2] de gain 1 : 2·(f2 − f1)/fs.
        let p1_attendu = (1.0 + 0.75 * a) * 2.0 * (6000.0 - 20.0) / fs;
        assert!((p1 / p1_attendu - 1.0).abs() < 0.05, "H1 {p1}, attendu {p1_attendu}");
        assert!(ratio < -0.018 && ratio > -0.028);
        assert!((pos3 - h[2].zero).abs() < 2.0);
        // Pas de distorsion paire : H2 reste au niveau des rebonds de H1 (< −50 dB).
        assert!(p2.abs() < 3e-3 * p1.abs());
        // L'harmonique 3 précède bien la réponse linéaire : son pic est hors de la partie causale.
        let avance = s.harmonic_offset(3) * fs;
        assert!(avance > 1000.0);
        let causal_max = ir.causal()[..(0.01 * fs) as usize].iter().cloned().fold(0.0, |m: f64, v| m.max(v.abs()));
        assert!((causal_max - p1.abs()).abs() < 1e-9, "le pic linéaire est dans la partie causale");
    }

    #[test]
    fn etc_impulsion() {
        let mut x = vec![0.0; 1024];
        x[100] = 1.0;
        x[300] = 0.1;
        let mut fft = Fft::new();
        let e = etc_db(&x, &mut fft);
        assert!(e[100].abs() < 0.5);
        assert!((e[300] + 20.0).abs() < 1.0);
    }

    #[test]
    fn fenetre_direct() {
        let x = vec![1.0; 1000];
        let w = windowed(&x, 500, 100, 400);
        assert_eq!(w.len(), 500);
        assert_eq!(w[0], 0.0);
        assert_eq!(w[100], 1.0);
        assert_eq!(w[300], 1.0);
        assert!(w[499] < 0.01);
    }
}
