//! Client des consoles Midas M32 / Behringer X32 : OSC sur UDP 10023.
//!
//! La page ne connaît que les paramètres « façon Yamaha » (MIXER:Current/InCh/…) : ce module
//! les traduit dans les deux sens, si bien que l'onglet Console marche pareil avec les deux
//! consoles.
//!
//! - abonnement aux changements : « /xremote » toutes les 8 s (il dure 10 s côté console) ;
//! - vu-mètres : « /meters /meters/1 », renouvelé toutes les 8 s ; la console envoie un blob
//!   de flottants (32 voies, 8 entrées aux, 8 retours d'effets, 16 bus…) ;
//! - lecture d'un paramètre : son adresse sans argument, la console répond avec la valeur.
//!
//! Faders et niveaux d'envoi : 0 à 1 selon la loi des X32 / M32 (0,75 = 0 dB, 1 = +10 dB),
//! reprise de crates/star-remote/src/m32.rs (non importé : ce crate tire MIDI, mDNS et HTTP).

use rosc::{OscMessage, OscPacket, OscType};
use serde_json::Value;
use std::collections::VecDeque;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::hub::Hub;
use crate::yamaha::{Out, IN, MIX};

/// Échelles des valeurs d'EQ données à la page (comme les réglages Yamaha par défaut).
pub const FREQ_SCALE: f64 = 10.0;
pub const Q_SCALE: f64 = 1000.0;

/// Valeur Yamaha de −∞ (centièmes de dB).
const NEG_INF: i64 = -32768;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Name,
    /// Fader ou niveau d'envoi (0 à 1, loi X32).
    Fader,
    On,
    /// Gain d'EQ : 0 à 1 → −15 à +15 dB.
    Gain,
    /// Fréquence d'EQ : 0 à 1 → 20 Hz à 20 kHz (logarithmique).
    Freq,
    /// Q d'EQ : 0 à 1 → 10 à 0,3 (logarithmique).
    Q,
    /// Type de bande (0 LCut, 1 LShv, 2 PEQ, 3 VEQ, 4 HShv, 5 HCut) ; y = bande.
    Type(u32),
    /// Fréquence du coupe-bas du préampli : 0 à 1 → 20 à 400 Hz (logarithmique).
    HpfFreq,
}

/// Paramètre « façon Yamaha » → adresse M32.
pub fn to_m32(addr: &str, x: u32, y: u32) -> Option<(String, Kind)> {
    let a = addr.strip_prefix("MIXER:Current/")?;
    let ch = format!("/ch/{:02}", x + 1);
    let bus = format!("/bus/{:02}", x + 1);
    Some(match a {
        "InCh/Label/Name" => (format!("{ch}/config/name"), Kind::Name),
        "InCh/Fader/Level" => (format!("{ch}/mix/fader"), Kind::Fader),
        "InCh/Fader/On" => (format!("{ch}/mix/on"), Kind::On),
        "Mix/Label/Name" => (format!("{bus}/config/name"), Kind::Name),
        "Mix/Fader/Level" => (format!("{bus}/mix/fader"), Kind::Fader),
        "Mix/Fader/On" => (format!("{bus}/mix/on"), Kind::On),
        "InCh/PEQ/On" => (format!("{ch}/eq/on"), Kind::On),
        "InCh/PEQ/Band/Gain" if y < 4 => (format!("{ch}/eq/{}/g", y + 1), Kind::Gain),
        "InCh/PEQ/Band/Freq" if y < 4 => (format!("{ch}/eq/{}/f", y + 1), Kind::Freq),
        "InCh/PEQ/Band/Q" if y < 4 => (format!("{ch}/eq/{}/q", y + 1), Kind::Q),
        "InCh/PEQ/Band/Type" if y < 4 => (format!("{ch}/eq/{}/type", y + 1), Kind::Type(y)),
        "InCh/HPF/On" => (format!("{ch}/preamp/hpon"), Kind::On),
        "InCh/HPF/Freq" => (format!("{ch}/preamp/hpf"), Kind::HpfFreq),
        "InCh/ToMix/Level" if y < 16 => (format!("{ch}/mix/{:02}/level", y + 1), Kind::Fader),
        "InCh/ToMix/On" if y < 16 => (format!("{ch}/mix/{:02}/on", y + 1), Kind::On),
        _ => return None,
    })
}

