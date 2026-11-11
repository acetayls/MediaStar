//! Envoi des commandes OSC (UDP) vers CueStar et JingleStar, et retour d'état.
//!
//! Une seule socket : elle envoie les commandes et l'abonnement « <préfixe>/subscribe <port> »
//! (renouvelé toutes les 3 s, l'abonnement dure 5 s côté appli), et reçoit le retour d'état.

use rosc::{OscMessage, OscPacket, OscType};
use serde_json::Value;
use std::net::{SocketAddr, ToSocketAddrs, UdpSocket};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use crate::config::OscTarget;
use crate::hub::Hub;

static SOCKET: OnceLock<UdpSocket> = OnceLock::new();

fn socket() -> Option<&'static UdpSocket> {
    if let Some(s) = SOCKET.get() {
        return Some(s);
    }
    let s = UdpSocket::bind(("0.0.0.0", 0)).ok()?;
    let _ = SOCKET.set(s);
    SOCKET.get()
}

fn resolve(t: &OscTarget) -> Option<SocketAddr> {
    (t.host.trim(), t.port).to_socket_addrs().ok()?.find(|a| a.is_ipv4())
}

pub fn full_address(t: &OscTarget, address: &str) -> String {
    let prefix = t.prefix.trim().trim_end_matches('/');
    let prefix = if prefix.is_empty() || prefix.starts_with('/') { prefix.to_string() } else { format!("/{prefix}") };
    format!("{prefix}/{}", address.trim().trim_start_matches('/'))
}

pub fn arg_of(v: &Value) -> Option<OscType> {
    Some(match v {
        Value::Bool(b) => OscType::Bool(*b),
        Value::String(s) => OscType::String(s.clone()),
        Value::Number(n) if n.is_i64() => OscType::Int(n.as_i64()? as i32),
        Value::Number(n) => OscType::Float(n.as_f64()? as f32),
        _ => return None,
    })
}

pub fn send(t: &OscTarget, address: &str, args: &[Value]) -> Result<(), String> {
    let sock = socket().ok_or("socket UDP indisponible")?;
    let to = resolve(t).ok_or_else(|| format!("adresse introuvable : {}", t.host))?;
    let msg = OscMessage { addr: full_address(t, address), args: args.iter().filter_map(arg_of).collect() };
    let bytes = rosc::encoder::encode(&OscPacket::Message(msg)).map_err(|e| e.to_string())?;
    sock.send_to(&bytes, to).map_err(|e| e.to_string())?;
    Ok(())
}

/// Lance l'abonnement et la réception du retour d'état.
pub fn start(hub: Arc<Hub>) {
    let Some(sock) = socket() else {
        crate::journal::warn("OSC : socket UDP indisponible");
        return;
    };
    let _ = sock.set_read_timeout(Some(Duration::from_millis(250)));
    std::thread::Builder::new()
        .name("osc".into())
        .spawn(move || {
            let mut buf = vec![0u8; 65536];
            let mut last_sub = Instant::now() - Duration::from_secs(10);
            let mut last_rx: Vec<Option<Instant>> = Vec::new();
            loop {
                let targets = hub.cfg().osc;
                last_rx.resize(targets.len(), None);
                if last_sub.elapsed() >= Duration::from_secs(3) {
                    last_sub = Instant::now();
                    let port = sock.local_addr().map(|a| a.port()).unwrap_or(0) as i32;
                    for t in targets.iter().filter(|t| t.subscribe) {
                        let _ = send(t, "subscribe", &[Value::from(port)]);
                    }
                }
                if let Ok((n, from)) = sock.recv_from(&mut buf) {
                    if let Ok((_, packet)) = rosc::decoder::decode_udp(&buf[..n]) {
                        let mut msgs = Vec::new();
                        flatten(packet, &mut msgs);
                        for m in msgs {
                            receive(&hub, &targets, from, m, &mut last_rx);
                        }
                    }
                }
                let alive = last_rx.iter().map(|t| t.is_some_and(|t| t.elapsed() < Duration::from_secs(7))).collect();
                hub.set_osc_alive(alive);
            }
        })
        .expect("thread osc");
}

fn flatten(p: OscPacket, out: &mut Vec<OscMessage>) {
    match p {
        OscPacket::Message(m) => out.push(m),
        OscPacket::Bundle(b) => b.content.into_iter().for_each(|p| flatten(p, out)),
    }
}

fn receive(hub: &Hub, targets: &[OscTarget], from: SocketAddr, m: OscMessage, last_rx: &mut [Option<Instant>]) {
    for (i, t) in targets.iter().enumerate() {
        let prefix = full_address(t, "");
        let Some(rest) = m.addr.strip_prefix(prefix.as_str()) else { continue };
        if resolve(t).is_some_and(|a| a.port() != from.port()) {
            continue;
        }
        last_rx[i] = Some(Instant::now());
        if t.feedback.iter().any(|f| f.address.trim_start_matches('/') == rest) {
            let v = match m.args.first() {
                Some(OscType::String(s)) => Value::from(s.clone()),
                Some(OscType::Int(n)) => Value::from(*n),
                Some(OscType::Float(f)) => Value::from((*f as f64 * 10.0).round() / 10.0),
                Some(OscType::Double(f)) => Value::from((*f * 10.0).round() / 10.0),
                Some(OscType::Bool(b)) => Value::from(*b),
                _ => Value::Null,
            };
            hub.set_osc_value(i, rest, v);
        }
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses() {
        let mut t = OscTarget { prefix: "/cuestar".into(), ..Default::default() };
        assert_eq!(full_address(&t, "cuelist/go"), "/cuestar/cuelist/go");
        t.prefix = "jinglestar/".into();
        assert_eq!(full_address(&t, "/go"), "/jinglestar/go");
        t.prefix = String::new();
        assert_eq!(full_address(&t, "go"), "/go");
    }
}
