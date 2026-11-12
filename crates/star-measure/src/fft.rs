//! FFT réelles (realfft) et fenêtres d'analyse.
//!
//! [`Fft`] garde un planificateur : chaque taille n'est planifiée qu'une fois, les appels
//! suivants réutilisent le plan. Le spectre d'un signal de `n` points a `n/2 + 1` cases
//! (de 0 à la fréquence de Nyquist) ; l'inverse est normalisé (aller-retour = identité).

use std::f64::consts::PI;
use std::sync::Arc;

use num_complex::Complex64;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};

pub struct Fft {
    planner: RealFftPlanner<f64>,
}

impl Default for Fft {
    fn default() -> Self {
        Self::new()
    }
}

impl Fft {
    pub fn new() -> Self {
        Self { planner: RealFftPlanner::new() }
    }

    /// Plan direct de taille `n`, pour les traitements par blocs qui gardent leurs tampons.
    pub fn forward_plan(&mut self, n: usize) -> Arc<dyn RealToComplex<f64>> {
        self.planner.plan_fft_forward(n)
    }

    pub fn inverse_plan(&mut self, n: usize) -> Arc<dyn ComplexToReal<f64>> {
        self.planner.plan_fft_inverse(n)
    }

    /// Spectre de `signal` complété par des zéros (ou tronqué) à `n` points.
    pub fn forward(&mut self, signal: &[f64], n: usize) -> Vec<Complex64> {
        let plan = self.planner.plan_fft_forward(n);
        let mut input = vec![0.0; n];
        let m = signal.len().min(n);
        input[..m].copy_from_slice(&signal[..m]);
        let mut out = plan.make_output_vec();
        plan.process(&mut input, &mut out).expect("tailles cohérentes");
        out
    }

    /// Signal réel de `n` points dont le spectre est `spectrum` (`n/2 + 1` cases).
    /// Les parties imaginaires du continu et de Nyquist sont ignorées.
    pub fn inverse(&mut self, spectrum: &[Complex64], n: usize) -> Vec<f64> {
        assert_eq!(spectrum.len(), n / 2 + 1, "spectre de taille incohérente");
        let plan = self.planner.plan_fft_inverse(n);
        let mut spec = spectrum.to_vec();
        spec[0].im = 0.0;
        if n.is_multiple_of(2) {
            spec[n / 2].im = 0.0;
        }
        let mut out = vec![0.0; n];
        plan.process(&mut spec, &mut out).expect("tailles cohérentes");
        let k = 1.0 / n as f64;
        out.iter_mut().for_each(|v| *v *= k);
        out
    }

    /// Convolution linéaire de deux signaux (par FFT).
    pub fn convolve(&mut self, a: &[f64], b: &[f64]) -> Vec<f64> {
        if a.is_empty() || b.is_empty() {
            return Vec::new();
        }
        let len = a.len() + b.len() - 1;
        let n = next_pow2(len);
        let fa = self.forward(a, n);
        let fb = self.forward(b, n);
        let prod: Vec<Complex64> = fa.iter().zip(&fb).map(|(x, y)| x * y).collect();
        let mut out = self.inverse(&prod, n);
        out.truncate(len);
        out
    }
}

/// Plus petite puissance de deux ≥ `n`.
pub fn next_pow2(n: usize) -> usize {
    n.max(1).next_power_of_two()
}

/// Fréquence (Hz) de la case `k` d'une FFT de `n` points.
pub fn bin_freq(k: usize, n: usize, sample_rate: f64) -> f64 {
    k as f64 * sample_rate / n as f64
}

/// Fenêtre d'analyse.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Window {
    Rectangular,
    /// Hann périodique (adaptée aux FFT avec recouvrement).
    Hann,
    /// Tukey : plateau, bords en demi-Hann ; le paramètre (0 à 1) est la part tapérée.
    Tukey(f64),
}

impl Window {
    pub fn coefficients(self, n: usize) -> Vec<f64> {
        match self {
            Window::Rectangular => vec![1.0; n],
            Window::Hann => hann(n),
            Window::Tukey(alpha) => tukey(n, alpha),
        }
    }
}

/// Hann périodique de `n` points.
pub fn hann(n: usize) -> Vec<f64> {
    (0..n).map(|i| 0.5 - 0.5 * (2.0 * PI * i as f64 / n as f64).cos()).collect()
}

/// Tukey symétrique de `n` points ; `alpha` = part de la fenêtre dans les deux bords.
pub fn tukey(n: usize, alpha: f64) -> Vec<f64> {
    let alpha = alpha.clamp(0.0, 1.0);
    if n < 2 || alpha == 0.0 {
        return vec![1.0; n];
    }
    let m = (n - 1) as f64;
    let edge = alpha * m / 2.0;
    (0..n)
        .map(|i| {
            let x = i as f64;
            let d = x.min(m - x);
            if d >= edge {
                1.0
            } else {
                0.5 - 0.5 * (PI * d / edge).cos()
            }
        })
        .collect()
}

/// Demi-Hann montant sur `fade_in` échantillons au début, descendant sur `fade_out` à la fin.
pub fn apply_fades(signal: &mut [f64], fade_in: usize, fade_out: usize) {
    let n = signal.len();
    let fi = fade_in.min(n);
    for (i, v) in signal.iter_mut().take(fi).enumerate() {
        *v *= 0.5 - 0.5 * (PI * i as f64 / fi as f64).cos();
    }
    let fo = fade_out.min(n);
    for i in 0..fo {
        signal[n - 1 - i] *= 0.5 - 0.5 * (PI * i as f64 / fo as f64).cos();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aller_retour_identite() {
        let mut fft = Fft::new();
        let x: Vec<f64> = (0..1000).map(|i| ((i * 7919) % 101) as f64 - 50.0).collect();
        let spec = fft.forward(&x, 1024);
        let y = fft.inverse(&spec, 1024);
        for i in 0..1000 {
            assert!((x[i] - y[i]).abs() < 1e-9);
        }
        assert!(y[1000..].iter().all(|v| v.abs() < 1e-9));
    }

    #[test]
    fn convolution() {
        let mut fft = Fft::new();
        let y = fft.convolve(&[1.0, 2.0, 3.0], &[0.0, 1.0, 0.5]);
        let attendu = [0.0, 1.0, 2.5, 4.0, 1.5];
        for (a, b) in y.iter().zip(attendu) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn fenetres() {
        let w = hann(8);
        assert_eq!(w[0], 0.0);
        assert!((w[4] - 1.0).abs() < 1e-12);
        let t = tukey(101, 0.5);
        assert_eq!(t[0], 0.0);
        assert_eq!(t[50], 1.0);
        assert!((t[10] - t[90]).abs() < 1e-12);
    }
}
