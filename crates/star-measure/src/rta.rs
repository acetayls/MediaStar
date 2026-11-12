//! Analyse en temps réel (RTA) : bandes fractionnaires d'octave, pondérations, spectre moyenné.
//!
//! Bandes selon IEC 61260-1, en base dix (G = 10^(3/10)) autour de 1000 Hz :
//! centre 1000·G^(x/b) pour b impair, 1000·G^((2x+1)/(2b)) pour b pair ;
//! bords au centre ×/÷ G^(1/(2b)).
//! Pondérations A et C selon IEC 61672-1 (expressions analytiques), 0 dB à 1 kHz.

use crate::fft::{Fft, Window};

/// Base des bandes d'octave en base dix.
pub const OCTAVE_RATIO: f64 = 1.995_262_314_968_879_5; // 10^(3/10)

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Band {
    pub center: f64,
    pub low: f64,
    pub high: f64,
}

/// Bandes de 1/`fraction` d'octave (1, 2, 3, 6, 12, 24…) dont le centre tombe dans
/// [`fmin`, `fmax`], à une demi-bande près (la bande de 20 Hz, centrée sur 19,95 Hz, est
/// gardée pour `fmin` = 20).
pub fn fractional_octave_bands(fraction: u32, fmin: f64, fmax: f64) -> Vec<Band> {
    let b = fraction.max(1) as f64;
    let g = OCTAVE_RATIO;
    let half = g.powf(1.0 / (2.0 * b));
    let center = |x: i32| {
        if fraction % 2 == 1 {
            1000.0 * g.powf(x as f64 / b)
        } else {
            1000.0 * g.powf((2 * x + 1) as f64 / (2.0 * b))
        }
    };
    let xmin = (b * (fmin / 1000.0).ln() / g.ln()).floor() as i32 - 2;
    let xmax = (b * (fmax / 1000.0).ln() / g.ln()).ceil() as i32 + 2;
    (xmin..=xmax)
        .map(center)
        .filter(|&c| c >= fmin / half && c <= fmax * half)
        .map(|c| Band { center: c, low: c / half, high: c * half })
        .collect()
}

/// Puissance par bande, à partir d'une puissance par case de FFT (`bin_hz` de large, case `k`
/// centrée sur k·bin_hz). Une case à cheval sur deux bandes est partagée au prorata.
pub fn band_powers(power: &[f64], bin_hz: f64, bands: &[Band]) -> Vec<f64> {
    bands
        .iter()
        .map(|band| {
            let lo = band.low / bin_hz;
            let hi = band.high / bin_hz;
            let k0 = (lo + 0.5).floor().max(0.0) as usize;
            let k1 = ((hi + 0.5).ceil() as usize).min(power.len());
            (k0..k1)
                .map(|k| {
                    let a = (k as f64 - 0.5).max(lo);
                    let b = (k as f64 + 0.5).min(hi);
                    if b > a {
                        power[k] * (b - a)
                    } else {
                        0.0
                    }
                })
                .sum()
        })
        .collect()
}

/// Pondération fréquentielle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Weighting {
    A,
    C,
    Z,
}

impl Weighting {
    /// Gain en dB à la fréquence `f` (Hz).
    pub fn gain_db(self, f: f64) -> f64 {
        match self {
            Weighting::Z => 0.0,
            _ => 20.0 * (self.raw(f) / self.raw(1000.0)).log10(),
        }
    }

    /// Gain en amplitude.
    pub fn gain(self, f: f64) -> f64 {
        10f64.powf(self.gain_db(f) / 20.0)
    }

    fn raw(self, f: f64) -> f64 {
        const F1: f64 = 20.598_997;
        const F2: f64 = 107.652_65;
        const F3: f64 = 737.862_23;
        const F4: f64 = 12_194.217;
        let f2 = f * f;
        match self {
            Weighting::A => F4 * F4 * f2 * f2 / ((f2 + F1 * F1) * ((f2 + F2 * F2) * (f2 + F3 * F3)).sqrt() * (f2 + F4 * F4)),
            Weighting::C => F4 * F4 * f2 / ((f2 + F1 * F1) * (f2 + F4 * F4)),
            Weighting::Z => 1.0,
        }
    }
}

