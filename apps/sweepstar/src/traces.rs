//! Mesures enregistrées (traces), à la manière d'un analyseur : chaque capture (sweep ou
//! RTA) est écrite tout de suite dans le dossier des mesures, la liste est relue au
//! lancement, et les données ne sont chargées qu'à l'affichage.
//!
//! Format `.ssm` : ligne magique, longueur (u32 LE) et en-tête JSON, puis les tableaux en
//! petit-boutiste : sweep = H (ré, im en f32), IR (f32), ETC (f32) ; RTA = puissance (f64) ;
//! transfert = H (ré, im en f32), cohérence (f32).

use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use star_measure::Complex64;

use crate::measure::{Measurement, Method};

pub const EXTENSION: &str = "ssm";
const MAGIC: &[u8] = b"SWEEPSTAR-MESURE\n";
const FORMAT: u32 = 1;

/// Spectre du RTA figé (puissance par case, sans pondération).
#[derive(Debug, Clone)]
pub struct RtaSnapshot {
    pub power: Vec<f64>,
    pub bin_hz: f64,
}

/// Fonction de transfert figée.
#[derive(Debug, Clone)]
pub struct TransferSnapshot {
    pub h: Vec<Complex64>,
    pub coherence: Vec<f32>,
    pub bin_hz: f64,
    pub sample_rate: f64,
    /// Retard de la référence au moment de la capture (ms).
    pub delay_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceKind {
    Sweep,
    Rta,
    Transfer,
}

#[derive(Debug, Clone)]
pub enum TraceData {
    Sweep(Arc<Measurement>),
    Rta(Arc<RtaSnapshot>),
    Transfer(Arc<TransferSnapshot>),
}

impl TraceData {
    pub fn kind(&self) -> TraceKind {
        match self {
            TraceData::Sweep(_) => TraceKind::Sweep,
            TraceData::Rta(_) => TraceKind::Rta,
            TraceData::Transfer(_) => TraceKind::Transfer,
        }
    }
}

/// En-tête du fichier.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Header {
    format: u32,
    name: String,
    color: [u8; 3],
    kind: TraceKind,
    /// Date de la capture (secondes depuis 1970).
    created: u64,
    sample_rate: f64,
    bin_hz: f64,
    #[serde(default)]
    reference: bool,
    #[serde(default)]
    delay_ms: f64,
    #[serde(default)]
    ir_pre: usize,
    #[serde(default)]
    mic_peak_db: f32,
    #[serde(default)]
    ref_peak_db: Option<f32>,
    /// Longueurs des tableaux qui suivent.
    lens: Vec<usize>,
}

/// Une mesure de la liste.
pub struct Trace {
    pub name: String,
    pub color: [u8; 3],
    pub visible: bool,
    pub kind: TraceKind,
    pub created: u64,
    /// Fichier de la mesure (`None` : pas encore écrit, écriture impossible).
    pub path: Option<PathBuf>,
    data: Option<TraceData>,
}

impl Trace {
    pub fn new(name: String, color: [u8; 3], data: TraceData) -> Self {
        Self { name, color, visible: true, kind: data.kind(), created: now(), path: None, data: Some(data) }
    }

    #[cfg(test)]
    pub fn is_loaded(&self) -> bool {
        self.data.is_some()
    }

    pub fn measurement(&self) -> Option<&Arc<Measurement>> {
        match &self.data {
            Some(TraceData::Sweep(m)) => Some(m),
            _ => None,
        }
    }

    pub fn rta(&self) -> Option<&Arc<RtaSnapshot>> {
        match &self.data {
            Some(TraceData::Rta(r)) => Some(r),
            _ => None,
        }
    }

    pub fn transfer(&self) -> Option<&Arc<TransferSnapshot>> {
        match &self.data {
            Some(TraceData::Transfer(t)) => Some(t),
            _ => None,
        }
    }

    /// Charge les données depuis le fichier si besoin.
    pub fn load(&mut self) -> Result<(), String> {
        if self.data.is_some() {
            return Ok(());
        }
        let Some(p) = &self.path else { return Err("pas de fichier".into()) };
        let (_, data) = read_file(p).map_err(|e| format!("{} : {e}", p.display()))?;
        self.data = Some(data);
        Ok(())
    }