/// Adresse M32 → paramètre « façon Yamaha » (adresse, x, y, sorte).
pub fn from_m32(addr: &str) -> Option<(String, u32, u32, Kind)> {
    let p: Vec<&str> = addr.trim_start_matches('/').split('/').collect();
    let num = |s: &str| s.parse::<u32>().ok().filter(|n| *n >= 1).map(|n| n - 1);
    let y = |a: &str| format!("MIXER:Current/{a}");
    Some(match p.as_slice() {
        ["ch", c, "config", "name"] => (y("InCh/Label/Name"), num(c)?, 0, Kind::Name),
        ["ch", c, "mix", "fader"] => (y("InCh/Fader/Level"), num(c)?, 0, Kind::Fader),
        ["ch", c, "mix", "on"] => (y("InCh/Fader/On"), num(c)?, 0, Kind::On),
        ["bus", b, "config", "name"] => (y("Mix/Label/Name"), num(b)?, 0, Kind::Name),
        ["bus", b, "mix", "fader"] => (y("Mix/Fader/Level"), num(b)?, 0, Kind::Fader),
        ["bus", b, "mix", "on"] => (y("Mix/Fader/On"), num(b)?, 0, Kind::On),
        ["ch", c, "eq", "on"] => (y("InCh/PEQ/On"), num(c)?, 0, Kind::On),
        ["ch", c, "eq", band, "g"] => (y("InCh/PEQ/Band/Gain"), num(c)?, num(band)?, Kind::Gain),
        ["ch", c, "eq", band, "f"] => (y("InCh/PEQ/Band/Freq"), num(c)?, num(band)?, Kind::Freq),
        ["ch", c, "eq", band, "q"] => (y("InCh/PEQ/Band/Q"), num(c)?, num(band)?, Kind::Q),
        ["ch", c, "eq", band, "type"] => {
            let b = num(band)?;
            (y("InCh/PEQ/Band/Type"), num(c)?, b, Kind::Type(b))
        }
        ["ch", c, "preamp", "hpon"] => (y("InCh/HPF/On"), num(c)?, 0, Kind::On),
        ["ch", c, "preamp", "hpf"] => (y("InCh/HPF/Freq"), num(c)?, 0, Kind::HpfFreq),
        ["ch", c, "mix", m, "level"] => (y("InCh/ToMix/Level"), num(c)?, num(m)?, Kind::Fader),
        ["ch", c, "mix", m, "on"] => (y("InCh/ToMix/On"), num(c)?, num(m)?, Kind::On),
        _ => return None,
    })
}

/// Position d'un fader (0 à 1) → dB, loi des X32 / M32 (reprise de star-remote).
pub fn fader_db(f: f32) -> Option<f32> {
    let f = f.clamp(0.0, 1.0);
    if f <= 0.0 {
        None
    } else if f >= 0.5 {
        Some(f * 40.0 - 30.0)
    } else if f >= 0.25 {
        Some(f * 80.0 - 50.0)
    } else if f >= 0.0625 {
        Some(f * 160.0 - 70.0)
    } else {
        Some(f * 480.0 - 90.0)
    }
}

/// dB → position du fader (inverse de `fader_db`).
pub fn db_fader(db: f32) -> f32 {
    let f = if db >= -10.0 {
        (db + 30.0) / 40.0
    } else if db >= -30.0 {
        (db + 50.0) / 80.0
    } else if db >= -60.0 {
        (db + 70.0) / 160.0
    } else {
        (db + 90.0) / 480.0
    };
    f.clamp(0.0, 1.0)
}

fn log_to(f: f32, min: f64, max: f64) -> f64 {
    min * (max / min).powf(f.clamp(0.0, 1.0) as f64)
}

fn log_from(v: f64, min: f64, max: f64) -> f32 {
    ((v.max(1e-9) / min).ln() / (max / min).ln()).clamp(0.0, 1.0) as f32
}

