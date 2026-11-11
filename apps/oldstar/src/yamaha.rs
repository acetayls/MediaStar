//! Client des consoles Yamaha CL/QL : protocole texte de contrôle à distance sur TCP 49280.
//!
//! - `set <adresse> <x> <y> <valeur>` modifie un paramètre, la console répond « OK set … » ;
//! - `get <adresse> <x> <y>` le lit, réponse « OK get … <valeur> » ;
//! - chaque changement fait en façade (ou par un autre client) arrive en « NOTIFY set … ».
//!
//! Les niveaux sont en centièmes de dB (-32768 : -∞), les ON valent 0 ou 1. La connexion est
//! rétablie toute seule. Les vu-mètres arrivent en « NOTIFY mtr … » après un abonnement, ou
//! sont relevés par des « get » réguliers (voir `MeterConfig`).

use serde_json::Value;
use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config::{MeterMode, YamahaConfig};
use crate::hub::Hub;

pub const IN: &str = "MIXER:Current/InCh";
pub const MIX: &str = "MIXER:Current/Mix";

/// Demande venue d'un client.
pub enum Out {
    Set { addr: String, x: u32, y: u32, value: Value },
    /// Lit l'EQ, le HPF et les envois d'une voie (affichage du détail).
    LoadChannel(u32),
    /// Lit l'envoi de toutes les voies vers un mix (Sends on Fader).
    LoadSends(u32),
}

pub fn start(hub: Arc<Hub>) {
    let (tx, rx) = channel();
    if let Ok(mut t) = hub.yamaha_tx.lock() {
        *t = Some(tx);
    }
    std::thread::Builder::new().name("yamaha".into()).spawn(move || run(hub, rx)).expect("thread yamaha");
}

fn run(hub: Arc<Hub>, rx: Receiver<Out>) {
    loop {
        let gen = hub.generation.load(Ordering::Relaxed);
        let cfg = hub.cfg().yamaha;
        if !cfg.enabled {
            hub.set_yamaha_status(|s| {
                s.connected = false;
                s.error = "désactivée dans la config".into();
            });
            while rx.try_recv().is_ok() {}
            std::thread::sleep(Duration::from_millis(500));
            continue;
        }
        let target = format!("{}:{}", cfg.host.trim(), cfg.port);
        let stream = target
            .to_socket_addrs()
            .map_err(|e| e.to_string())
            .and_then(|mut a| a.next().ok_or_else(|| "adresse introuvable".to_string()))
            .and_then(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(2)).map_err(|e| e.to_string()));
        match stream {
            Ok(s) => {
                crate::journal::info(&format!("Console Yamaha connectée ({target})"));
                hub.set_yamaha_status(|st| {
                    st.connected = true;
                    st.error.clear();
                });
                let err = session(&hub, &cfg, s, &rx, gen);
                crate::journal::warn(&format!("Console Yamaha déconnectée : {err}"));
                hub.set_yamaha_status(|st| {
                    st.connected = false;
                    st.error = err;
                });
            }
            Err(e) => hub.set_yamaha_status(|st| {
                st.connected = false;
                st.error = format!("connexion impossible à {target} ({e})");
            }),
        }
        // Les demandes faites pendant la coupure sont perdues : l'iPad affichera l'état réel au retour.
        while rx.try_recv().is_ok() {}
        let until = Instant::now() + Duration::from_secs(2);
        while Instant::now() < until && hub.generation.load(Ordering::Relaxed) == gen {
            std::thread::sleep(Duration::from_millis(100));
        }
    }
}

/// File d'envoi : les « set » passent devant, un « set » remplace le précédent sur le même
/// paramètre (un fader qu'on glisse n'empile pas cent valeurs).
#[derive(Default)]
struct Queue {
    sets: VecDeque<(String, u32, u32, String)>,
    gets: VecDeque<String>,
}

impl Queue {
    fn set(&mut self, addr: String, x: u32, y: u32, value: &Value) {
        let v = match value {
            Value::String(s) => format!("\"{}\"", s.replace('"', "")),
            Value::Bool(b) => (*b as i32).to_string(),
            Value::Number(n) => n.as_i64().map(|i| i.to_string()).unwrap_or_else(|| n.as_f64().map(|f| (f.round() as i64).to_string()).unwrap_or_default()),
            _ => return,
        };
        let line = format!("set {addr} {x} {y} {v}");
        if let Some(slot) = self.sets.iter_mut().find(|s| s.0 == addr && s.1 == x && s.2 == y) {
            slot.3 = line;
        } else {
            self.sets.push_back((addr, x, y, line));
        }
    }
    fn get(&mut self, addr: &str, x: u32, y: u32) {
        self.gets.push_back(format!("get {addr} {x} {y}"));
    }
    fn pop(&mut self) -> Option<String> {
        self.sets.pop_front().map(|s| s.3).or_else(|| self.gets.pop_front())
    }
}