    /// Libère les données (une mesure masquée ne garde que son en-tête).
    pub fn unload(&mut self) {
        if self.path.is_some() {
            self.data = None;
        }
    }

    /// Écrit la mesure dans `path` (données chargées au besoin).
    pub fn write_to(&mut self, path: &Path) -> Result<(), String> {
        self.load()?;
        let Some(data) = &self.data else { return Err("données absentes".into()) };
        write_file(path, &self.name, self.color, self.created, data).map_err(|e| format!("{} : {e}", path.display()))
    }

    /// Réécrit le fichier (nom ou couleur changés).
    pub fn save(&mut self) -> Result<(), String> {
        match self.path.clone() {
            Some(p) => self.write_to(&p),
            None => Ok(()),
        }
    }

    /// Date lisible de la capture (UTC, « 2026-11-12 08:15 »).
    pub fn date(&self) -> String {
        fmt_date(self.created)
    }
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Date civile (UTC) depuis des secondes Unix, sans dépendance.
fn civil(secs: u64) -> (i64, u32, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d, (rem / 3600) as u32, (rem / 60 % 60) as u32, (rem % 60) as u32)
}

pub fn fmt_date(secs: u64) -> String {
    let (y, m, d, h, mi, _) = civil(secs);
    format!("{y}-{m:02}-{d:02} {h:02}:{mi:02}")
}

/// Nom de fichier d'une nouvelle mesure : date et nom, sans caractère interdit.
pub fn file_name(name: &str, created: u64) -> String {
    let (y, m, d, h, mi, s) = civil(created);
    let clean: String = name.chars().map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' }).take(40).collect();
    format!("{y}{m:02}{d:02}-{h:02}{mi:02}{s:02}-{clean}.{EXTENSION}")
}

fn write_file(path: &Path, name: &str, color: [u8; 3], created: u64, data: &TraceData) -> std::io::Result<()> {
    let mut header = Header {
        format: FORMAT,
        name: name.to_string(),
        color,
        kind: data.kind(),
        created,
        sample_rate: 0.0,
        bin_hz: 0.0,
        reference: false,
        delay_ms: 0.0,
        ir_pre: 0,
        mic_peak_db: 0.0,
        ref_peak_db: None,
        lens: Vec::new(),
    };
    match data {
        TraceData::Sweep(m) => {
            header.sample_rate = m.sample_rate;
            header.bin_hz = m.bin_hz;
            header.reference = m.method == Method::Reference;
            header.delay_ms = m.delay_ms;
            header.ir_pre = m.ir_pre;
            header.mic_peak_db = m.mic_peak_db;
            header.ref_peak_db = m.ref_peak_db;
            header.lens = vec![m.h.len(), m.ir.len(), m.etc.len()];
        }
        TraceData::Rta(r) => {
            header.bin_hz = r.bin_hz;
            header.lens = vec![r.power.len()];
        }
        TraceData::Transfer(t) => {
            header.sample_rate = t.sample_rate;
            header.bin_hz = t.bin_hz;
            header.delay_ms = t.delay_ms;
            header.lens = vec![t.h.len(), t.coherence.len()];
        }
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // Écriture dans un fichier voisin puis renommage : jamais de fichier à moitié écrit.
    let tmp = path.with_extension("ssm-tmp");
    {
        let mut w = BufWriter::new(File::create(&tmp)?);
        let json = serde_json::to_vec(&header)?;
        w.write_all(MAGIC)?;
        w.write_all(&(json.len() as u32).to_le_bytes())?;
        w.write_all(&json)?;
        match data {
            TraceData::Sweep(m) => {
                for c in &m.h {
                    w.write_all(&(c.re as f32).to_le_bytes())?;
                    w.write_all(&(c.im as f32).to_le_bytes())?;
                }
                for v in m.ir.iter().chain(&m.etc) {
                    w.write_all(&v.to_le_bytes())?;
                }
            }
            TraceData::Rta(r) => {
                for v in &r.power {
                    w.write_all(&v.to_le_bytes())?;
                }
            }
            TraceData::Transfer(t) => {
                for c in &t.h {
                    w.write_all(&(c.re as f32).to_le_bytes())?;
                    w.write_all(&(c.im as f32).to_le_bytes())?;
                }
                for v in &t.coherence {
                    w.write_all(&v.to_le_bytes())?;
                }
            }
        }
        w.flush()?;
    }
    std::fs::rename(&tmp, path)
}

fn bad(msg: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, msg.to_string())
}

