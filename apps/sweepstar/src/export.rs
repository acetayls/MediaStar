//! Exports : réponse impulsionnelle en WAV (flottant 32 bits, mono) et réponse en fréquence
//! en texte au format d'import de REW (lignes « fréquence magnitude phase », en-têtes `*`).

use std::io::Write;
use std::path::Path;

/// Écrit `samples` en WAV mono flottant 32 bits.
pub fn write_wav_f32(path: &Path, sample_rate: u32, samples: &[f32]) -> anyhow::Result<()> {
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(&wav_bytes(sample_rate, samples))?;
    f.flush()?;
    Ok(())
}

pub fn wav_bytes(sample_rate: u32, samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 4) as u32;
    let mut b = Vec::with_capacity(58 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(50 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVE");
    // fmt : format 3 (IEEE float), 1 canal, avec extension vide (18 octets).
    b.extend_from_slice(b"fmt ");
    b.extend_from_slice(&18u32.to_le_bytes());
    b.extend_from_slice(&3u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&sample_rate.to_le_bytes());
    b.extend_from_slice(&(sample_rate * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&32u16.to_le_bytes());
    b.extend_from_slice(&0u16.to_le_bytes());
    // fact : nombre d'échantillons (exigé pour un format non PCM).
    b.extend_from_slice(b"fact");
    b.extend_from_slice(&4u32.to_le_bytes());
    b.extend_from_slice(&(samples.len() as u32).to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&s.to_le_bytes());
    }
    b
}

/// Texte REW : une ligne par point (Hz, dB, degrés), points de `points`.
pub fn rew_text(name: &str, points: &[(f64, f64, f64)]) -> String {
    let mut s = String::new();
    s.push_str("* Measurement data measured by SweepStar\n");
    s.push_str(&format!("* Source: {name}\n"));
    s.push_str(&format!("* Created: {}\n", star_core::time::now(false)));
    s.push_str("* Freq(Hz) SPL(dB) Phase(degrees)\n");
    for &(f, db, ph) in points {
        s.push_str(&format!("{f:.6} {db:.3} {ph:.3}\n"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entete_wav() {
        let b = wav_bytes(48_000, &[0.0, 0.5, -1.0]);
        assert_eq!(&b[0..4], b"RIFF");
        assert_eq!(u32::from_le_bytes(b[4..8].try_into().unwrap()) as usize, b.len() - 8);
        assert_eq!(u16::from_le_bytes(b[20..22].try_into().unwrap()), 3);
        assert_eq!(u32::from_le_bytes(b[24..28].try_into().unwrap()), 48_000);
        let data = b.windows(4).position(|w| w == b"data").unwrap();
        assert_eq!(u32::from_le_bytes(b[data + 4..data + 8].try_into().unwrap()), 12);
        assert_eq!(f32::from_le_bytes(b[data + 12..data + 16].try_into().unwrap()), 0.5);
    }

    #[test]
    fn texte_rew() {
        let t = rew_text("essai", &[(20.0, -3.5, 12.25), (1000.0, 0.0, -90.0)]);
        let lines: Vec<&str> = t.lines().filter(|l| !l.starts_with('*')).collect();
        assert_eq!(lines, ["20.000000 -3.500 12.250", "1000.000000 0.000 -90.000"]);
    }
}
