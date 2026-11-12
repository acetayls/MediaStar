//! Lissage fractionnaire d'octave d'une courbe sur une grille de fréquences régulière (FFT).
//!
//! Chaque point est la moyenne de la puissance sur [f·2^(−1/2b), f·2^(1/2b)]. La moyenne se
//! fait sur la courbe constante par case (case `k` = [k − ½, k + ½]·bin_hz) grâce à une
//! somme cumulée : le calcul reste en O(n) et les bords fractionnaires sont pris au prorata,
//! ce qui évite les marches dans le grave où la fenêtre est plus étroite qu'une case.

/// Lisse une courbe de puissance (|H|², densité…) en 1/`fraction` d'octave.
pub fn smooth_power(power: &[f64], bin_hz: f64, fraction: f64) -> Vec<f64> {
    let n = power.len();
    if n == 0 || fraction <= 0.0 {
        return power.to_vec();
    }
    let mut cum = Vec::with_capacity(n + 1);
    cum.push(0.0);
    let mut s = 0.0;
    for &p in power {
        s += p;
        cum.push(s);
    }
    // Intégrale de −½ jusqu'à x (en cases).
    let integral = |x: f64| {
        let x = (x + 0.5).clamp(0.0, n as f64);
        let i = (x.floor() as usize).min(n - 1);
        cum[i] + power[i] * (x - i as f64)
    };
    let half = 2f64.powf(0.5 / fraction);
    (0..n)
        .map(|k| {
            if k == 0 {
                return power[0];
            }
            let f = k as f64 * bin_hz;
            let a = (f / half / bin_hz).max(-0.5);
            let b = (f * half / bin_hz).min(n as f64 - 0.5);
            if b - a < 1e-12 {
                power[k]
            } else {
                (integral(b) - integral(a)) / (b - a)
            }
        })
        .collect()
}

/// Lisse une courbe de magnitude en dB (passage par la puissance).
pub fn smooth_db(db: &[f64], bin_hz: f64, fraction: f64) -> Vec<f64> {
    let p: Vec<f64> = db.iter().map(|&d| 10f64.powf(d / 10.0)).collect();
    smooth_power(&p, bin_hz, fraction).into_iter().map(crate::db10).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn courbe_plate() {
        let p = vec![2.0; 1000];
        assert!(smooth_power(&p, 10.0, 3.0).iter().all(|&v| (v - 2.0).abs() < 1e-12));
    }

    /// Filtre en peigne |1 + e^(−jωτ)|² (τ = 1 ms) : lissé en 1/3 d'octave, il tend vers sa
    /// puissance moyenne, 2 (+3 dB), dès que la fenêtre couvre plusieurs dents.
    #[test]
    fn peigne_lisse() {
        let bin_hz = 48_000.0 / 65_536.0;
        let tau = 0.001;
        let p: Vec<f64> = (0..32_769).map(|k| {
            let w = 2.0 * std::f64::consts::PI * k as f64 * bin_hz * tau;
            2.0 + 2.0 * w.cos()
        }).collect();
        let s = smooth_power(&p, bin_hz, 3.0);
        for f in [4000.0, 8000.0, 16_000.0] {
            let k = (f / bin_hz) as usize;
            let db = crate::db10(s[k]);
            println!("peigne lissé à {f} Hz : {db:.3} dB");
            assert!((db - 3.01).abs() < 0.5);
        }
        // Sans lissage, les creux restent.
        assert!(p.iter().cloned().fold(f64::MAX, f64::min) < 1e-3);
    }
}