fn read_header(r: &mut impl Read) -> std::io::Result<Header> {
    let mut magic = vec![0u8; MAGIC.len()];
    r.read_exact(&mut magic)?;
    if magic != MAGIC {
        return Err(bad("ce n'est pas une mesure SweepStar"));
    }
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > 1 << 20 {
        return Err(bad("en-tête trop long"));
    }
    let mut json = vec![0u8; len];
    r.read_exact(&mut json)?;
    let h: Header = serde_json::from_slice(&json)?;
    if h.format > FORMAT {
        return Err(bad("mesure enregistrée par une version plus récente de SweepStar"));
    }
    let expected = match h.kind {
        TraceKind::Sweep => 3,
        TraceKind::Rta => 1,
        TraceKind::Transfer => 2,
    };
    if h.lens.len() != expected || h.lens.iter().any(|&n| n > 1 << 24) {
        return Err(bad("en-tête invalide"));
    }
    Ok(h)
}

fn read_f32s(r: &mut impl Read, n: usize) -> std::io::Result<Vec<f32>> {
    let mut b = vec![0u8; n * 4];
    r.read_exact(&mut b)?;
    Ok(b.as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect())
}

fn read_file(path: &Path) -> std::io::Result<(Header, TraceData)> {
    let mut r = BufReader::new(File::open(path)?);
    let h = read_header(&mut r)?;
    let data = match h.kind {
        TraceKind::Sweep => {
            let hv = read_f32s(&mut r, 2 * h.lens[0])?;
            let ir = read_f32s(&mut r, h.lens[1])?;
            let etc = read_f32s(&mut r, h.lens[2])?;
            TraceData::Sweep(Arc::new(Measurement {
                sample_rate: h.sample_rate,
                method: if h.reference { Method::Reference } else { Method::InverseFilter },
                delay_ms: h.delay_ms,
                h: hv.as_chunks::<2>().0.iter().map(|c| Complex64::new(c[0] as f64, c[1] as f64)).collect(),
                bin_hz: h.bin_hz,
                ir,
                ir_pre: h.ir_pre,
                etc,
                mic_peak_db: h.mic_peak_db,
                ref_peak_db: h.ref_peak_db,
            }))
        }
        TraceKind::Rta => {
            let mut b = vec![0u8; h.lens[0] * 8];
            r.read_exact(&mut b)?;
            let power = b.as_chunks::<8>().0.iter().map(|c| f64::from_le_bytes(*c)).collect();
            TraceData::Rta(Arc::new(RtaSnapshot { power, bin_hz: h.bin_hz }))
        }
        TraceKind::Transfer => {
            let hv = read_f32s(&mut r, 2 * h.lens[0])?;
            let coherence = read_f32s(&mut r, h.lens[1])?;
            TraceData::Transfer(Arc::new(TransferSnapshot {
                h: hv.as_chunks::<2>().0.iter().map(|c| Complex64::new(c[0] as f64, c[1] as f64)).collect(),
                coherence,
                bin_hz: h.bin_hz,
                sample_rate: h.sample_rate,
                delay_ms: h.delay_ms,
            }))
        }
    };
    Ok((h, data))
}

/// Lit l'en-tête d'un fichier de mesure (sans les données).
pub fn open(path: &Path) -> Result<Trace, String> {
    let mut r = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let h = read_header(&mut r).map_err(|e| e.to_string())?;
    Ok(Trace { name: h.name, color: h.color, visible: false, kind: h.kind, created: h.created, path: Some(path.to_path_buf()), data: None })
}

