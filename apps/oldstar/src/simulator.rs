//! Fausse console Yamaha (`oldstar --simulateur [port]`) pour essayer sans matériel.
//!
//! Elle répond aux « get » et aux « set » comme une QL1, prévient les autres clients par
//! « NOTIFY set », bouge de temps en temps un fader « en façade », et envoie des niveaux
//! factices (« NOTIFY mtr ») après « mtrstart », ou en réponse aux « get » des vu-mètres.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::yamaha::{tokens, IN, MIX};

const CHANNELS: u32 = 40;
const MIXES: u32 = 16;

#[derive(Default)]
struct Console {
    params: HashMap<(String, u32, u32), String>,
    /// Clients : connexion d'écriture et abonnement aux niveaux.
    clients: Vec<(u64, TcpStream, bool)>,
    next_id: u64,
}

fn default_value(addr: &str, x: u32, y: u32) -> Option<String> {
    let a = addr.strip_prefix("MIXER:Current/")?;
    Some(match a {
        "InCh/Label/Name" => format!("\"CH {}\"", x + 1),
        "Mix/Label/Name" => format!("\"MIX {}\"", x + 1),
        "InCh/Fader/Level" => if x < 8 { "0" } else { "-32768" }.into(),
        "Mix/Fader/Level" => "0".into(),
        "InCh/Fader/On" | "Mix/Fader/On" | "InCh/PEQ/On" => "1".into(),
        "InCh/ToMix/On" => "1".into(),
        "InCh/ToMix/Level" => "-32768".into(),
        "InCh/PEQ/Band/Gain" => "0".into(),
        // Hz × 10 et Q × 1000, comme les échelles par défaut d'OldStar.
        "InCh/PEQ/Band/Freq" => ["1250", "10000", "40000", "100000"].get(y as usize)?.to_string(),
        "InCh/PEQ/Band/Q" => "700".into(),
        // 0 : en cloche ; 1 : plateau (LOW, HIGH) ; 2 : coupe-haut (HIGH). Supposé.
        "InCh/PEQ/Band/Type" => "0".into(),
        "InCh/HPF/On" => "0".into(),
        "InCh/HPF/Freq" => "800".into(),
        _ if a.starts_with("Meter/") => "0".into(),
        _ => return None,
    })
}

fn text_of(addr: &str, v: &str) -> String {
    if addr.ends_with("/On") {
        return if v == "1" { "ON".into() } else { "OFF".into() };
    }
    if addr.ends_with("/Level") || addr.ends_with("/Gain") {
        return match v.parse::<i64>() {
            Ok(-32768) => "-Inf".into(),
            Ok(n) => format!("{:.2}", n as f64 / 100.0),
            Err(_) => v.into(),
        };
    }
    v.trim_matches('"').to_string()
}

/// Niveau factice d'une voie à l'instant t (0 à 127), qui bouge comme de la musique.
fn fake_level(ch: u32, t: f64, on: bool, fader_db: f64) -> u32 {
    if !on || fader_db < -80.0 {
        return 0;
    }
    let base = 70.0 + 30.0 * (t * (0.7 + ch as f64 * 0.13)).sin() * (t * 2.3 + ch as f64).cos();
    let jitter = ((t * 37.0 + ch as f64 * 7.7).sin() * 43758.5453).fract().abs() * 25.0;
    (base + jitter + fader_db / 2.0).clamp(0.0, 127.0) as u32
}

pub fn run(port: u16) {
    let listener = match TcpListener::bind(("0.0.0.0", port)) {
        Ok(l) => l,
        Err(e) => {
            crate::journal::error(&format!("Simulateur : port {port} indisponible ({e})"));
            return;
        }
    };
    crate::journal::info(&format!("Simulateur de console Yamaha (QL1, {CHANNELS} voies, {MIXES} mix) à l'écoute sur le port {port}"));
    let console = Arc::new(Mutex::new(Console::default()));
    {
        let console = console.clone();
        std::thread::spawn(move || ticker(console));
    }
    for stream in listener.incoming().flatten() {
        let console = console.clone();
        std::thread::spawn(move || client(console, stream));
    }
}

fn value(c: &Console, addr: &str, x: u32, y: u32) -> Option<String> {
    c.params.get(&(addr.to_string(), x, y)).cloned().or_else(|| default_value(addr, x, y))
}

