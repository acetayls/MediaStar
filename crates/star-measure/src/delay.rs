//! Délai : pic d'une réponse impulsionnelle, intercorrélation GCC-PHAT, conversions.

use num_complex::Complex64;

use crate::fft::{next_pow2, Fft};
use crate::to_f64;

/// Indice de la plus grande valeur absolue.
pub fn peak_index(x: &[f64]) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, v) in x.iter().enumerate() {
        let a = v.abs();
        if best.is_none_or(|(_, b)| a > b) {
            best = Some((i, a));
        }
    }
    best.map(|(i, _)| i)
}

/// Décalage (−0,5 à 0,5) du sommet de la parabole passant par trois points.
pub fn parabolic(prev: f64, center: f64, next: f64) -> f64 {
    let den = prev - 2.0 * center + next;
    if den.abs() < f64::MIN_POSITIVE {
        0.0
    } else {
        (0.5 * (prev - next) / den).clamp(-0.5, 0.5)
    }
}

/// Position fractionnaire du pic de |x| (interpolation parabolique sur les valeurs absolues).
pub fn peak_position(x: &[f64]) -> Option<f64> {
    let i = peak_index(x)?;
    if i == 0 || i + 1 >= x.len() {
        return Some(i as f64);
    }
    Some(i as f64 + parabolic(x[i - 1].abs(), x[i].abs(), x[i + 1].abs()))
}

/// Délai (échantillons, fractionnaire) de `measured` par rapport à `reference`, par
/// intercorrélation à transformée de phase (GCC-PHAT) : l'interspectre est ramené à un
/// module unité, ce qui rend le pic du direct très étroit, même avec de la réverbération
/// ou un signal coloré. Positif quand `measured` est en retard.
///
/// `max_lag` borne la recherche (en échantillons, dans les deux sens).
pub fn gcc_phat(reference: &[f32], measured: &[f32], max_lag: Option<usize>, fft: &mut Fft) -> Option<f64> {
    if reference.is_empty() || measured.is_empty() {
        return None;
    }
    let n = next_pow2(reference.len() + measured.len());
    let x = fft.forward(&to_f64(reference), n);
    let y = fft.forward(&to_f64(measured), n);
    let max = x.iter().zip(&y).map(|(a, b)| (a.conj() * b).norm()).fold(0.0, f64::max);
    if max <= 0.0 {
        return None;
    }
    let floor = max * 1e-12;
    let g: Vec<Complex64> = x
        .iter()
        .zip(&y)
        .map(|(a, b)| {
            let c = a.conj() * b;
            let m = c.norm();
            if m > floor {
                c / m
            } else {
                Complex64::new(0.0, 0.0)
            }
        })
        .collect();
    let cc = fft.inverse(&g, n);
    let lag = max_lag.unwrap_or(n / 2 - 1).min(n / 2 - 1) as isize;
    // Corrélation réordonnée de −lag à +lag, pour interpoler sans se soucier du repli.
    let ordered: Vec<f64> = (-lag..=lag).map(|l| cc[(l.rem_euclid(n as isize)) as usize]).collect();
    // Le pic est positif (corrélation) : on cherche le maximum signé.
    let (i, _) = ordered.iter().enumerate().fold((0, f64::MIN), |b, (i, &v)| if v > b.1 { (i, v) } else { b });
    let frac = if i > 0 && i + 1 < ordered.len() { parabolic(ordered[i - 1], ordered[i], ordered[i + 1]) } else { 0.0 };
    Some(i as f64 - lag as f64 + frac)
}

/// Vitesse du son (m/s) dans l'air à `temp_c` °C : c = 331,3·√(1 + T/273,15).
pub fn speed_of_sound(temp_c: f64) -> f64 {
    331.3 * (1.0 + temp_c / 273.15).sqrt()
}

pub fn ms_to_m(ms: f64, temp_c: f64) -> f64 {
    ms / 1000.0 * speed_of_sound(temp_c)
}

pub fn m_to_ms(m: f64, temp_c: f64) -> f64 {
    m / speed_of_sound(temp_c) * 1000.0
}

pub fn samples_to_ms(samples: f64, sample_rate: f64) -> f64 {
    samples * 1000.0 / sample_rate
}

pub fn ms_to_samples(ms: f64, sample_rate: f64) -> f64 {
    ms * sample_rate / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{Generator, WhiteNoise};
    use crate::test_util::fractional_delay;

    #[test]
    fn conversions() {
        assert!((speed_of_sound(20.0) - 343.2).abs() < 0.1);
        assert!((ms_to_m(10.0, 20.0) - 3.432).abs() < 0.001);
        assert!((m_to_ms(ms_to_m(7.5, 12.0), 12.0) - 7.5).abs() < 1e-12);
        assert_eq!(ms_to_samples(10.0, 48_000.0), 480.0);
    }

    #[test]
    fn pic_parabolique() {
        // Parabole de sommet 10,3.
        let x: Vec<f64> = (0..20).map(|i| 1000.0 - (i as f64 - 10.3).powi(2)).collect();
        assert!((peak_position(&x).unwrap() - 10.3).abs() < 1e-9);
    }

    /// GCC-PHAT retrouve le délai du direct malgré une réverbération synthétique
    /// (réflexion forte à +3 ms et queue diffuse décroissante).
    #[test]
    fn gcc_phat_reverberation() {
        let fs = 48_000.0;
        let mut fft = Fft::new();
        let mut noise = WhiteNoise::new(0.5, 1);
        let mut x = vec![0.0f32; 48_000];
        noise.fill(&mut x);

        let mut tail = WhiteNoise::new(1.0, 7);
        for delay_ms in [7.0, 12.34567] {
            let d = delay_ms * fs / 1000.0;
            // Réponse de la salle (après le direct) : réflexion −3 dB à 3 ms, queue de RT60 ≈ 0,6 s.
            let len = 24_000;
            let mut room = vec![0.0f32; len];
            tail.fill(&mut room);
            let mut room: Vec<f64> = room.iter().enumerate().map(|(i, &v)| v as f64 * 0.3 * (-6.9 * i as f64 / (0.6 * fs)).exp()).collect();
            room[..(0.001 * fs) as usize].iter_mut().for_each(|v| *v = 0.0);
            room[0] = 1.0;
            room[(0.003 * fs) as usize] += 0.7;
            let wet = fft.convolve(&crate::to_f64(&x), &room);
            let y = fractional_delay(&wet, d, 0, &mut fft);
            let y: Vec<f32> = y.iter().map(|&v| v as f32).collect();
            let got = gcc_phat(&x, &y, Some(4800), &mut fft).unwrap();
            let got_ms = samples_to_ms(got, fs);
            println!("GCC-PHAT : attendu {delay_ms:.5} ms, trouvé {got_ms:.5} ms");
            assert!((got_ms - delay_ms).abs() < 0.01);
        }
    }
}
