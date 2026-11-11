//! Fausse console Midas M32 (`oldstar --simulateur-m32 [port]`) pour essayer sans matériel.
//!
//! Elle répond en OSC sur UDP (10023 par défaut) comme une M32 : lecture d'une adresse sans
//! argument, écriture avec argument (et prévient les autres clients abonnés par /xremote),
//! /info, vu-mètres « /meters/1 » (blob de 96 flottants) pendant 10 s après « /meters ».
//! Un fader bouge de temps en temps « en façade ».

use rosc::{OscMessage, OscPacket, OscType};
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

const CHANNELS: u32 = 32;
const BUSES: u32 = 16;

struct Client {
    addr: SocketAddr,
    xremote: Instant,
    meters: Instant,
}

/// Valeur par défaut d'une adresse connue, sinon `None` (adresse inconnue : pas de réponse).
fn default_value(addr: &str) -> Option<OscType> {
    let p: Vec<&str> = addr.trim_start_matches('/').split('/').collect();
    let num = |s: &str| s.parse::<u32>().unwrap_or(0);
    let log = |v: f64, min: f64, max: f64| ((v / min).ln() / (max / min).ln()) as f32;
    Some(match p.as_slice() {
        ["ch", c, "config", "name"] => OscType::String(format!("Ch {c}")),
        ["bus", b, "config", "name"] => OscType::String(format!("Bus {b}")),
        ["ch", c, "mix", "fader"] => OscType::Float(if num(c) <= 8 { 0.75 } else { 0.0 }),
        ["bus", _, "mix", "fader"] => OscType::Float(0.75),
        ["ch", _, "mix", "on"] | ["bus", _, "mix", "on"] | ["ch", _, "eq", "on"] => OscType::Int(1),
        ["ch", _, "eq", _, "g"] => OscType::Float(0.5),
        ["ch", _, "eq", b, "f"] => OscType::Float(log([125.0, 1000.0, 4000.0, 10000.0][(num(b).clamp(1, 4) - 1) as usize], 20.0, 20000.0)),
        ["ch", _, "eq", _, "q"] => OscType::Float(log(0.7, 10.0, 0.3)),
        ["ch", _, "eq", _, "type"] => OscType::Int(2),
        ["ch", _, "preamp", "hpon"] => OscType::Int(0),
        ["ch", _, "preamp", "hpf"] => OscType::Float(log(80.0, 20.0, 400.0)),
        ["ch", _, "mix", _, "level"] => OscType::Float(0.0),
        ["ch", _, "mix", _, "on"] => OscType::Int(1),
        _ => return None,
    })
}

fn encode(addr: &str, args: Vec<OscType>) -> Vec<u8> {
    rosc::encoder::encode(&OscPacket::Message(OscMessage { addr: addr.into(), args })).unwrap_or_default()
}