/// Mouvements « en façade » et niveaux factices.
fn ticker(console: Arc<Mutex<Console>>) {
    let start = Instant::now();
    let mut last_move = Instant::now();
    let mut n = 0u32;
    loop {
        std::thread::sleep(Duration::from_millis(100));
        let t = start.elapsed().as_secs_f64();
        let Ok(mut c) = console.lock() else { return };
        let mut lines: Vec<(String, bool)> = Vec::new();
        if last_move.elapsed() > Duration::from_secs(5) {
            last_move = Instant::now();
            n += 1;
            let ch = (n * 7) % CHANNELS;
            let level = if n % 2 == 0 { -1000 } else { -500 };
            let addr = format!("{IN}/Fader/Level");
            c.params.insert((addr.clone(), ch, 0), level.to_string());
            let v = level.to_string();
            lines.push((format!("NOTIFY set {addr} {ch} 0 {v} \"{}\"", text_of(&addr, &v)), false));
        }
        let fader = |c: &Console, base: &str, ch: u32| {
            let on = value(c, &format!("{base}/Fader/On"), ch, 0).as_deref() == Some("1");
            let db = value(c, &format!("{base}/Fader/Level"), ch, 0).and_then(|v| v.parse::<f64>().ok()).unwrap_or(-32768.0) / 100.0;
            (on, db)
        };
        let inch: Vec<String> = (0..CHANNELS)
            .map(|ch| {
                let (on, db) = fader(&c, IN, ch);
                fake_level(ch, t, on, db.max(-90.0)).to_string()
            })
            .collect();
        let mix: Vec<String> = (0..MIXES)
            .map(|m| {
                let (on, db) = fader(&c, MIX, m);
                (fake_level(m + 100, t, on, db.max(-90.0)) * 9 / 10).to_string()
            })
            .collect();
        lines.push((format!("NOTIFY mtr MIXER:Current/Meter/InCh PostOn {}", inch.join(" ")), true));
        lines.push((format!("NOTIFY mtr MIXER:Current/Meter/Mix PostOn {}", mix.join(" ")), true));
        // Les « get » des vu-mètres (mode poll) lisent ces valeurs.
        for (ch, v) in inch.iter().enumerate() {
            c.params.insert(("MIXER:Current/Meter/InCh/PostOn".into(), ch as u32, 0), v.clone());
        }
        for (m, v) in mix.iter().enumerate() {
            c.params.insert(("MIXER:Current/Meter/Mix/PostOn".into(), m as u32, 0), v.clone());
        }
        c.clients.retain_mut(|(_, s, metered)| {
            for (l, is_meter) in &lines {
                if *is_meter && !*metered {
                    continue;
                }
                if s.write_all(format!("{l}\n").as_bytes()).is_err() {
                    return false;
                }
            }
            true
        });
    }
}

fn client(console: Arc<Mutex<Console>>, stream: TcpStream) {
    let peer = stream.peer_addr().map(|a| a.to_string()).unwrap_or_default();
    crate::journal::info(&format!("Simulateur : client connecté ({peer})"));
    let Ok(writer) = stream.try_clone() else { return };
    let id = {
        let Ok(mut c) = console.lock() else { return };
        c.next_id += 1;
        let id = c.next_id;
        c.clients.push((id, writer, false));
        id
    };
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let Ok(line) = line else { break };
        let t = tokens(line.trim());
        let t: Vec<&str> = t.iter().map(String::as_str).collect();
        let Ok(mut c) = console.lock() else { break };
        let mut reply: Vec<String> = Vec::new();
        let mut notify: Option<String> = None;
        match t.as_slice() {
            ["get", addr, x, y] => match (x.parse::<u32>(), y.parse::<u32>()) {
                (Ok(x), Ok(y)) => match value(&c, addr, x, y) {
                    Some(v) => reply.push(format!("OK get {addr} {x} {y} {v}")),
                    None => reply.push("ERROR get UnknownAddress".into()),
                },
                _ => reply.push("ERROR get InvalidArgument".into()),
            },
            ["set", addr, x, y, v] => match (x.parse::<u32>(), y.parse::<u32>(), default_value(addr, 0, 0)) {
                (Ok(x), Ok(y), Some(_)) => {
                    let v = if v.starts_with('"') { v.to_string() } else { v.to_string() };
                    let v = if addr.ends_with("/Name") { format!("\"{}\"", v.trim_matches('"')) } else { v };
                    c.params.insert((addr.to_string(), x, y), v.clone());
                    let text = text_of(addr, &v);
                    reply.push(format!("OK set {addr} {x} {y} {v} \"{text}\""));
                    notify = Some(format!("NOTIFY set {addr} {x} {y} {v} \"{text}\""));
                }
                _ => reply.push("ERROR set InvalidArgument".into()),
            },
            ["devinfo", "productname"] => reply.push("OK devinfo productname \"QL1\"".into()),
            ["mtrstart", ..] => {
                if let Some(cl) = c.clients.iter_mut().find(|cl| cl.0 == id) {
                    cl.2 = true;
                }
                reply.push(format!("OK {}", line.trim()));
            }
            ["mtrstop", ..] => {
                if let Some(cl) = c.clients.iter_mut().find(|cl| cl.0 == id) {
                    cl.2 = false;
                }
                reply.push(format!("OK {}", line.trim()));
            }
            [] => continue,
            _ => reply.push("ERROR unknown UnknownCommand".into()),
        }
        if let Some(cl) = c.clients.iter_mut().find(|cl| cl.0 == id) {
            for r in &reply {
                let _ = cl.1.write_all(format!("{r}\n").as_bytes());
            }
        }
        if let Some(n) = notify {
            for cl in c.clients.iter_mut().filter(|cl| cl.0 != id) {
                let _ = cl.1.write_all(format!("{n}\n").as_bytes());
            }
        }
    }
    if let Ok(mut c) = console.lock() {
        c.clients.retain(|cl| cl.0 != id);
    }
    crate::journal::info(&format!("Simulateur : client parti ({peer})"));
}
