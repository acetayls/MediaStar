//! Outils des tests : biquads RBJ (mêmes formules que star_core::eq), retard fractionnaire.

use std::f64::consts::PI;

use num_complex::Complex64;

use crate::fft::{next_pow2, Fft};

#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    fs: f64,
}

impl Biquad {
    fn normalized(fs: f64, b: [f64; 3], a: [f64; 3]) -> Self {
        Self { b: [b[0] / a[0], b[1] / a[0], b[2] / a[0]], a: [a[1] / a[0], a[2] / a[0]], fs }
    }

    pub fn peaking(fs: f64, f0: f64, q: f64, gain_db: f64) -> Self {
        let a = 10f64.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * f0 / fs;
        let alpha = w0.sin() / (2.0 * q);
        let c = w0.cos();
        Self::normalized(fs, [1.0 + alpha * a, -2.0 * c, 1.0 - alpha * a], [1.0 + alpha / a, -2.0 * c, 1.0 - alpha / a])
    }

    pub fn high_shelf(fs: f64, f0: f64, q: f64, gain_db: f64) -> Self {
        let a = 10f64.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * f0 / fs;
        let alpha = w0.sin() / (2.0 * q);
        let c = w0.cos();
        let sa = 2.0 * a.sqrt() * alpha;
        Self::normalized(
            fs,
            [a * ((a + 1.0) + (a - 1.0) * c + sa), -2.0 * a * ((a - 1.0) + (a + 1.0) * c), a * ((a + 1.0) + (a - 1.0) * c - sa)],
            [(a + 1.0) - (a - 1.0) * c + sa, 2.0 * ((a - 1.0) - (a + 1.0) * c), (a + 1.0) - (a - 1.0) * c - sa],
        )
    }

    pub fn filter(&self, x: &[f64]) -> Vec<f64> {
        let (mut x1, mut x2, mut y1, mut y2) = (0.0, 0.0, 0.0, 0.0);
        x.iter()
            .map(|&x0| {
                let y0 = self.b[0] * x0 + self.b[1] * x1 + self.b[2] * x2 - self.a[0] * y1 - self.a[1] * y2;
                x2 = x1;
                x1 = x0;
                y2 = y1;
                y1 = y0;
                y0
            })
            .collect()
    }

    pub fn magnitude(&self, f: f64) -> f64 {
        let z1 = Complex64::from_polar(1.0, -2.0 * PI * f / self.fs);
        let z2 = z1 * z1;
        let num = self.b[0] + z1 * self.b[1] + z2 * self.b[2];
        let den = 1.0 + z1 * self.a[0] + z2 * self.a[1];
        (num / den).norm()
    }
}

/// Retarde `x` de `d` échantillons (fractionnaire) par déphasage dans le domaine fréquentiel ;
/// la sortie fait `x.len() + extra` échantillons.
pub fn fractional_delay(x: &[f64], d: f64, extra: usize, fft: &mut Fft) -> Vec<f64> {
    let out_len = x.len() + extra;
    let n = next_pow2(out_len + d.ceil() as usize + 4096);
    let mut spec = fft.forward(x, n);
    for (k, c) in spec.iter_mut().enumerate() {
        *c *= Complex64::from_polar(1.0, -2.0 * PI * k as f64 * d / n as f64);
    }
    let mut y = fft.inverse(&spec, n);
    y.truncate(out_len);
    y
}
