//! Suivi des micros HF : pile, HF, audio, mute et alertes de chaque voie, quelle que soit la
//! marque. Un fil par récepteur ; tout est ramené à `hub::Mic`.
//!
//! - **Sennheiser** ew 300/500 G3, EM 2000/2050 (« Media control protocol », doc 120122) :
//!   texte sur UDP, port 53212, lignes terminées par CR. `Push <délai s> <cycle ms> <mode>`
//!   abonne aux attributs cycliques (RF1, RF2, States, RF, AF, Bat, Msg, Config) pendant
//!   <délai> secondes ; on le renvoie avant l'échéance. Le mode 7 envoie aussi les attributs de
//!   configuration (Name, Frequency, Mute…) à chaque changement. L'EM 100 G3 n'a pas de réseau.
//! - **Shure** Axient Digital (AD4D, AD4Q ; « Command Strings » version 4) : texte sur TCP, port
//!   2202, messages `< REP x CLÉ valeur >` envoyés à chaque changement, et `< SAMPLE x ALL … >`
//!   au rythme réglé par `METER_RATE` pour les niveaux.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs, UdpSocket};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::{WirelessKind, WirelessRx};
use crate::hub::{Hub, Mic};
use crate::journal;

/// Sans nouvelles depuis ce délai, le récepteur est hors ligne.
const SILENCE: Duration = Duration::from_secs(4);

/// Lance le suivi des récepteurs des réglages, et le relance à chaque nouvelle config.
pub fn start(hub: Arc<Hub>) {
    std::thread::Builder::new()
        .name("micros".into())
        .spawn(move || loop {
            let gen = hub.generation.load(Ordering::Relaxed);
            let list = hub.cfg().wireless;
            hub.clear_mics();
            for (i, rx) in list.into_iter().enumerate() {
                if !rx.enabled || rx.host.trim().is_empty() {
                    continue;
                }
                let hub = hub.clone();
                let _ = std::thread::Builder::new().name(format!("micro-{i}")).spawn(move || match rx.kind {
                    WirelessKind::Sennheiser => sennheiser(&hub, i, &rx, gen),
                    WirelessKind::Shure => shure(&hub, i, &rx, gen),
                });
            }
            while hub.generation.load(Ordering::Relaxed) == gen {
                std::thread::sleep(Duration::from_millis(300));
            }
        })
        .expect("thread micros");
}

fn alive(hub: &Hub, gen: u64) -> bool {
    hub.generation.load(Ordering::Relaxed) == gen
}

fn addr(rx: &WirelessRx) -> Option<std::net::SocketAddr> {
    (rx.host.trim(), rx.port()).to_socket_addrs().ok()?.next()
}

/// Niveau en pourcentage, borné à 0..100.
fn pct(v: f64) -> u8 {
    v.clamp(0.0, 100.0).round() as u8
}

/* ---------- Sennheiser ---------- */

/// Applique une ligne reçue d'un récepteur Sennheiser.
pub fn sennheiser_line(m: &mut Mic, line: &str) {
    let mut it = line.split_whitespace();
    let Some(key) = it.next() else { return };
    let args: Vec<&str> = it.collect();
    let num = |i: usize| args.get(i).and_then(|s| s.parse::<f64>().ok());
    match key {
        "Name" => m.name = line[4..].trim().to_string(),
        "Frequency" => m.freq = num(0).unwrap_or(0.0) as u32,
        "Mute" => m.mute = num(0) == Some(1.0),
        // Niveau HF de chaque antenne : mini, maxi depuis le dernier cycle (100 % = 40 dBµV).
        "RF1" => m.rf_a = pct(num(1).unwrap_or(0.0)),
        "RF2" => m.rf_b = pct(num(1).unwrap_or(0.0)),
        // Niveau courant, antenne active, pilote.
        "RF" => m.ant = num(1).unwrap_or(0.0) as u8,
        // Audio : niveau, crête maintenue (0 % = −50 dB, 100 % = crête), état de mute (bits).
        "AF" => {
            m.af = pct(num(0).unwrap_or(0.0));
            m.peak = pct(num(1).unwrap_or(0.0));
            let bits = num(2).unwrap_or(0.0) as u32;
            // Bit 2 : RF-Mute (pas d'émetteur reçu).
            m.tx = bits & 0b100 == 0;
        }
        // Pile : 0, 30, 70, 100 %, ou « ? » (pas d'information de l'émetteur).
        "Bat" => {
            m.bat = num(0).map(|v| v as u8);
            m.bars = m.bat.map(|b| match b {
                0 => 0,
                1..=30 => 1,
                31..=70 => 3,
                _ => 5,
            });
        }
        "Msg" => {
            m.warn = args
                .iter()
                .filter_map(|w| match *w {
                    "Low_Battery" => Some("low_bat"),
                    "Low_RF_Signal" => Some("low_rf"),
                    "AF_Peak" => Some("af_peak"),
                    "RX_Mute" => Some("rx_mute"),
                    "TX_Mute" => Some("tx_mute"),
                    "RF_Mute" => Some("rf_mute"),
                    _ => None,
                })
                .collect();
        }
        _ => {}
    }
}

