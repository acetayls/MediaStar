//! Sonomètre du micro : niveaux pondérés A, C et Z calculés sur chaque spectre du RTA
//! (un spectre par demi-bloc de 8192, fenêtre de Hann à 50 % : chaque échantillon compte
//! autant), constantes de temps Rapide (125 ms) et Lente (1 s), niveaux équivalents (Leq)
//! glissants sur 1 et 15 minutes et depuis la remise à zéro, maximum et crête.
//!
//! Tout est en dBFS : l'interface ajoute la calibration du micro pour lire des dB SPL.
//! Repères en France (décret n° 2017-1244, lieux diffusant des sons amplifiés) : 102 dB(A)
//! et 118 dB(C) en Leq sur 15 minutes.

use std::collections::VecDeque;

use star_measure::rta::Weighting;

/// Durée d'une case de l'historique (s).
pub const BIN_S: f64 = 1.0;
/// Historique gardé (s) : 15 minutes.
pub const HISTORY_S: f64 = 900.0;

/// État publié pour l'interface (dBFS, −200 pour le silence).
#[derive(Debug, Clone, Default)]
pub struct SplState {
    pub la_fast: f64,
    pub la_slow: f64,
    pub lc_fast: f64,
    pub lz_fast: f64,
    pub laeq_1m: f64,
    pub laeq_15m: f64,
    pub lceq_15m: f64,
    /// Leq depuis la remise à zéro, et sa durée (s).
    pub laeq_total: f64,
    pub lceq_total: f64,
    pub total_s: f64,
    /// Maximum de LA Rapide et crête de l'échantillon (Z) depuis la remise à zéro.
    pub la_max: f64,
    pub peak: f64,
    /// LAeq de chaque case d'environ une seconde, la plus ancienne d'abord (15 minutes au plus).
    pub history_a: Vec<f32>,
    /// Durée couverte par l'historique (s).
    pub history_s: f64,
}

/// Moyenne quadratique pondérée (puissance) d'un spectre de RTA, de 20 Hz à 20 kHz.
pub fn weighted_power(power: &[f64], bin_hz: f64, w: Weighting) -> f64 {
    power
        .iter()
        .enumerate()
        .filter(|(k, _)| (20.0..=20_000.0).contains(&(*k as f64 * bin_hz)))
        .map(|(k, &p)| {
            // `gain` est un gain en amplitude : au carré pour une puissance.
            let g = w.gain(k as f64 * bin_hz);
            p * g * g
        })
        .sum()
}

fn db(p: f64) -> f64 {
    if p > 1e-20 {
        10.0 * p.log10()
    } else {
        -200.0
    }
}

/// Énergie (puissance × durée) d'une seconde, en A et en C.
#[derive(Debug, Clone, Copy, Default)]
struct Bin {
    a: f64,
    c: f64,
    secs: f64,
}

pub struct SplMeter {
    /// Puissances lissées (Rapide, Lente).
    a_fast: f64,
    a_slow: f64,
    c_fast: f64,
    z_fast: f64,
    started: bool,
    current: Bin,
    bins: VecDeque<Bin>,
    /// Durée couverte par `bins` (s).
    bins_s: f64,
    total: Bin,
    a_max: f64,
    peak: f64,
}

impl Default for SplMeter {
    fn default() -> Self {
        Self { a_fast: 0.0, a_slow: 0.0, c_fast: 0.0, z_fast: 0.0, started: false, current: Bin::default(), bins: VecDeque::with_capacity(1024), bins_s: 0.0, total: Bin::default(), a_max: 0.0, peak: 0.0 }
    }
}