fn session(hub: &Hub, cfg: &YamahaConfig, mut stream: TcpStream, rx: &Receiver<Out>, gen: u64) -> String {
    let _ = stream.set_nodelay(true);
    if let Err(e) = stream.set_read_timeout(Some(Duration::from_millis(10))) {
        return e.to_string();
    }
    let mut q = Queue::default();
    q.gets.push_back("devinfo productname".into());
    for ch in 0..cfg.channels {
        for a in ["Label/Name", "Fader/Level", "Fader/On"] {
            q.get(&format!("{IN}/{a}"), ch, 0);
        }
    }
    for m in 0..cfg.mixes {
        for a in ["Label/Name", "Fader/Level", "Fader/On"] {
            q.get(&format!("{MIX}/{a}"), m, 0);
        }
    }
    if cfg.meters.mode == MeterMode::Subscribe {
        for c in &cfg.meters.subscribe {
            q.gets.push_back(c.clone());
        }
    }
    let mut pending: Vec<u8> = Vec::new();
    let mut buf = [0u8; 8192];
    let mut last_poll = Instant::now();
    let mut last_rx = Instant::now();
    loop {
        if hub.generation.load(Ordering::Relaxed) != gen {
            return "config rechargée".into();
        }
        loop {
            match rx.try_recv() {
                Ok(Out::Set { addr, x, y, value }) => {
                    // L'iPad voit tout de suite la nouvelle valeur ; la console confirme par « OK set ».
                    hub.set_param(&addr, x, y, value.clone(), None);
                    q.set(addr, x, y, &value);
                }
                Ok(Out::LoadChannel(ch)) => {
                    q.get(&format!("{IN}/PEQ/On"), ch, 0);
                    // Adresses du HPF et du type de bande : supposées (à vérifier sur une vraie CL/QL).
                    q.get(&format!("{IN}/HPF/On"), ch, 0);
                    q.get(&format!("{IN}/HPF/Freq"), ch, 0);
                    for band in 0..4 {
                        for p in ["Gain", "Freq", "Q", "Type"] {
                            q.get(&format!("{IN}/PEQ/Band/{p}"), ch, band);
                        }
                    }
                    for m in 0..cfg.mixes {
                        q.get(&format!("{IN}/ToMix/Level"), ch, m);
                        q.get(&format!("{IN}/ToMix/On"), ch, m);
                    }
                }
                Ok(Out::LoadSends(m)) => {
                    for ch in 0..cfg.channels {
                        q.get(&format!("{IN}/ToMix/Level"), ch, m);
                        q.get(&format!("{IN}/ToMix/On"), ch, m);
                    }
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return "arrêt".into(),
            }
        }
        if cfg.meters.mode == MeterMode::Poll && last_poll.elapsed() >= Duration::from_millis(cfg.meters.poll_ms.max(20)) && q.gets.len() < 50 {
            last_poll = Instant::now();
            for ch in 0..cfg.channels {
                q.get(&cfg.meters.poll_inch, ch, 0);
            }
            for m in 0..cfg.mixes {
                q.get(&cfg.meters.poll_mix, m, 0);
            }
        }
        // Au plus 16 lignes toutes les ~10 ms, pour ne pas noyer la console.
        let mut out = String::new();
        for _ in 0..16 {
            match q.pop() {
                Some(l) => {
                    out.push_str(&l);
                    out.push('\n');
                }
                None => break,
            }
        }
        if !out.is_empty() {
            if let Err(e) = stream.write_all(out.as_bytes()) {
                return format!("envoi impossible ({e})");
            }
        }
        match stream.read(&mut buf) {
            Ok(0) => return "la console a fermé la connexion".into(),
            Ok(n) => {
                last_rx = Instant::now();
                pending.extend_from_slice(&buf[..n]);
                while let Some(pos) = pending.iter().position(|b| *b == b'\n') {
                    let raw: Vec<u8> = pending.drain(..=pos).collect();
                    let line = String::from_utf8_lossy(&raw).trim().to_string();
                    if !line.is_empty() {
                        handle_line(hub, cfg, &line);
                    }
                }
            }
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {
                // Silence prolongé : on vérifie que la console répond encore.
                if last_rx.elapsed() > Duration::from_secs(10) && q.gets.is_empty() {
                    q.gets.push_back("devinfo productname".into());
                    last_rx = Instant::now() - Duration::from_secs(5);
                }
            }
            Err(e) => return format!("connexion perdue ({e})"),
        }
    }
}

/// Mots de la ligne ; un texte entre guillemets compte pour un mot (et garde ses guillemets
/// en tête pour qu'on le reconnaisse).
pub fn tokens(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quoted = false;
    let mut started = false;
    for c in line.chars() {
        match c {
            '"' => {
                if !quoted && !started {
                    cur.push('"');
                }
                quoted = !quoted;
                started = true;
            }
            c if c.is_whitespace() && !quoted => {
                if started {
                    out.push(std::mem::take(&mut cur));
                    started = false;
                }
            }
            c => {
                cur.push(c);
                started = true;
            }
        }
    }
    if started {
        out.push(cur);
    }
    out
}

fn value_of(tok: &str) -> Value {
    if let Some(s) = tok.strip_prefix('"') {
        return Value::String(s.to_string());
    }
    tok.parse::<i64>().map(Value::from).unwrap_or_else(|_| Value::String(tok.to_string()))
}

fn unquote(tok: &str) -> String {
    tok.trim_start_matches('"').to_string()
}

#[derive(Debug, PartialEq)]
pub enum Line {
    Param { addr: String, x: u32, y: u32, value: Value, text: Option<String> },
    Model(String),
    /// Relevé de niveaux : adresse et valeurs brutes, à partir de la voie `start`.
    Meter { addr: String, start: u32, values: Vec<f64> },
    Error(String),
}

pub fn parse_line(line: &str) -> Option<Line> {
    let t = tokens(line);
    let t: Vec<&str> = t.iter().map(String::as_str).collect();
    match t.as_slice() {
        ["NOTIFY" | "OK", "set" | "get", addr, x, y, value, rest @ ..] => Some(Line::Param {
            addr: addr.to_string(),
            x: x.parse().ok()?,
            y: y.parse().ok()?,
            value: value_of(value),
            text: rest.first().map(|s| unquote(s)),
        }),
        ["OK", "devinfo", "productname", name, ..] => Some(Line::Model(unquote(name))),
        // « NOTIFY mtr <adresse> [type] v0 v1 … » : les mots non numériques sont ignorés.
        ["NOTIFY" | "OK", "mtr", addr, rest @ ..] => {
            let values: Vec<f64> = rest.iter().filter_map(|s| s.parse::<f64>().ok()).collect();
            Some(Line::Meter { addr: addr.to_string(), start: 0, values })
        }
        ["ERROR", ..] => Some(Line::Error(line.to_string())),
        _ => None,
    }
}

fn norm(cfg: &YamahaConfig, raw: f64) -> u8 {
    let (lo, hi) = (cfg.meters.raw_min, cfg.meters.raw_max);
    if hi <= lo {
        return 0;
    }
    (((raw - lo) / (hi - lo)).clamp(0.0, 1.0) * 100.0).round() as u8
}

fn handle_line(hub: &Hub, cfg: &YamahaConfig, line: &str) {
    match parse_line(line) {
        Some(Line::Param { addr, x, y, value, text }) => {
            let m = &cfg.meters;
            if m.mode == MeterMode::Poll && (addr == m.poll_inch || addr == m.poll_mix) {
                let raw = value.as_f64().unwrap_or(m.raw_min);
                let v = (x as usize, vec![norm(cfg, raw)]);
                if addr == m.poll_inch {
                    hub.set_meters(Some(v), None);
                } else {
                    hub.set_meters(None, Some(v));
                }
            } else {
                hub.set_param(&addr, x, y, value, text);
            }
        }
        Some(Line::Model(name)) => hub.set_yamaha_status(|s| s.model = name),
        Some(Line::Meter { addr, start, values }) => {
            let v: Vec<u8> = values.iter().map(|r| norm(cfg, *r)).collect();
            if addr.contains("Mix") {
                hub.set_meters(None, Some((start as usize, v)));
            } else {
                hub.set_meters(Some((start as usize, v)), None);
            }
        }
        Some(Line::Error(e)) => {
            star_core::log::debug(&format!("Console : {e}"));
            hub.set_yamaha_status(|s| s.last_error_line = e);
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_lines() {
        assert_eq!(
            parse_line(r#"NOTIFY set MIXER:Current/InCh/Fader/On 31 0 1 "ON""#),
            Some(Line::Param { addr: "MIXER:Current/InCh/Fader/On".into(), x: 31, y: 0, value: json!(1), text: Some("ON".into()) })
        );
        assert_eq!(
            parse_line(r#"OK get MIXER:Current/InCh/Label/Name 0 0 "Kick In""#),
            Some(Line::Param { addr: "MIXER:Current/InCh/Label/Name".into(), x: 0, y: 0, value: json!("Kick In"), text: None })
        );
        assert_eq!(parse_line(r#"OK devinfo productname "QL1""#), Some(Line::Model("QL1".into())));
        assert_eq!(
            parse_line("NOTIFY mtr MIXER:Current/Meter/InCh PostOn 0 64 127"),
            Some(Line::Meter { addr: "MIXER:Current/Meter/InCh".into(), start: 0, values: vec![0.0, 64.0, 127.0] })
        );
        assert!(matches!(parse_line("ERROR set InvalidArgument"), Some(Line::Error(_))));
    }

    #[test]
    fn queue_coalesces_sets() {
        let mut q = Queue::default();
        q.get("A", 0, 0);
        q.set("F".into(), 1, 0, &json!(-1000));
        q.set("F".into(), 1, 0, &json!(-500));
        q.set("N".into(), 1, 0, &json!("Voix \"1\""));
        assert_eq!(q.pop().as_deref(), Some("set F 1 0 -500"));
        assert_eq!(q.pop().as_deref(), Some("set N 1 0 \"Voix 1\""));
        assert_eq!(q.pop().as_deref(), Some("get A 0 0"));
    }
}
