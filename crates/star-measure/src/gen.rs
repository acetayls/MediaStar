//! Générateurs de signaux de mesure : bruit blanc, bruit rose, bruit rose périodique, sinus.
//!
//! Chaque générateur garde son état ; `fill` n'alloue rien et ne bloque pas, il peut donc
//! tourner dans le fil audio. Les tables (bruit périodique) se préparent à la construction.

use std::f64::consts::PI;

use num_complex::Complex64;

use crate::fft::Fft;

pub trait Generator {
    /// Remplit `out` avec les échantillons suivants.
    fn fill(&mut self, out: &mut [f32]);
}

/// Générateur pseudo-aléatoire xorshift64*, reproductible à partir d'une graine.
#[derive(Debug, Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        // Mélange de la graine (splitmix64) : une graine nulle ou petite reste valable.
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Self((z ^ (z >> 31)) | 1)
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniforme dans [−1, 1).
    pub fn uniform(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    }
}

/// Bruit blanc uniforme d'amplitude crête `amplitude`.
#[derive(Debug, Clone)]
pub struct WhiteNoise {
    rng: Rng,
    pub amplitude: f32,
}

impl WhiteNoise {
    pub fn new(amplitude: f32, seed: u64) -> Self {
        Self { rng: Rng::new(seed), amplitude }
    }
}

impl Generator for WhiteNoise {
    fn fill(&mut self, out: &mut [f32]) {
        for v in out {
            *v = self.rng.uniform() as f32 * self.amplitude;
        }
    }
}

/// Bruit rose (−3 dB/octave) par le filtre de Paul Kellet (version « raffinée », à ±0,05 dB
/// au-dessus de 9 Hz). Valeur efficace ≈ 0,1·`amplitude`, crête rarement au-delà.
#[derive(Debug, Clone)]
pub struct PinkNoise {
    rng: Rng,
    b: [f64; 7],
    pub amplitude: f32,
}

impl PinkNoise {
    pub fn new(amplitude: f32, seed: u64) -> Self {
        Self { rng: Rng::new(seed), b: [0.0; 7], amplitude }
    }

    fn next(&mut self) -> f64 {
        let w = self.rng.uniform();
        let b = &mut self.b;
        b[0] = 0.99886 * b[0] + w * 0.0555179;
        b[1] = 0.99332 * b[1] + w * 0.0750759;
        b[2] = 0.96900 * b[2] + w * 0.1538520;
        b[3] = 0.86650 * b[3] + w * 0.3104856;
        b[4] = 0.55000 * b[4] + w * 0.5329522;
        b[5] = -0.7616 * b[5] - w * 0.0168980;
        let pink = b[0] + b[1] + b[2] + b[3] + b[4] + b[5] + b[6] + w * 0.5362;
        b[6] = w * 0.115926;
        pink * 0.11
    }
}

impl Generator for PinkNoise {
    fn fill(&mut self, out: &mut [f32]) {
        for v in out {
            *v = (self.next() * self.amplitude as f64) as f32;
        }
    }
}

/// Bruit rose périodique : une table de `len` échantillons (longueur de la FFT d'analyse)
/// dont le spectre a exactement une pente de −3 dB/octave entre `f_low` et `f_high`, avec
/// des phases aléatoires. Analysé avec une FFT de même longueur et une fenêtre
/// rectangulaire, il ne donne aucune fuite spectrale. La table est jouée en boucle.
#[derive(Debug, Clone)]
pub struct PeriodicPink {
    table: Vec<f32>,
    pos: usize,
    pub amplitude: f32,
}

impl PeriodicPink {
    /// Table normalisée à une crête de 1 ; `amplitude` règle la crête jouée.
    pub fn new(len: usize, sample_rate: f64, f_low: f64, f_high: f64, amplitude: f32, seed: u64, fft: &mut Fft) -> Self {
        assert!(len >= 4 && len.is_multiple_of(2), "longueur paire requise");
        let mut rng = Rng::new(seed);
        let bins = len / 2 + 1;
        let spec: Vec<Complex64> = (0..bins)
            .map(|k| {
                let f = k as f64 * sample_rate / len as f64;
                if k == 0 || k == bins - 1 || f < f_low || f > f_high {
                    Complex64::new(0.0, 0.0)
                } else {
                    let phase = PI * rng.uniform();
                    Complex64::from_polar(1.0 / f.sqrt(), phase)
                }
            })
            .collect();
        let x = fft.inverse(&spec, len);
        let peak = x.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(f64::MIN_POSITIVE);
        let table = x.iter().map(|&v| (v / peak) as f32).collect();
        Self { table, pos: 0, amplitude }
    }

    pub fn len(&self) -> usize {
        self.table.len()
    }

    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }

    /// Valeur efficace de la table (pour une amplitude de 1).
    pub fn rms(&self) -> f64 {
        (self.table.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / self.table.len() as f64).sqrt()
    }

    /// Revient au début de la table.
    pub fn restart(&mut self) {
        self.pos = 0;
    }
}

impl Generator for PeriodicPink {
    fn fill(&mut self, out: &mut [f32]) {
        for v in out {
            *v = self.table[self.pos] * self.amplitude;
            self.pos += 1;
            if self.pos == self.table.len() {
                self.pos = 0;
            }
        }
    }
}