/// Mesures du dossier, de la plus ancienne à la plus récente ; rend aussi les fichiers illisibles.
pub fn scan(dir: &Path) -> (Vec<Trace>, Vec<String>) {
    let mut traces = Vec::new();
    let mut errors = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return (traces, errors) };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x.eq_ignore_ascii_case(EXTENSION)) {
            match open(&p) {
                Ok(t) => traces.push(t),
                Err(err) => errors.push(format!("{} : {err}", p.display())),
            }
        }
    }
    traces.sort_by(|a, b| a.created.cmp(&b.created).then_with(|| a.path.cmp(&b.path)));
    (traces, errors)
}

/// Dossier des mesures par défaut : Documents/SweepStar/Mesures.
pub fn default_dir() -> Option<PathBuf> {
    directories::UserDirs::new()
        .and_then(|u| u.document_dir().map(|d| d.join("SweepStar").join("Mesures")))
        .or_else(|| crate::controller::APP.config_file("Mesures"))
}

/// Ouvre le dossier dans le gestionnaire de fichiers du système.
pub fn reveal(dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let cmd = if cfg!(target_os = "windows") {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(cmd).arg(dir).spawn().map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!("sweepstar-traces-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    #[test]
    fn aller_retour_sweep_et_rta() {
        let d = dir();
        let m = Measurement {
            sample_rate: 48_000.0,
            method: Method::Reference,
            delay_ms: 12.5,
            h: (0..100).map(|k| Complex64::new(k as f64 * 0.5, -1.0)).collect(),
            bin_hz: 0.73,
            ir: vec![0.25; 50],
            ir_pre: 7,
            etc: vec![-3.0; 50],
            mic_peak_db: -12.0,
            ref_peak_db: Some(-6.0),
        };
        let mut t = Trace::new("Façade L".into(), [1, 2, 3], TraceData::Sweep(Arc::new(m)));
        let p = d.join(file_name(&t.name, t.created));
        t.write_to(&p).unwrap();
        let mut r = Trace::new("RTA".into(), [4, 5, 6], TraceData::Rta(Arc::new(RtaSnapshot { power: vec![1e-3, 2e-3], bin_hz: 5.86 })));
        r.created += 1;
        r.write_to(&d.join("b.ssm")).unwrap();
        std::fs::write(d.join("abime.ssm"), b"rien").unwrap();

        let (mut list, errors) = scan(&d);
        assert_eq!(errors.len(), 1);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Façade L");
        assert!(!list[0].is_loaded());
        list[0].load().unwrap();
        let back = list[0].measurement().unwrap();
        assert_eq!(back.method, Method::Reference);
        assert_eq!(back.h[10], Complex64::new(5.0, -1.0));
        assert_eq!((back.ir_pre, back.ref_peak_db), (7, Some(-6.0)));
        list[1].load().unwrap();
        assert_eq!(list[1].rta().unwrap().power, vec![1e-3, 2e-3]);
        let snap = TransferSnapshot { h: vec![Complex64::new(1.0, 0.5); 3], coherence: vec![0.9; 3], bin_hz: 2.9, sample_rate: 48_000.0, delay_ms: 3.5 };
        let mut tf = Trace::new("TF".into(), [7, 8, 9], TraceData::Transfer(Arc::new(snap)));
        tf.write_to(&d.join("c.ssm")).unwrap();
        let mut back = open(&d.join("c.ssm")).unwrap();
        back.load().unwrap();
        let b = back.transfer().unwrap();
        assert_eq!((b.h[2], b.coherence[1], b.delay_ms), (Complex64::new(1.0, 0.5), 0.9, 3.5));
        // Renommage : le fichier est réécrit.
        list[1].name = "Après EQ".into();
        list[1].save().unwrap();
        assert_eq!(open(&d.join("b.ssm")).unwrap().name, "Après EQ");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn dates() {
        assert_eq!(fmt_date(0), "1970-01-01 00:00");
        assert_eq!(fmt_date(1_794_470_400), "2026-11-12 08:00");
        assert!(file_name("a/b c", 0).starts_with("19700101-000000-a_b_c"));
    }
}