/// Puissance moyenne par case (méthode de Welch), normalisée pour que la somme des cases
/// donne la valeur quadratique moyenne du signal (un sinus d'amplitude 1 donne 0,5,
/// soit −3 dB re 1 ; ajouter 3 dB pour une échelle « dBFS sinus »).
pub fn welch_power(signal: &[f32], fft_len: usize, overlap: f64, window: Window, fft: &mut Fft) -> Vec<f64> {
    let bins = fft_len / 2 + 1;
    let mut acc = vec![0.0; bins];
    if signal.len() < fft_len {
        return acc;
    }
    let w = window.coefficients(fft_len);
    let wsum2: f64 = w.iter().map(|v| v * v).sum();
    let hop = ((fft_len as f64 * (1.0 - overlap.clamp(0.0, 0.95))).round() as usize).max(1);
    let plan = fft.forward_plan(fft_len);
    let mut input = vec![0.0; fft_len];
    let mut out = plan.make_output_vec();
    let mut blocks = 0usize;
    let mut start = 0;
    while start + fft_len <= signal.len() {
        for ((d, &s), &wi) in input.iter_mut().zip(&signal[start..start + fft_len]).zip(&w) {
            *d = s as f64 * wi;
        }
        plan.process(&mut input, &mut out).expect("tailles cohérentes");
        for (a, c) in acc.iter_mut().zip(&out) {
            *a += c.norm_sqr();
        }
        blocks += 1;
        start += hop;
    }
    let norm = 1.0 / (blocks as f64 * fft_len as f64 * wsum2);
    let last = bins - 1;
    for (k, a) in acc.iter_mut().enumerate() {
        let side = if k == 0 || (k == last && fft_len.is_multiple_of(2)) { 1.0 } else { 2.0 };
        *a *= norm * side;
    }
    acc
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{Generator, Sine};

    #[test]
    fn bandes_tiers_octave() {
        let b = fractional_octave_bands(3, 20.0, 20_000.0);
        assert_eq!(b.len(), 31);
        assert!(b.iter().any(|x| (x.center - 1000.0).abs() < 1e-9));
        assert!((b[0].center - 19.953).abs() < 0.01);
        // Bandes contiguës.
        for w in b.windows(2) {
            assert!((w[0].high - w[1].low).abs() < 1e-9);
        }
        let b24 = fractional_octave_bands(24, 20.0, 20_000.0);
        assert_eq!(b24.len(), 241);
        let b1 = fractional_octave_bands(1, 31.5, 16_000.0);
        assert_eq!(b1.len(), 10);
        // b pair : 1000 Hz est une limite de bande, pas un centre.
        let b6 = fractional_octave_bands(6, 900.0, 1100.0);
        assert!(b6.iter().any(|x| (x.high - 1000.0).abs() < 1e-9));
    }

    #[test]
    fn ponderations() {
        let a1k = Weighting::A.gain_db(1000.0);
        let a100 = Weighting::A.gain_db(100.0);
        println!("A(1 kHz) = {a1k:.4} dB, A(100 Hz) = {a100:.3} dB, A(10 kHz) = {:.3} dB, C(100 Hz) = {:.3} dB", Weighting::A.gain_db(10_000.0), Weighting::C.gain_db(100.0));
        assert!(a1k.abs() < 1e-9);
        assert!((a100 + 19.1).abs() < 0.1);
        assert!((Weighting::A.gain_db(10_000.0) + 2.5).abs() < 0.1);
        assert!((Weighting::A.gain_db(31.5) + 39.4).abs() < 0.2);
        assert!(Weighting::C.gain_db(1000.0).abs() < 1e-9);
        assert!((Weighting::C.gain_db(100.0) + 0.3).abs() < 0.1);
        assert!((Weighting::C.gain_db(31.5) + 3.0).abs() < 0.2);
        assert_eq!(Weighting::Z.gain_db(50.0), 0.0);
    }

    #[test]
    fn sinus_dans_sa_bande() {
        let mut s = Sine::new(1000.0, 48_000.0, 1.0);
        let mut x = vec![0.0f32; 48_000];
        s.fill(&mut x);
        let mut fft = Fft::new();
        let p = welch_power(&x, 8192, 0.5, Window::Hann, &mut fft);
        let bands = fractional_octave_bands(3, 20.0, 20_000.0);
        let lv = band_powers(&p, 48_000.0 / 8192.0, &bands);
        let i = bands.iter().position(|b| (b.center - 1000.0).abs() < 1.0).unwrap();
        let l = crate::db10(lv[i]);
        println!("sinus 1 kHz d'amplitude 1 : {l:.3} dB dans la bande de 1 kHz");
        assert!((l + 3.01).abs() < 0.1);
        assert!(crate::db10(lv[i + 2]) < l - 40.0);
    }
}