fn sennheiser(hub: &Hub, i: usize, rx: &WirelessRx, gen: u64) {
    let host = rx.host.trim().to_string();
    let setup = |m: &mut Mic| {
        m.brand = "Sennheiser";
        m.model = "ew G3 / 2000".into();
        if m.name.is_empty() {
            m.name = host.clone();
        }
    };
    hub.update_mic(i, 1, setup);
    let Some(to) = addr(rx) else {
        journal::warn(&format!("Micros : adresse « {host} » invalide"));
        return;
    };
    // Port local quelconque : le récepteur répond à l'adresse qui l'interroge.
    let sock = match UdpSocket::bind(("0.0.0.0", 0)) {
        Ok(s) => s,
        Err(e) => {
            journal::error(&format!("Micros : UDP impossible ({e})"));
            return;
        }
    };
    let _ = sock.set_read_timeout(Some(Duration::from_millis(300)));
    journal::info(&format!("Micros : suivi du récepteur Sennheiser {to}"));
    let mut last_push = Instant::now() - Duration::from_secs(60);
    let mut last_rx: Option<Instant> = None;
    let mut online = false;
    let mut buf = [0u8; 1600];
    while alive(hub, gen) {
        // Abonnement de 15 s, renouvelé toutes les 5 s : cycle de 200 ms, configuration et
        // alertes envoyées à chaque changement.
        if last_push.elapsed() >= Duration::from_secs(5) {
            let _ = sock.send_to(b"Push 15 200 7\r", to);
            last_push = Instant::now();
        }
        if let Ok((n, from)) = sock.recv_from(&mut buf) {
            if from.ip() == to.ip() {
                let text = String::from_utf8_lossy(&buf[..n]).into_owned();
                last_rx = Some(Instant::now());
                if !online {
                    online = true;
                    journal::info(&format!("Micros : récepteur Sennheiser {to} en ligne"));
                }
                hub.update_mic(i, 1, |m| {
                    m.online = true;
                    for line in text.split(['\r', '\n', ';']) {
                        let line = line.trim();
                        if !line.is_empty() {
                            sennheiser_line(m, line);
                        }
                    }
                });
            }
        }
        if online && last_rx.is_none_or(|t| t.elapsed() > SILENCE) {
            online = false;
            journal::warn(&format!("Micros : récepteur Sennheiser {to} injoignable"));
            hub.update_rx(i, offline);
        }
    }
}

fn offline(m: &mut Mic) {
    m.online = false;
    m.tx = false;
    m.rf_a = 0;
    m.rf_b = 0;
    m.af = 0;
    m.peak = 0;
}

/* ---------- Shure ---------- */

/// Contenu d'un `{…}` sans les espaces de remplissage.
fn braced(s: &str) -> String {
    s.trim().trim_start_matches('{').trim_end_matches('}').trim().to_string()
}

/// Message Shure (sans les chevrons) → voie concernée et changement à appliquer.
/// Rend `(voie, clé, valeur)` ; voie 0 pour un message de l'appareil.
pub fn shure_parse(msg: &str) -> Option<(u32, String, String)> {
    let mut parts = msg.trim().splitn(2, ' ');
    let kind = parts.next()?;
    if kind != "REP" && kind != "SAMPLE" {
        return None;
    }
    let rest = parts.next()?.trim();
    let (ch, rest) = match rest.split_once(' ') {
        Some((n, r)) if n.chars().all(|c| c.is_ascii_digit()) => (n.parse().ok()?, r),
        _ => (0, rest),
    };
    let (key, value) = rest.split_once(' ').unwrap_or((rest, ""));
    let key = if kind == "SAMPLE" { format!("SAMPLE_{key}") } else { key.to_string() };
    Some((ch, key, value.to_string()))
}