pub fn run(port: u16) {
    let sock = match UdpSocket::bind(("0.0.0.0", port)) {
        Ok(s) => s,
        Err(e) => {
            crate::journal::error(&format!("Simulateur M32 : port UDP {port} indisponible ({e})"));
            return;
        }
    };
    let _ = sock.set_read_timeout(Some(Duration::from_millis(20)));
    crate::journal::info(&format!("Simulateur de console M32 ({CHANNELS} voies, {BUSES} bus) à l'écoute sur le port UDP {port}"));
    let mut state: HashMap<String, OscType> = HashMap::new();
    let mut clients: Vec<Client> = Vec::new();
    let mut buf = vec![0u8; 65536];
    let start = Instant::now();
    let mut last_meters = Instant::now();
    let mut last_move = Instant::now();
    let mut n = 0u32;
    loop {
        while let Ok((len, from)) = sock.recv_from(&mut buf) {
            let Ok((_, OscPacket::Message(m))) = rosc::decoder::decode_udp(&buf[..len]) else { continue };
            let client = match clients.iter_mut().position(|c| c.addr == from) {
                Some(i) => &mut clients[i],
                None => {
                    crate::journal::info(&format!("Simulateur M32 : client {from}"));
                    clients.push(Client { addr: from, xremote: Instant::now() - Duration::from_secs(20), meters: Instant::now() - Duration::from_secs(20) });
                    clients.last_mut().expect("client ajouté")
                }
            };
            match m.addr.as_str() {
                "/xremote" => client.xremote = Instant::now(),
                "/meters" => {
                    if matches!(m.args.first(), Some(OscType::String(s)) if s == "/meters/1") {
                        client.meters = Instant::now();
                    }
                }
                "/info" => {
                    let args = ["V2.07", "osc-server", "M32", "4.06"].iter().map(|s| OscType::String(s.to_string())).collect();
                    let _ = sock.send_to(&encode("/info", args), from);
                }
                addr => {
                    if m.args.is_empty() {
                        if let Some(v) = state.get(addr).cloned().or_else(|| default_value(addr)) {
                            let _ = sock.send_to(&encode(addr, vec![v]), from);
                        }
                    } else if default_value(addr).is_some() {
                        state.insert(addr.to_string(), m.args[0].clone());
                        let msg = encode(addr, vec![m.args[0].clone()]);
                        for c in clients.iter().filter(|c| c.addr != from && c.xremote.elapsed() < Duration::from_secs(10)) {
                            let _ = sock.send_to(&msg, c.addr);
                        }
                    }
                }
            }
        }
        let t = start.elapsed().as_secs_f64();
        // Un fader bouge « en façade » toutes les 5 s.
        if last_move.elapsed() >= Duration::from_secs(5) {
            last_move = Instant::now();
            n += 1;
            let addr = format!("/ch/{:02}/mix/fader", (n * 7) % CHANNELS + 1);
            let v = OscType::Float(if n % 2 == 0 { 0.5 } else { 0.625 });
            state.insert(addr.clone(), v.clone());
            let msg = encode(&addr, vec![v]);
            for c in clients.iter().filter(|c| c.xremote.elapsed() < Duration::from_secs(10)) {
                let _ = sock.send_to(&msg, c.addr);
            }
        }
        // Vu-mètres toutes les 50 ms.
        if last_meters.elapsed() >= Duration::from_millis(50) {
            last_meters = Instant::now();
            let mut vals = vec![0f32; 96];
            let level = |key: &str, on_key: &str, i: f64| -> f32 {
                let fader = match state.get(key).cloned().or_else(|| default_value(key)) {
                    Some(OscType::Float(f)) => f,
                    _ => 0.0,
                };
                let on = !matches!(state.get(on_key).cloned().or_else(|| default_value(on_key)), Some(OscType::Int(0)));
                if !on || fader <= 0.0 {
                    return 0.0;
                }
                let wave = 0.5 + 0.4 * (t * (0.7 + i * 0.13)).sin() * (t * 2.3 + i).cos();
                (wave * fader as f64 * 1.2).clamp(0.0, 1.0) as f32
            };
            for c in 0..CHANNELS {
                vals[c as usize] = level(&format!("/ch/{:02}/mix/fader", c + 1), &format!("/ch/{:02}/mix/on", c + 1), c as f64);
            }
            for b in 0..BUSES {
                vals[48 + b as usize] = level(&format!("/bus/{:02}/mix/fader", b + 1), &format!("/bus/{:02}/mix/on", b + 1), b as f64 + 0.5);
            }
            let mut blob = (vals.len() as u32).to_le_bytes().to_vec();
            for v in &vals {
                blob.extend_from_slice(&v.to_le_bytes());
            }
            let msg = encode("/meters/1", vec![OscType::Blob(blob)]);
            for c in clients.iter().filter(|c| c.meters.elapsed() < Duration::from_secs(10)) {
                let _ = sock.send_to(&msg, c.addr);
            }
        }
    }
}