impl SplMeter {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    /// Ajoute un spectre couvrant `dt` secondes et la crête des échantillons de ce bloc.
    pub fn push(&mut self, power: &[f64], bin_hz: f64, dt: f64, peak: f32) {
        let a = weighted_power(power, bin_hz, Weighting::A);
        let c = weighted_power(power, bin_hz, Weighting::C);
        let z = weighted_power(power, bin_hz, Weighting::Z);
        if !self.started {
            (self.a_fast, self.a_slow, self.c_fast, self.z_fast) = (a, a, c, z);
            self.started = true;
        } else {
            let k = |tau: f64| 1.0 - (-dt / tau).exp();
            let (f, s) = (k(0.125), k(1.0));
            self.a_fast += f * (a - self.a_fast);
            self.a_slow += s * (a - self.a_slow);
            self.c_fast += f * (c - self.c_fast);
            self.z_fast += f * (z - self.z_fast);
        }
        self.a_max = self.a_max.max(self.a_fast);
        self.peak = self.peak.max(peak as f64);
        for b in [&mut self.current, &mut self.total] {
            b.a += a * dt;
            b.c += c * dt;
            b.secs += dt;
        }
        if self.current.secs >= BIN_S {
            self.bins_s += self.current.secs;
            self.bins.push_back(std::mem::take(&mut self.current));
            while self.bins.front().is_some_and(|b| self.bins_s - b.secs >= HISTORY_S) {
                let b = self.bins.pop_front().expect("case");
                self.bins_s -= b.secs;
            }
        }
    }

    /// Leq (puissance moyenne) des `window` dernières secondes (cases complètes).
    fn leq(&self, window: f64) -> (f64, f64) {
        let mut sum = Bin::default();
        for b in self.bins.iter().rev() {
            if sum.secs + b.secs / 2.0 > window {
                break;
            }
            sum.a += b.a;
            sum.c += b.c;
            sum.secs += b.secs;
        }
        if sum.secs <= 0.0 {
            return (-200.0, -200.0);
        }
        (db(sum.a / sum.secs), db(sum.c / sum.secs))
    }

    pub fn state(&self) -> SplState {
        let (laeq_1m, _) = self.leq(60.0);
        let (laeq_15m, lceq_15m) = self.leq(HISTORY_S);
        let t = &self.total;
        let (laeq_total, lceq_total) = if t.secs > 0.0 { (db(t.a / t.secs), db(t.c / t.secs)) } else { (-200.0, -200.0) };
        SplState {
            la_fast: db(self.a_fast),
            la_slow: db(self.a_slow),
            lc_fast: db(self.c_fast),
            lz_fast: db(self.z_fast),
            laeq_1m,
            laeq_15m,
            lceq_15m,
            laeq_total,
            lceq_total,
            total_s: t.secs,
            la_max: db(self.a_max),
            peak: if self.peak > 0.0 { 20.0 * self.peak.log10() } else { -200.0 },
            history_a: self.bins.iter().map(|b| db(b.a / b.secs.max(1e-9)) as f32).collect(),
            history_s: self.bins_s,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spectre plat : la bande de 1 kHz porte toute la puissance (A = C = Z à 1 kHz).
    fn tone(level_db: f64) -> Vec<f64> {
        let mut p = vec![0.0; 4097];
        p[171] = 10f64.powf(level_db / 10.0); // 171 × 5,86 Hz ≈ 1002 Hz
        p
    }

    #[test]
    fn leq_et_maximum() {
        let mut m = SplMeter::default();
        let bin = 48_000.0 / 8192.0;
        let dt = 4096.0 / 48_000.0;
        // 10 minutes à −30 dBFS puis 5 minutes à −20 : Leq 15 min = 10·log10((10·10⁻³ + 5·10⁻²)/15).
        for _ in 0..(600.0 / dt) as usize {
            m.push(&tone(-30.0), bin, dt, 0.05);
        }
        let s = m.state();
        assert!((s.la_slow + 30.0).abs() < 0.1 && (s.laeq_1m + 30.0).abs() < 0.1, "{s:?}");
        for _ in 0..(300.0 / dt) as usize {
            m.push(&tone(-20.0), bin, dt, 0.2);
        }
        let s = m.state();
        let attendu = 10.0 * ((10.0 * 1e-3 + 5.0 * 1e-2) / 15.0f64).log10();
        assert!((s.laeq_15m - attendu).abs() < 0.2, "{} {attendu}", s.laeq_15m);
        assert!((s.laeq_1m + 20.0).abs() < 0.1);
        assert!((s.la_max + 20.0).abs() < 0.1);
        assert!((s.peak - 20.0 * 0.2f64.log10()).abs() < 1e-6);
        assert!((850..=905).contains(&s.history_a.len()), "{}", s.history_a.len());
        m.reset();
        assert_eq!(m.state().total_s, 0.0);
    }
}
