//! Sweep sinusoïdal exponentiel et déconvolution.
//!
//! Le sweep est « synchronisé » (Novak, 2015) : la constante de temps `L` est arrondie pour
//! que `f1·L` soit entier. Ainsi la phase de chaque harmonique `k` coïncide avec celle du sweep
//! avancé de `L·ln k` : après déconvolution, la réponse de l'harmonique `k` se retrouve,
//! propre, `L·ln k` secondes avant la réponse linéaire (méthode de Farina).
//!
//! x(t) = A·sin(2π·f1·L·(e^(t/L) − 1)), fréquence instantanée f1·e^(t/L), durée L·ln(f2/f1).
//!
//! Deux déconvolutions :
//! - [`Sweep::deconvolve`] : par le filtre inverse analytique (sweep retourné dans le temps,
//!   enveloppe décroissant de 6 dB/octave pour compenser l'énergie du sweep, plus forte dans
//!   le grave), normalisé pour que sweep ∗ inverse ait un gain de 1 dans la bande ;
//! - [`deconvolve_with_reference`] : division spectrale par une référence captée (boucle
//!   électrique), régularisée hors de la bande utile. La latence de la chaîne et la couleur
//!   de la carte son s'annulent.

use std::f64::consts::PI;

use num_complex::Complex64;

use crate::fft::{apply_fades, next_pow2, Fft};
use crate::ir::Ir;
use crate::to_f64;

/// Régularisation relative dans la bande pour la division spectrale (−80 dB).
pub const DEFAULT_REGULARIZATION: f64 = 1e-8;

/// Transition de la régularisation de part et d'autre de la bande, en octaves.
const TRANSITION_OCT: f64 = 1.0 / 3.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SweepParams {
    /// Fréquence de départ (Hz).
    pub f1: f64,
    /// Fréquence d'arrivée (Hz), sous Nyquist.
    pub f2: f64,
    /// Durée souhaitée (s) ; la durée réelle est ajustée par la synchronisation.
    pub duration: f64,
    pub sample_rate: f64,
    /// Fondu d'entrée (s), demi-Hann.
    pub fade_in: f64,
    /// Fondu de sortie (s), demi-Hann.
    pub fade_out: f64,
    /// Amplitude crête (1 = pleine échelle).
    pub amplitude: f64,
}

impl Default for SweepParams {
    fn default() -> Self {
        Self { f1: 20.0, f2: 20_000.0, duration: 3.0, sample_rate: 48_000.0, fade_in: 0.05, fade_out: 0.005, amplitude: 0.5 }
    }
}

pub struct Sweep {
    params: SweepParams,
    /// Constante de temps L (s).
    rate: f64,
    signal: Vec<f32>,
    inverse: Vec<f64>,
}

impl Sweep {
    pub fn new(params: SweepParams) -> Self {
        let SweepParams { f1, f2, duration, sample_rate: fs, fade_in, fade_out, amplitude } = params;
        assert!(f1 > 0.0 && f2 > f1 && f2 <= fs / 2.0, "bande du sweep invalide");
        assert!(duration > 0.0, "durée du sweep invalide");
        let ratio = (f2 / f1).ln();
        let k = (f1 * duration / ratio).round().max(1.0);
        let rate = k / f1;
        let len = (rate * ratio * fs).round() as usize;

        let mut x: Vec<f64> = (0..len)
            .map(|n| {
                let t = n as f64 / fs;
                amplitude * (2.0 * PI * f1 * rate * ((t / rate).exp() - 1.0)).sin()
            })
            .collect();
        apply_fades(&mut x, (fade_in * fs) as usize, (fade_out * fs) as usize);

        // Filtre inverse : sweep retourné, enveloppe e^(−t/L) (−6 dB/octave vers le grave).
        let mut inverse: Vec<f64> = (0..len).map(|n| x[len - 1 - n] * (-(n as f64) / fs / rate).exp()).collect();

        // Normalisation : |X·V| ramené à 1 (moyenne géométrique sur la bande [2·f1, f2/2]).
        let mut fft = Fft::new();
        let n = next_pow2(2 * len);
        let sx = fft.forward(&x, n);
        let sv = fft.forward(&inverse, n);
        let (lo, hi) = ((2.0 * f1 / fs * n as f64) as usize, (f2 / 2.0 / fs * n as f64) as usize);
        let (mut sum, mut count) = (0.0, 0usize);
        for kk in lo.max(1)..=hi.min(n / 2) {
            let m = (sx[kk] * sv[kk]).norm();
            if m > 0.0 {
                sum += m.ln();
                count += 1;
            }
        }
        if count > 0 {
            let g = (sum / count as f64).exp();
            inverse.iter_mut().for_each(|v| *v /= g);
        }

        let signal = x.iter().map(|&v| v as f32).collect();
        Self { params, rate, signal, inverse }
    }