/// Sinus à phase continue (la fréquence peut changer sans clic).
#[derive(Debug, Clone)]
pub struct Sine {
    phase: f64,
    inc: f64,
    /// Incrément visé : `inc` le rejoint en douceur (glissement de fréquence).
    target_inc: f64,
    /// Part de l'écart rattrapée à chaque échantillon (1 : saut immédiat).
    glide: f64,
    sample_rate: f64,
    pub amplitude: f32,
}

impl Sine {
    pub fn new(freq: f64, sample_rate: f64, amplitude: f32) -> Self {
        let inc = freq / sample_rate;
        Self { phase: 0.0, inc, target_inc: inc, glide: 1.0, sample_rate, amplitude }
    }

    /// Change la fréquence d'un coup (la phase reste continue).
    pub fn set_frequency(&mut self, freq: f64) {
        self.inc = freq / self.sample_rate;
        self.target_inc = self.inc;
    }

    /// Glisse vers `freq` avec une constante de temps `secs` (phase continue, sans clic).
    pub fn glide_to(&mut self, freq: f64, secs: f64) {
        self.target_inc = freq / self.sample_rate;
        let n = (secs * self.sample_rate).max(1.0);
        self.glide = 1.0 - (-1.0 / n).exp();
    }

    pub fn frequency(&self) -> f64 {
        self.inc * self.sample_rate
    }
}

impl Generator for Sine {
    fn fill(&mut self, out: &mut [f32]) {
        for v in out {
            *v = ((2.0 * PI * self.phase).sin() * self.amplitude as f64) as f32;
            if self.inc != self.target_inc {
                self.inc += (self.target_inc - self.inc) * self.glide;
                if (self.inc - self.target_inc).abs() < 1e-12 {
                    self.inc = self.target_inc;
                }
            }
            self.phase += self.inc;
            self.phase -= self.phase.floor();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fft::Window;
    use crate::rta::{band_powers, fractional_octave_bands, welch_power};

    /// Niveaux par octave (dB) de 63 Hz à 8 kHz, relatifs à la bande de 1 kHz.
    fn octaves_relatives(x: &[f32], fft_len: usize, window: Window, overlap: f64) -> Vec<(f64, f64)> {
        let mut fft = Fft::new();
        let p = welch_power(x, fft_len, overlap, window, &mut fft);
        let bands = fractional_octave_bands(1, 60.0, 8500.0);
        let lv = band_powers(&p, 48_000.0 / fft_len as f64, &bands);
        let ref_i = bands.iter().position(|b| (b.center - 1000.0).abs() < 1.0).unwrap();
        bands.iter().zip(&lv).map(|(b, &l)| (b.center, crate::db10(l) - crate::db10(lv[ref_i]))).collect()
    }

    /// Bruit rose : puissance égale par octave, c'est-à-dire une densité de −3 dB/octave.
    #[test]
    fn bruit_rose_pente() {
        let mut g = PinkNoise::new(1.0, 42);
        let mut x = vec![0.0f32; 1 << 21];
        g.fill(&mut x);
        let rms = (x.iter().map(|&v| (v as f64).powi(2)).sum::<f64>() / x.len() as f64).sqrt();
        println!("bruit rose : valeur efficace {rms:.4}");
        for (f, l) in octaves_relatives(&x, 8192, Window::Hann, 0.5) {
            println!("  octave {f:7.1} Hz : {l:+.3} dB");
            assert!(l.abs() < 1.0);
        }
    }

    #[test]
    fn bruit_rose_periodique() {
        let mut fft = Fft::new();
        let len = 65_536;
        let mut g = PeriodicPink::new(len, 48_000.0, 20.0, 20_000.0, 1.0, 3, &mut fft);
        let mut x = vec![0.0f32; len * 4];
        g.fill(&mut x);
        assert_eq!(x[0], x[len]);
        for (f, l) in octaves_relatives(&x, len, Window::Rectangular, 0.0) {
            println!("  rose périodique, octave {f:7.1} Hz : {l:+.3} dB");
            assert!(l.abs() < 0.2);
        }
    }

    #[test]
    fn sinus() {
        let mut s = Sine::new(1000.0, 48_000.0, 0.5);
        let mut x = vec![0.0f32; 48];
        s.fill(&mut x);
        assert!(x[0].abs() < 1e-6);
        assert!((x[12] - 0.5).abs() < 1e-6);
    }

    /// Glissement : aucun saut d'un échantillon à l'autre plus grand que la pente maximale.
    #[test]
    fn sinus_glisse_sans_saut() {
        let mut s = Sine::new(1000.0, 48_000.0, 1.0);
        let mut x = vec![0.0f32; 4800];
        s.fill(&mut x[..1000]);
        s.glide_to(2000.0, 0.01);
        s.fill(&mut x[1000..]);
        let pente = 2.0 * PI * 2000.0 / 48_000.0;
        assert!(x.windows(2).all(|w| ((w[1] - w[0]).abs() as f64) <= pente * 1.01));
        assert!((s.frequency() - 2000.0).abs() < 1.0);
    }
}