/// Valeur reçue de la console → valeur « façon Yamaha » pour la page.
pub fn value_from(kind: Kind, arg: &OscType) -> Option<Value> {
    let f = match arg {
        OscType::Float(f) => Some(*f),
        OscType::Double(d) => Some(*d as f32),
        _ => None,
    };
    let i = match arg {
        OscType::Int(i) => Some(*i),
        _ => None,
    };
    Some(match kind {
        Kind::Name => match arg {
            OscType::String(s) => Value::from(s.clone()),
            _ => return None,
        },
        Kind::On => Value::from(i.or(f.map(|f| (f > 0.5) as i32))? as i64),
        Kind::Fader => Value::from(fader_db(f?).map_or(NEG_INF, |db| (db * 100.0).round() as i64)),
        Kind::Gain => Value::from(((f? as f64 * 30.0 - 15.0) * 100.0).round() as i64),
        Kind::Freq => Value::from((log_to(f?, 20.0, 20000.0) * FREQ_SCALE).round() as i64),
        Kind::Q => Value::from((log_to(f?, 10.0, 0.3) * Q_SCALE).round() as i64),
        Kind::HpfFreq => Value::from((log_to(f?, 20.0, 400.0) * FREQ_SCALE).round() as i64),
        // Type M32 → type de la page : 0 cloche, 1 plateau, 2 coupe-haut (bande HIGH).
        Kind::Type(_) => Value::from(match i? {
            0 | 1 | 4 => 1,
            5 => 2,
            _ => 0,
        }),
    })
}

/// Valeur « façon Yamaha » venue de la page → argument OSC pour la console.
pub fn value_to(kind: Kind, v: &Value) -> Option<OscType> {
    let n = v.as_f64();
    Some(match kind {
        Kind::Name => OscType::String(v.as_str()?.to_string()),
        Kind::On => OscType::Int((n? > 0.5) as i32),
        Kind::Fader => {
            let raw = n?;
            OscType::Float(if raw <= NEG_INF as f64 { 0.0 } else { db_fader((raw / 100.0) as f32) })
        }
        Kind::Gain => OscType::Float((((n? / 100.0) + 15.0) / 30.0).clamp(0.0, 1.0) as f32),
        Kind::Freq => OscType::Float(log_from(n? / FREQ_SCALE, 20.0, 20000.0)),
        Kind::Q => OscType::Float(log_from(n? / Q_SCALE, 10.0, 0.3)),
        Kind::HpfFreq => OscType::Float(log_from(n? / FREQ_SCALE, 20.0, 400.0)),
        Kind::Type(band) => OscType::Int(match (band, n? as i64) {
            (0, 1) => 1,
            (3, 1) => 4,
            (3, 2) => 5,
            _ => 2,
        }),
    })
}

/// Niveau d'un vu-mètre (linéaire, 1 = 0 dBFS) → 0 à 100 sur 60 dB.
fn meter_pct(v: f32) -> u8 {
    if v <= 0.0 {
        return 0;
    }
    let db = 20.0 * v.log10();
    (((db + 60.0) / 60.0) * 100.0).clamp(0.0, 100.0) as u8
}

/// Blob de « /meters/1 » : entier (petit-boutiste) = nombre de flottants, puis les flottants.
pub fn parse_meters(blob: &[u8]) -> Vec<f32> {
    if blob.len() < 4 {
        return vec![];
    }
    let n = u32::from_le_bytes([blob[0], blob[1], blob[2], blob[3]]) as usize;
    blob[4..].chunks_exact(4).take(n).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
}

pub fn start(hub: Arc<Hub>) {
    let (tx, rx) = channel();
    if let Ok(mut t) = hub.yamaha_tx.lock() {
        *t = Some(tx);
    }
    std::thread::Builder::new().name("m32".into()).spawn(move || run(hub, rx)).expect("thread m32");
}

fn send(sock: &UdpSocket, to: SocketAddr, addr: &str, args: Vec<OscType>) {
    if let Ok(bytes) = rosc::encoder::encode(&OscPacket::Message(OscMessage { addr: addr.to_string(), args })) {
        let _ = sock.send_to(&bytes, to);
    }
}