    pub fn params(&self) -> &SweepParams {
        &self.params
    }

    /// Signal à jouer.
    pub fn signal(&self) -> &[f32] {
        &self.signal
    }

    /// Filtre inverse normalisé.
    pub fn inverse(&self) -> &[f64] {
        &self.inverse
    }

    pub fn len(&self) -> usize {
        self.signal.len()
    }

    pub fn is_empty(&self) -> bool {
        self.signal.is_empty()
    }

    /// Durée réelle (s).
    pub fn duration(&self) -> f64 {
        self.len() as f64 / self.params.sample_rate
    }

    /// Constante de temps L (s).
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Instant (s) où le sweep passe par la fréquence `f`.
    pub fn time_at(&self, f: f64) -> f64 {
        self.rate * (f / self.params.f1).ln()
    }

    /// Avance (s) de la réponse de l'harmonique `k` sur la réponse linéaire.
    pub fn harmonic_offset(&self, k: usize) -> f64 {
        self.rate * (k.max(1) as f64).ln()
    }

    /// Réponse impulsionnelle d'un enregistrement du sweep (le sweep et sa traîne).
    /// La réponse linéaire commence à `Ir::zero` ; les harmoniques sont avant.
    pub fn deconvolve(&self, recording: &[f32], fft: &mut Fft) -> Ir {
        let n = next_pow2(recording.len() + self.inverse.len());
        let y = fft.forward(&to_f64(recording), n);
        let v = fft.forward(&self.inverse, n);
        let prod: Vec<Complex64> = y.iter().zip(&v).map(|(a, b)| a * b).collect();
        Ir { samples: fft.inverse(&prod, n), zero: self.inverse.len() - 1, sample_rate: self.params.sample_rate }
    }
}

/// Réponse impulsionnelle par division spectrale : H = Y·X* / (|X|² + ε).
///
/// `reference` est le signal capté en boucle (ou le sweep joué), `recording` le signal du
/// micro, de même origine des temps. ε vaut `regularization`·max|X|² dans `band` (Hz) et
/// monte jusqu'à max|X|² au-delà, sur un tiers d'octave, pour ne pas amplifier le bruit là
/// où la référence n'a pas d'énergie. Le résultat garde `reference.len()` échantillons avant
/// t = 0, où tombent les harmoniques d'un sweep.
pub fn deconvolve_with_reference(
    recording: &[f32],
    reference: &[f32],
    sample_rate: f64,
    band: (f64, f64),
    regularization: f64,
    fft: &mut Fft,
) -> Ir {
    let n = next_pow2(recording.len() + reference.len());
    let y = fft.forward(&to_f64(recording), n);
    let x = fft.forward(&to_f64(reference), n);
    let max = x.iter().map(|c| c.norm_sqr()).fold(0.0, f64::max).max(f64::MIN_POSITIVE);
    let inside = (regularization * max).max(f64::MIN_POSITIVE);
    let prod: Vec<Complex64> = y
        .iter()
        .zip(&x)
        .enumerate()
        .map(|(k, (yk, xk))| {
            let f = k as f64 * sample_rate / n as f64;
            let eps = regularization_at(f, band, inside, max);
            yk * xk.conj() / (xk.norm_sqr() + eps)
        })
        .collect();
    let h = fft.inverse(&prod, n);
    let zero = reference.len().min(n / 2);
    let samples = (0..n).map(|i| h[(i + n - zero) % n]).collect();
    Ir { samples, zero, sample_rate }
}