/// Applique un message Shure à une voie.
pub fn shure_apply(m: &mut Mic, key: &str, value: &str) {
    let n = |s: &str| s.trim().parse::<u32>().ok();
    let set_warn = |m: &mut Mic, w: &'static str, on: bool| {
        m.warn.retain(|x| *x != w);
        if on {
            m.warn.push(w);
        }
    };
    match key {
        "CHAN_NAME" => m.name = braced(value),
        "FREQUENCY" => m.freq = n(value).unwrap_or(0),
        "AUDIO_MUTE" => m.mute = value.trim() == "ON",
        "TX_BATT_BARS" => {
            m.bars = n(value).filter(|b| *b <= 5).map(|b| b as u8);
            let low = m.bars.is_some_and(|b| b <= 1);
            set_warn(m, "low_bat", low);
        }
        "TX_BATT_CHARGE_PERCENT" => m.bat = n(value).filter(|b| *b <= 100).map(|b| b as u8),
        "TX_BATT_MINS" => m.mins = n(value).filter(|v| *v <= 65532),
        "TX_MODEL" => m.tx = value.trim() != "UNKNOWN",
        "TX_MUTE_MODE_STATUS" => set_warn(m, "tx_mute", value.trim() == "MUTE"),
        "INTERFERENCE_STATUS" => set_warn(m, "interference", value.trim() == "DETECTED"),
        "ENCRYPTION_STATUS" => set_warn(m, "encryption", value.trim() == "ERROR"),
        // < SAMPLE x ALL qual audBitmap audPeak audRms rfAntStats rfBitmapA rfRssiA rfBitmapB rfRssiB >
        "SAMPLE_ALL" => {
            let f: Vec<&str> = value.split_whitespace().collect();
            if f.len() < 9 {
                return;
            }
            let bitmap = n(f[1]).unwrap_or(0);
            // dBFS = valeur − 120 ; −50 dBFS → 0 %, 0 dBFS → 100 %.
            let db = |s: &str| n(s).map(|v| v as f64 - 120.0).unwrap_or(-120.0);
            m.af = pct((db(f[3]) + 50.0) * 2.0);
            m.peak = pct((db(f[2]) + 50.0) * 2.0);
            set_warn(m, "af_peak", bitmap & 0x80 != 0);
            // Antennes : barres de la façade (5 diodes) et dBm (valeur − 120).
            let leds = |s: &str| (n(s).unwrap_or(0) & 0x1f).count_ones() as u8 * 20;
            m.rf_a = leds(f[5]);
            m.rf_b = leds(f[7]);
            m.dbm_a = n(f[6]).map(|v| v as i32 - 120);
            m.dbm_b = n(f[8]).map(|v| v as i32 - 120);
            let st: Vec<char> = f[4].chars().collect();
            m.ant = match (st.first() == Some(&'B'), st.get(1) == Some(&'B')) {
                (true, false) => 1,
                (false, true) => 2,
                _ => 0,
            };
        }
        _ => {}
    }
}

