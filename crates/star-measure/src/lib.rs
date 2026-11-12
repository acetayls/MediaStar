//! Moteur de mesure acoustique de SweepStar : calculs purs, sans interface ni audio.
//!
//! - [`sweep`] : sweep exponentiel synchronisé (Farina / Novak), filtre inverse,
//!   déconvolution par le filtre inverse ou par division spectrale avec une référence captée.
//! - [`ir`] : réponse impulsionnelle, séparation des harmoniques, ETC, fenêtrage du direct.
//! - [`delay`] : délai par le pic d'IR ou par intercorrélation GCC-PHAT, conversion ms ↔ m.
//! - [`transfer`] : fonction de transfert bi-canal en direct (H1, phase, cohérence).
//! - [`rta`] : bandes fractionnaires d'octave, pondérations A/C/Z, spectre moyenné.
//! - [`smooth`] : lissage fractionnaire d'octave.
//! - [`gen`] : bruits blanc et rose, bruit rose périodique, sinus, sans allocation dans `fill`.
//!
//! Les signaux arrivent et repartent en `f32` (format des cartes son) ; les calculs se font
//! en `f64`, pour garder la précision sur les délais et les faibles niveaux.

pub mod delay;
pub mod fft;
pub mod gen;
pub mod ir;
pub mod rta;
pub mod smooth;
pub mod sweep;
pub mod transfer;

pub use num_complex::Complex64;

/// Plancher des niveaux en dB, pour un signal nul.
pub const FLOOR_DB: f64 = -200.0;

/// Amplitude → dB (20·log10), avec plancher.
pub fn db20(x: f64) -> f64 {
    if x > 0.0 {
        (20.0 * x.log10()).max(FLOOR_DB)
    } else {
        FLOOR_DB
    }
}

/// Puissance → dB (10·log10), avec plancher.
pub fn db10(x: f64) -> f64 {
    if x > 0.0 {
        (10.0 * x.log10()).max(FLOOR_DB)
    } else {
        FLOOR_DB
    }
}

pub(crate) fn to_f64(x: &[f32]) -> Vec<f64> {
    x.iter().map(|&v| v as f64).collect()
}

#[cfg(test)]
pub(crate) mod test_util;