/// ε à la fréquence `f` : `inside` dans la bande, montée en cosinus (en log) jusqu'à
/// `outside` sur [`TRANSITION_OCT`] octave hors de la bande.
fn regularization_at(f: f64, band: (f64, f64), inside: f64, outside: f64) -> f64 {
    let (lo, hi) = band;
    let d = if f < lo {
        (lo / f.max(1e-9)).log2()
    } else if f > hi {
        (f / hi).log2()
    } else {
        0.0
    };
    let t = (d / TRANSITION_OCT).min(1.0);
    let s = 0.5 - 0.5 * (PI * t).cos();
    inside * (outside / inside).powf(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_util::{fractional_delay, Biquad};

    fn params() -> SweepParams {
        SweepParams { f1: 20.0, f2: 20_000.0, duration: 2.0, sample_rate: 48_000.0, fade_in: 0.05, fade_out: 0.005, amplitude: 0.5 }
    }

    #[test]
    fn synchronisation() {
        let s = Sweep::new(params());
        assert!((s.params().f1 * s.rate() - (s.params().f1 * s.rate()).round()).abs() < 1e-9);
        // L est arrondie à un multiple de 1/f1 : la durée bouge d'au plus ln(f2/f1)/(2·f1).
        assert!((s.duration() - 2.0).abs() <= (1000f64).ln() / 40.0 + 1e-3, "durée {}", s.duration());
        assert!((s.time_at(20_000.0) - s.duration()).abs() < 1e-3);
    }

    /// Délai entier et fractionnaire retrouvés à 0,01 ms près, par les deux déconvolutions.
    #[test]
    fn delai_retrouve() {
        let s = Sweep::new(params());
        let fs = 48_000.0;
        let mut fft = Fft::new();
        for delay_ms in [10.0, 10.00625, 3.3333] {
            let d = delay_ms * fs / 1000.0;
            let rec = fractional_delay(&to_f64(s.signal()), d, 4800, &mut fft);
            let rec32: Vec<f32> = rec.iter().map(|&v| v as f32).collect();
            let ir = s.deconvolve(&rec32, &mut fft);
            let got = ir.delay_ms();
            println!("filtre inverse : attendu {delay_ms:.5} ms, trouvé {got:.5} ms");
            assert!((got - delay_ms).abs() < 0.01);

            let ir = deconvolve_with_reference(&rec32, s.signal(), fs, (20.0, 20_000.0), DEFAULT_REGULARIZATION, &mut fft);
            let got = ir.delay_ms();
            println!("division spectrale : attendu {delay_ms:.5} ms, trouvé {got:.5} ms");
            assert!((got - delay_ms).abs() < 0.01);
        }
    }

    /// Magnitude d'un biquad connu retrouvée à 0,1 dB près entre 50 Hz et 15 kHz.
    #[test]
    fn magnitude_biquad() {
        // Sweep de 3 s (réglage par défaut) : l'ondulation du filtre inverse près de f1
        // reste sous 0,1 dB à 50 Hz.
        let s = Sweep::new(SweepParams { duration: 3.0, ..params() });
        let fs = 48_000.0;
        let mut fft = Fft::new();
        for (kind, bq) in [("cloche +6 dB 1 kHz", Biquad::peaking(fs, 1000.0, 1.0, 6.0)), ("cloche −9 dB 120 Hz", Biquad::peaking(fs, 120.0, 2.0, -9.0)), ("shelf aigu +4 dB 6 kHz", Biquad::high_shelf(fs, 6000.0, 0.707, 4.0))] {
            let mut x = to_f64(s.signal());
            x.extend(std::iter::repeat_n(0.0, 24_000));
            let y: Vec<f32> = bq.filter(&x).iter().map(|&v| v as f32).collect();

            let pre = 4096;
            let len = 65_536;
            for (methode, ir) in [
                ("filtre inverse", s.deconvolve(&y, &mut fft)),
                ("division spectrale", deconvolve_with_reference(&y, s.signal(), fs, (20.0, 20_000.0), DEFAULT_REGULARIZATION, &mut fft)),
            ] {
                let lin = ir.linear(pre, len);
                let spec = fft.forward(&lin, len);
                let mut worst: f64 = 0.0;
                let mut worst_f = 0.0;
                for (k, c) in spec.iter().enumerate() {
                    let f = k as f64 * fs / len as f64;
                    if !(50.0..=15_000.0).contains(&f) {
                        continue;
                    }
                    let err = crate::db20(c.norm()) - crate::db20(bq.magnitude(f));
                    if err.abs() > worst {
                        worst = err.abs();
                        worst_f = f;
                    }
                }
                println!("{kind}, {methode} : écart max {worst:.4} dB à {worst_f:.0} Hz");
                assert!(worst < 0.1);
            }
        }
    }
}