fn shure(hub: &Hub, i: usize, rx: &WirelessRx, gen: u64) {
    let host = rx.host.trim().to_string();
    let mut model = String::from("Axient Digital");
    let mut warned = false;
    while alive(hub, gen) {
        let Some(to) = addr(rx) else {
            journal::warn(&format!("Micros : adresse « {host} » invalide"));
            return;
        };
        let mut stream = match TcpStream::connect_timeout(&to, Duration::from_secs(3)) {
            Ok(s) => s,
            Err(e) => {
                if !warned {
                    journal::warn(&format!("Micros : récepteur Shure {to} injoignable ({e})"));
                    warned = true;
                }
                hub.update_mic(i, 1, |m| {
                    m.brand = "Shure";
                    m.model = model.clone();
                    if m.name.is_empty() {
                        m.name = host.clone();
                    }
                    offline(m);
                });
                for _ in 0..10 {
                    if !alive(hub, gen) {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(300));
                }
                continue;
            }
        };
        warned = false;
        journal::info(&format!("Micros : récepteur Shure {to} connecté"));
        let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
        let mut hello = String::from("< GET MODEL >< GET 0 ALL >");
        for ch in 1..=4 {
            hello.push_str(&format!("< SET {ch} METER_RATE 00200 >"));
        }
        if stream.write_all(hello.as_bytes()).is_err() {
            continue;
        }
        let mut pending = String::new();
        let mut buf = [0u8; 4096];
        let mut last_rx = Instant::now();
        while alive(hub, gen) {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    last_rx = Instant::now();
                    pending.push_str(&String::from_utf8_lossy(&buf[..n]));
                    while let (Some(a), Some(b)) = (pending.find('<'), pending.find('>')) {
                        if b < a {
                            pending.drain(..=b);
                            continue;
                        }
                        let msg = pending[a + 1..b].to_string();
                        pending.drain(..=b);
                        let Some((ch, key, value)) = shure_parse(&msg) else { continue };
                        if key == "MODEL" {
                            model = braced(&value);
                            let m = model.clone();
                            hub.update_rx(i, |x| x.model = m.clone());
                            continue;
                        }
                        if !(1..=4).contains(&ch) {
                            continue;
                        }
                        let model = model.clone();
                        hub.update_mic(i, ch, |m| {
                            m.brand = "Shure";
                            m.model = model;
                            m.online = true;
                            shure_apply(m, &key, &value);
                        });
                    }
                    if pending.len() > 8192 {
                        pending.clear();
                    }
                }
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                    if last_rx.elapsed() > SILENCE {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        journal::warn(&format!("Micros : liaison avec le récepteur Shure {to} perdue"));
        hub.update_rx(i, offline);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sennheiser_cyclique() {
        let mut m = Mic::default();
        for l in ["RF1 25 65 1", "RF2 28 78 0", "States 3 2", "RF 50 1 1", "AF 40 65 3", "Bat 70", "Msg Low_RF_Signal Low_Battery", "Config 234", "Name Vocal 1", "Frequency 822000 2 10", "Mute 1"] {
            sennheiser_line(&mut m, l);
        }
        assert_eq!((m.rf_a, m.rf_b, m.ant, m.af, m.peak), (65, 78, 1, 40, 65));
        assert_eq!((m.bat, m.bars), (Some(70), Some(3)));
        assert_eq!(m.warn, vec!["low_rf", "low_bat"]);
        assert_eq!((m.name.as_str(), m.freq, m.mute, m.tx), ("Vocal 1", 822000, true, true));
        sennheiser_line(&mut m, "AF 0 0 5");
        assert!(!m.tx, "RF-Mute : pas d'émetteur");
        sennheiser_line(&mut m, "Bat ?");
        assert_eq!(m.bat, None);
    }

    #[test]
    fn shure_messages() {
        let mut m = Mic::default();
        let mut apply = |s: &str| {
            let (ch, k, v) = shure_parse(s).unwrap();
            assert_eq!(ch, 1);
            shure_apply(&mut m, &k, &v);
        };
        apply("REP 1 CHAN_NAME {Lead Vox                       }");
        apply("REP 1 FREQUENCY 0578350");
        apply("REP 1 TX_BATT_BARS 001");
        apply("REP 1 TX_BATT_MINS 00125");
        apply("REP 1 TX_MODEL AD2");
        apply("SAMPLE 1 ALL 005 131 102 090 BX 31 086 07 065");
        assert_eq!((m.name.as_str(), m.freq, m.bars, m.mins, m.tx), ("Lead Vox", 578350, Some(1), Some(125), true));
        assert_eq!((m.rf_a, m.rf_b, m.ant, m.dbm_a, m.dbm_b), (100, 60, 1, Some(-34), Some(-55)));
        assert_eq!((m.af, m.peak), (40, 64));
        assert!(m.warn.contains(&"low_bat") && m.warn.contains(&"af_peak"));
        let (ch, k, v) = shure_parse("REP MODEL {AD4D                            }").unwrap();
        assert_eq!((ch, k.as_str(), braced(&v).as_str()), (0, "MODEL", "AD4D"));
        assert!(shure_parse("REP ERR").is_some_and(|(c, k, _)| c == 0 && k == "ERR"));
    }
}