fn run(hub: Arc<Hub>, rx: Receiver<Out>) {
    let sock = match UdpSocket::bind(("0.0.0.0", 0)) {
        Ok(s) => s,
        Err(e) => {
            crate::journal::error(&format!("M32 : socket UDP impossible ({e})"));
            return;
        }
    };
    let _ = sock.set_read_timeout(Some(Duration::from_millis(20)));
    let mut buf = vec![0u8; 65536];
    loop {
        let gen = hub.generation.load(Ordering::Relaxed);
        let cfg = hub.cfg().m32;
        let target = format!("{}:{}", cfg.host.trim(), cfg.port);
        let Some(to) = target.to_socket_addrs().ok().and_then(|mut a| a.find(|a| a.is_ipv4())) else {
            hub.set_yamaha_status(|s| {
                s.connected = false;
                s.error = format!("adresse introuvable : {target}");
            });
            std::thread::sleep(Duration::from_secs(2));
            continue;
        };
        let mut gets: VecDeque<String> = VecDeque::new();
        let initial = |gets: &mut VecDeque<String>| {
            for ch in 0..cfg.channels {
                for a in ["Label/Name", "Fader/Level", "Fader/On"] {
                    if let Some((m, _)) = to_m32(&format!("{IN}/{a}"), ch, 0) {
                        gets.push_back(m);
                    }
                }
            }
            for b in 0..cfg.buses {
                for a in ["Label/Name", "Fader/Level", "Fader/On"] {
                    if let Some((m, _)) = to_m32(&format!("{MIX}/{a}"), b, 0) {
                        gets.push_back(m);
                    }
                }
            }
        };
        initial(&mut gets);
        let mut last_sub = Instant::now() - Duration::from_secs(60);
        let mut last_info = Instant::now() - Duration::from_secs(60);
        let mut last_rx: Option<Instant> = None;
        let mut was_connected = false;
        while hub.generation.load(Ordering::Relaxed) == gen {
            // Demandes de la page.
            loop {
                match rx.try_recv() {
                    Ok(Out::Set { addr, x, y, value }) => {
                        hub.set_param(&addr, x, y, value.clone(), None);
                        if let Some((m, kind)) = to_m32(&addr, x, y) {
                            if let Some(arg) = value_to(kind, &value) {
                                send(&sock, to, &m, vec![arg]);
                            }
                        }
                    }
                    Ok(Out::LoadChannel(ch)) => {
                        let mut list = vec![format!("{IN}/PEQ/On"), format!("{IN}/HPF/On"), format!("{IN}/HPF/Freq")]
                            .into_iter()
                            .map(|a| (a, 0))
                            .collect::<Vec<_>>();
                        for band in 0..4 {
                            for p in ["Gain", "Freq", "Q", "Type"] {
                                list.push((format!("{IN}/PEQ/Band/{p}"), band));
                            }
                        }
                        for m in 0..cfg.buses {
                            list.push((format!("{IN}/ToMix/Level"), m));
                            list.push((format!("{IN}/ToMix/On"), m));
                        }
                        for (a, y) in list {
                            if let Some((m, _)) = to_m32(&a, ch, y) {
                                gets.push_back(m);
                            }
                        }
                    }
                    Ok(Out::LoadSends(mix)) => {
                        for ch in 0..cfg.channels {
                            for a in ["ToMix/Level", "ToMix/On"] {
                                if let Some((m, _)) = to_m32(&format!("{IN}/{a}"), ch, mix) {
                                    gets.push_back(m);
                                }
                            }
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            // Abonnements et état.
            if last_sub.elapsed() >= Duration::from_secs(8) {
                last_sub = Instant::now();
                send(&sock, to, "/xremote", vec![]);
                if cfg.meters {
                    send(&sock, to, "/meters", vec![OscType::String("/meters/1".into())]);
                }
            }
            if last_info.elapsed() >= Duration::from_secs(4) {
                last_info = Instant::now();
                send(&sock, to, "/info", vec![]);
            }
            // Lectures, par petits paquets pour ne pas noyer la console.
            for _ in 0..40 {
                match gets.pop_front() {
                    Some(a) => send(&sock, to, &a, vec![]),
                    None => break,
                }
            }
            // Réception.
            while let Ok((n, from)) = sock.recv_from(&mut buf) {
                if from.port() != to.port() {
                    continue;
                }
                last_rx = Some(Instant::now());
                let Ok((_, packet)) = rosc::decoder::decode_udp(&buf[..n]) else { continue };
                let mut msgs = Vec::new();
                flatten(packet, &mut msgs);
                for m in msgs {
                    receive(&hub, &cfg, m);
                }
            }
            let connected = last_rx.is_some_and(|t| t.elapsed() < Duration::from_secs(12));
            if connected != was_connected {
                was_connected = connected;
                if connected {
                    crate::journal::info(&format!("Console M32 connectée ({target})"));
                    // Retour après une coupure : tout relire.
                    gets.clear();
                    initial(&mut gets);
                } else {
                    crate::journal::warn(&format!("Console M32 muette ({target})"));
                }
                hub.set_yamaha_status(|s| {
                    s.connected = connected;
                    s.error = if connected { String::new() } else { format!("pas de réponse de {target}") };
                });
            } else if !connected && last_rx.is_none() {
                hub.set_yamaha_status(|s| {
                    s.connected = false;
                    s.error = format!("pas de réponse de {target}");
                });
            }
        }
    }
}

fn flatten(p: OscPacket, out: &mut Vec<OscMessage>) {
    match p {
        OscPacket::Message(m) => out.push(m),
        OscPacket::Bundle(b) => b.content.into_iter().for_each(|p| flatten(p, out)),
    }
}

fn receive(hub: &Hub, cfg: &crate::config::M32Config, m: OscMessage) {
    if m.addr == "/info" {
        if let Some(OscType::String(model)) = m.args.get(2) {
            let model = model.clone();
            hub.set_yamaha_status(|s| s.model = model);
        }
        return;
    }
    if m.addr == "/meters/1" {
        if let Some(OscType::Blob(b)) = m.args.first() {
            let v = parse_meters(b);
            let ch = cfg.channels.min(32) as usize;
            let bus = cfg.buses.min(16) as usize;
            let inch: Vec<u8> = v.iter().take(ch).map(|x| meter_pct(*x)).collect();
            let mix: Vec<u8> = v.iter().skip(48).take(bus).map(|x| meter_pct(*x)).collect();
            hub.set_meters(Some((0, inch)), Some((0, mix)));
        }
        return;
    }
    let Some((addr, x, y, kind)) = from_m32(&m.addr) else { return };
    let Some(arg) = m.args.first() else { return };
    if let Some(v) = value_from(kind, arg) {
        hub.set_param(&addr, x, y, v, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_round_trip() {
        for (a, x, y) in [
            ("MIXER:Current/InCh/Fader/Level", 4, 0),
            ("MIXER:Current/Mix/Fader/On", 15, 0),
            ("MIXER:Current/InCh/PEQ/Band/Q", 0, 3),
            ("MIXER:Current/InCh/ToMix/Level", 31, 7),
            ("MIXER:Current/InCh/HPF/Freq", 2, 0),
        ] {
            let (m, _) = to_m32(a, x, y).unwrap();
            let (back, bx, by, _) = from_m32(&m).unwrap();
            assert_eq!((back.as_str(), bx, by), (a, x, y), "{m}");
        }
        assert_eq!(to_m32("MIXER:Current/InCh/Fader/Level", 0, 0).unwrap().0, "/ch/01/mix/fader");
        assert_eq!(to_m32("MIXER:Current/InCh/ToMix/On", 9, 1).unwrap().0, "/ch/10/mix/02/on");
    }

    #[test]
    fn fader_law_both_ways() {
        assert_eq!(fader_db(0.75), Some(0.0));
        for db in [-80.0, -45.0, -20.0, -5.0, 0.0, 6.0] {
            let back = fader_db(db_fader(db)).unwrap();
            assert!((back - db).abs() < 0.01, "{db} → {back}");
        }
        let v = value_from(Kind::Fader, &OscType::Float(0.0)).unwrap();
        assert_eq!(v, Value::from(NEG_INF));
        assert!(matches!(value_to(Kind::Fader, &Value::from(NEG_INF)), Some(OscType::Float(f)) if f == 0.0));
    }

    #[test]
    fn eq_values_both_ways() {
        let g = value_to(Kind::Gain, &Value::from(600)).unwrap();
        assert_eq!(value_from(Kind::Gain, &g).unwrap(), Value::from(600));
        let f = value_to(Kind::Freq, &Value::from(10000)).unwrap(); // 1 kHz
        let back = value_from(Kind::Freq, &f).unwrap().as_i64().unwrap();
        assert!((back - 10000).abs() <= 10, "{back}");
        assert_eq!(value_to(Kind::Type(3), &Value::from(2)), Some(OscType::Int(5)));
        assert_eq!(value_from(Kind::Type(3), &OscType::Int(5)), Some(Value::from(2)));
    }

    #[test]
    fn meters_blob() {
        let mut b = 3u32.to_le_bytes().to_vec();
        for v in [0.0f32, 0.001, 1.0] {
            b.extend_from_slice(&v.to_le_bytes());
        }
        let v = parse_meters(&b);
        assert_eq!(v.len(), 3);
        assert_eq!(meter_pct(v[0]), 0);
        assert_eq!(meter_pct(v[2]), 100);
    }
}
