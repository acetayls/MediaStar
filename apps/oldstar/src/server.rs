//! Serveur de la page : HTTP, WebSocket (RFC 6455) et long-polling sur un seul port.
//! Protocole : docs/protocole.md. La page est embarquée dans l'exécutable.
//!
//! Volontairement simple : un thread par connexion (quelques tablettes au plus), une requête
//! par connexion HTTP (« Connection: close »), sauf le WebSocket qui reste ouvert.

use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::frames::{Frames, Target};
use crate::hub::Hub;
use crate::yamaha::Out;

/// Fichiers de la page : chemin → (type, contenu).
const FILES: &[(&str, &str, &str)] = &[
    ("/", "text/html; charset=utf-8", include_str!("../web/index.html")),
    ("/index.html", "text/html; charset=utf-8", include_str!("../web/index.html")),
    ("/oldstar.css", "text/css; charset=utf-8", include_str!("../web/oldstar.css")),
    ("/js/util.js", "application/javascript; charset=utf-8", include_str!("../web/js/util.js")),
    ("/js/transport.js", "application/javascript; charset=utf-8", include_str!("../web/js/transport.js")),
    ("/js/eq.js", "application/javascript; charset=utf-8", include_str!("../web/js/eq.js")),
    ("/js/console.js", "application/javascript; charset=utf-8", include_str!("../web/js/console.js")),
    ("/js/applis.js", "application/javascript; charset=utf-8", include_str!("../web/js/applis.js")),
    ("/js/fenetres.js", "application/javascript; charset=utf-8", include_str!("../web/js/fenetres.js")),
    ("/js/aide.js", "application/javascript; charset=utf-8", include_str!("../web/js/aide.js")),
    ("/js/app.js", "application/javascript; charset=utf-8", include_str!("../web/js/app.js")),
];

/// Déplacements reçus depuis le dernier appui (résumé du glisser au journal).
static MOVES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Attente maximale d'une requête de long-polling.
const POLL_WAIT: Duration = Duration::from_secs(20);

/// Ouvre le port de la page (toutes les cartes réseau).
pub fn bind(port: u16) -> Result<TcpListener, String> {
    TcpListener::bind(("0.0.0.0", port)).map_err(|e| format!("Port {port} indisponible : {e}"))
}

/// Sert la page jusqu'à l'arrêt du programme (un thread par connexion).
pub fn serve(listener: TcpListener, hub: Arc<Hub>, frames: Arc<Frames>) {
    let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
    crate::journal::info(&format!("Page OldStar servie sur le port {port}"));
    for stream in listener.incoming().flatten() {
        let hub = hub.clone();
        let frames = frames.clone();
        let _ = std::thread::Builder::new().name("http".into()).spawn(move || {
            let _ = stream.set_nodelay(true);
            if let Err(e) = connection(&hub, &frames, stream) {
                star_core::log::debug(&format!("HTTP : {e}"));
            }
        });
    }
}

/// Adresses IPv4 du PC sur le réseau local : (nom de la carte, adresse).
pub fn addresses() -> Vec<(String, std::net::Ipv4Addr)> {
    let mut out = Vec::new();
    if let Ok(list) = if_addrs::get_if_addrs() {
        for i in list.iter().filter(|i| !i.is_loopback()) {
            if let std::net::IpAddr::V4(ip) = i.ip() {
                if !ip.is_link_local() && !out.iter().any(|(_, a)| *a == ip) {
                    out.push((i.name.clone(), ip));
                }
            }
        }
    }
    // Réseau privé d'abord (192.168…, 10…, 172.16-31…) : c'est celui des tablettes.
    out.sort_by_key(|(_, ip)| !ip.is_private());
    out
}

/// Adresses à taper, dans le journal.
pub fn log_addresses(port: u16) {
    crate::journal::info(&format!("Page OldStar sur ce PC : http://localhost:{port}/"));
    let list = addresses();
    for (name, ip) in &list {
        crate::journal::info(&format!("Page OldStar pour la tablette : http://{ip}:{port}/ ({name})"));
    }
    if list.is_empty() {
        crate::journal::warn("Aucune adresse réseau trouvée : le PC est-il branché au réseau ?");
    }
}

struct Request {
    method: String,
    path: String,
    query: HashMap<String, String>,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}

fn read_request(reader: &mut BufReader<TcpStream>) -> Result<Request, String> {
    let mut line = String::new();
    reader.read_line(&mut line).map_err(|e| e.to_string())?;
    let mut parts = line.split_whitespace();
    let method = parts.next().ok_or("requête vide")?.to_string();
    let target = parts.next().ok_or("requête sans chemin")?.to_string();
    let mut headers = HashMap::new();
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).map_err(|e| e.to_string())? == 0 {
            break;
        }
        let h = h.trim_end();
        if h.is_empty() {
            break;
        }
        if let Some((k, v)) = h.split_once(':') {
            headers.insert(k.trim().to_ascii_lowercase(), v.trim().to_string());
        }
        if headers.len() > 100 {
            return Err("trop d'en-têtes".into());
        }
    }
    let len: usize = headers.get("content-length").and_then(|v| v.parse().ok()).unwrap_or(0);
    if len > 1 << 20 {
        return Err("corps trop gros".into());
    }
    let mut body = vec![0; len];
    reader.read_exact(&mut body).map_err(|e| e.to_string())?;
    let (path, q) = target.split_once('?').unwrap_or((target.as_str(), ""));
    let query = q
        .split('&')
        .filter_map(|kv| kv.split_once('=').or(Some((kv, ""))))
        .filter(|(k, _)| !k.is_empty())
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    Ok(Request { method, path: path.to_string(), query, headers, body })
}

fn respond(mut s: &TcpStream, code: u16, ctype: &str, body: &[u8]) -> std::io::Result<()> {
    let reason = match code {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        404 => "Not Found",
        405 => "Method Not Allowed",
        _ => "Error",
    };
    let head = format!(
        "HTTP/1.1 {code} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes())?;
    s.write_all(body)?;
    s.flush()
}

fn connection(hub: &Arc<Hub>, frames: &Arc<Frames>, stream: TcpStream) -> Result<(), String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let req = read_request(&mut reader)?;
    let io = |r: std::io::Result<()>| r.map_err(|e| e.to_string());

    if req.path == "/ws" {
        if req.headers.get("upgrade").is_some_and(|u| u.eq_ignore_ascii_case("websocket")) {
            return websocket(hub, frames, stream, reader, &req);
        }
        return io(respond(&stream, 400, "text/plain; charset=utf-8", "WebSocket attendu".as_bytes()));
    }
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/poll") => {
            let msgs = poll(hub, &req);
            io(respond(&stream, 200, "application/json", json!({ "msgs": msgs }).to_string().as_bytes()))
        }
        ("POST", "/cmd") => {
            if let Ok(v) = serde_json::from_slice::<Value>(&req.body) {
                command(hub, frames, &v);
            }
            io(respond(&stream, 204, "text/plain", b""))
        }
        ("GET", "/frame") => {
            let target = match (req.query.get("h").and_then(|v| v.parse().ok()), req.query.get("w").and_then(|v| v.parse().ok())) {
                (Some(h), _) => Some(Target::Free(h)),
                (None, Some(w)) => Some(Target::Config(w)),
                _ => None,
            };
            let z = req.query.get("z").and_then(|v| v.parse().ok());
            match target.ok_or_else(|| "fenêtre manquante".to_string()).and_then(|t| frames.jpeg(hub, t, z)) {
                Ok(jpg) => io(respond(&stream, 200, "image/jpeg", &jpg)),
                Err(e) => io(respond(&stream, 404, "text/plain; charset=utf-8", e.as_bytes())),
            }
        }
        ("GET", "/windows") => {
            let free = hub.cfg().free_windows;
            let list: Vec<Value> = if free && crate::capture::AVAILABLE {
                let mut l = crate::capture::list();
                l.sort_by_key(|w| w.title.to_lowercase());
                l.iter().map(|w| json!({ "id": w.id, "title": w.title, "exe": w.exe, "min": w.minimized })).collect()
            } else {
                vec![]
            };
            let body = json!({ "available": crate::capture::AVAILABLE, "free": free, "list": list });
            io(respond(&stream, 200, "application/json", body.to_string().as_bytes()))
        }
        // Onglet Aide de la page : le manuel du PC, en HTML dans du JSON (`?en` : en anglais).
        ("GET", "/aide") => {
            let body = json!({ "html": crate::aide_web::html(req.query.contains_key("en")) });
            io(respond(&stream, 200, "application/json", body.to_string().as_bytes()))
        }
        // Icônes de la page (onglet du navigateur, écran d'accueil de l'iPad).
        ("GET", "/favicon.png" | "/apple-touch-icon.png" | "/apple-touch-icon-precomposed.png") => {
            io(respond(&stream, 200, "image/png", include_bytes!("../assets/icons/oldstar-256.png")))
        }
        ("GET", "/favicon.ico") => io(respond(&stream, 200, "image/x-icon", include_bytes!("../assets/icons/oldstar.ico"))),
        // Page : servie dans la langue des réglages (la page la lit dans <html lang>).
        ("GET", "/" | "/index.html") => {
            let lang = if hub.cfg().language == star_core::i18n::Lang::En { "en" } else { "fr" };
            let page = include_str!("../web/index.html").replacen("<html lang=\"fr\">", &format!("<html lang=\"{lang}\">"), 1);
            io(respond(&stream, 200, "text/html; charset=utf-8", page.as_bytes()))
        }
        ("GET", path) => match FILES.iter().find(|f| f.0 == path) {
            Some((_, ctype, body)) => io(respond(&stream, 200, ctype, body.as_bytes())),
            None => io(respond(&stream, 404, "text/plain; charset=utf-8", "introuvable".as_bytes())),
        },
        _ => io(respond(&stream, 405, "text/plain; charset=utf-8", "méthode non prise en charge".as_bytes())),
    }
}

/* ---------- Messages ---------- */

fn snap_msg(hub: &Hub) -> (u64, Value) {
    let d = hub.snapshot();
    let seq = d.get("seq").and_then(Value::as_u64).unwrap_or(0);
    (seq, json!({ "t": "snap", "seq": seq, "d": d }))
}

fn meter_fps(hub: &Hub) -> u32 {
    hub.cfg().meters().1
}

/// Commande de la page, quel que soit le transport (hello, snap et meters sont traités à part).
fn command(hub: &Hub, frames: &Frames, v: &Value) {
    let s = |k: &str| v.get(k).and_then(Value::as_str);
    let n = |k: &str| v.get(k).and_then(Value::as_u64);
    let f = |k: &str| v.get(k).and_then(Value::as_f64);
    match s("c") {
        Some("set") => {
            let (Some(a), Some(x), Some(y), Some(val)) = (s("a"), n("x"), n("y"), v.get("v")) else { return };
            // Seuls les paramètres de la console passent : la page ne peut pas envoyer n'importe quoi.
            if !a.starts_with("MIXER:Current/") || a.contains(char::is_whitespace) {
                return;
            }
            if let Ok(tx) = hub.yamaha_tx.lock() {
                if let Some(tx) = tx.as_ref() {
                    let _ = tx.send(Out::Set { addr: a.to_string(), x: x as u32, y: y as u32, value: val.clone() });
                }
            }
        }
        Some("load") => {
            let Some(ch) = n("ch") else { return };
            if let Ok(tx) = hub.yamaha_tx.lock() {
                if let Some(tx) = tx.as_ref() {
                    let _ = tx.send(Out::LoadChannel(ch as u32));
                }
            }
        }
        Some("loadsends") => {
            let Some(m) = n("mix") else { return };
            if let Ok(tx) = hub.yamaha_tx.lock() {
                if let Some(tx) = tx.as_ref() {
                    let _ = tx.send(Out::LoadSends(m as u32));
                }
            }
        }
        Some("osc") => {
            let cfg = hub.cfg();
            let (Some(t), Some(b)) = (n("t"), n("b")) else { return };
            let Some(target) = cfg.osc.get(t as usize) else { return };
            let Some(button) = target.buttons.get(b as usize) else { return };
            if let Err(e) = crate::osc::send(target, &button.address, &button.args) {
                crate::journal::warn(&format!("OSC vers {} : {e}", target.name));
            }
        }
        Some("input") => {
            let (Some(k), Some(x), Some(y)) = (s("k"), f("x"), f("y")) else { return };
            let target = match (v.get("h").and_then(Value::as_i64), n("w")) {
                (Some(h), _) => Target::Free(h as isize),
                (None, Some(w)) => Target::Config(w as usize),
                _ => return,
            };
            let z = n("z").map(|z| z as usize);
            let wheel = v.get("wheel").and_then(Value::as_i64).unwrap_or(0).clamp(-1200, 1200) as i32;
            // Journal des touchers (console) : utile pour comprendre un geste qui ne passe pas.
            match k {
                "down" => MOVES.store(0, Ordering::Relaxed),
                "move" => {
                    MOVES.fetch_add(1, Ordering::Relaxed);
                }
                _ => {}
            }
            if k == "up" {
                crate::journal::detail(&format!(
                    "Toucher : up {:?} x={x:.3} y={y:.3} : fin du glisser, {} déplacements reçus (page en mode {})",
                    target,
                    MOVES.load(Ordering::Relaxed),
                    s("m").unwrap_or("réglages")
                ));
            } else if k != "move" {
                crate::journal::detail(&format!("Toucher : {k} {:?} mode={} x={x:.3} y={y:.3} molette={wheel}", target, s("m").unwrap_or("réglages")));
            }
            if let Err(e) = frames.input(hub, target, z, s("m"), k, x, y, wheel) {
                star_core::log::debug(&format!("Toucher : {e}"));
            }
        }
        _ => {}
    }
}

/* ---------- Long-polling ---------- */

fn poll(hub: &Hub, req: &Request) -> Vec<Value> {
    let q = |k: &str| req.query.get(k).and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
    let since = q("since");
    let ms = q("ms");
    let meters = q("meters") == 1;
    if let Some(id) = req.query.get("id") {
        hub.poll_seen(id.chars().take(40).collect());
    }
    let mut msgs = Vec::new();
    let events = if since == 0 { None } else { hub.events_since(since) };
    match events {
        None => msgs.push(snap_msg(hub).1),
        Some((_, evs)) => {
            let has_meters = meters && hub.meters_since(ms).is_some();
            if evs.is_empty() && !has_meters {
                hub.wait(since, meters.then_some(ms), POLL_WAIT);
            }
            match hub.events_since(since) {
                Some((seq, evs)) if !evs.is_empty() => msgs.push(json!({ "t": "ev", "seq": seq, "d": evs })),
                Some(_) => {}
                None => msgs.push(snap_msg(hub).1),
            }
        }
    }
    if meters {
        if let Some((m, d)) = hub.meters_since(ms) {
            msgs.push(json!({ "t": "mtr", "ms": m, "d": d }));
        }
    }
    msgs
}

/* ---------- WebSocket ---------- */

#[derive(Default)]
struct WsState {
    /// Dernier événement envoyé ; `None` tant que le client n'a pas dit bonjour.
    seq: Option<u64>,
    want_snap: bool,
    meters: bool,
    alive: bool,
}

fn websocket(hub: &Arc<Hub>, frames: &Arc<Frames>, stream: TcpStream, mut reader: BufReader<TcpStream>, req: &Request) -> Result<(), String> {
    let key = req.headers.get("sec-websocket-key").ok_or("clé WebSocket manquante")?;
    let accept = base64(&sha1(format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11").as_bytes()));
    let head = format!("HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: {accept}\r\n\r\n");
    (&stream).write_all(head.as_bytes()).map_err(|e| e.to_string())?;
    let _ = stream.set_read_timeout(None);

    let writer = Arc::new(Mutex::new(stream.try_clone().map_err(|e| e.to_string())?));
    let state = Arc::new(Mutex::new(WsState { alive: true, ..Default::default() }));
    hub.ws_clients.fetch_add(1, Ordering::Relaxed);
    crate::journal::info(&format!("Tablette connectée : {}", stream.peer_addr().map(|a| a.to_string()).unwrap_or_default()));

    let pump = {
        let hub = hub.clone();
        let writer = writer.clone();
        let state = state.clone();
        std::thread::Builder::new().name("ws-envoi".into()).spawn(move || ws_pump(&hub, &writer, &state)).map_err(|e| e.to_string())?
    };

    let result = ws_read(hub, frames, &mut reader, &writer, &state);
    if let Ok(mut s) = state.lock() {
        s.alive = false;
    }
    let _ = stream.shutdown(std::net::Shutdown::Both);
    let _ = pump.join();
    hub.ws_clients.fetch_sub(1, Ordering::Relaxed);
    crate::journal::info("Tablette déconnectée");
    result
}

fn ws_send(writer: &Mutex<TcpStream>, opcode: u8, data: &[u8]) -> std::io::Result<()> {
    let mut frame = Vec::with_capacity(data.len() + 10);
    frame.push(0x80 | opcode);
    match data.len() {
        n if n < 126 => frame.push(n as u8),
        n if n < 65536 => {
            frame.push(126);
            frame.extend_from_slice(&(n as u16).to_be_bytes());
        }
        n => {
            frame.push(127);
            frame.extend_from_slice(&(n as u64).to_be_bytes());
        }
    }
    frame.extend_from_slice(data);
    let mut w = writer.lock().map_err(|_| std::io::Error::other("verrou"))?;
    w.write_all(&frame)?;
    w.flush()
}

fn ws_read(hub: &Hub, frames: &Frames, reader: &mut BufReader<TcpStream>, writer: &Mutex<TcpStream>, state: &Mutex<WsState>) -> Result<(), String> {
    let mut message: Vec<u8> = Vec::new();
    loop {
        let mut h = [0u8; 2];
        if reader.read_exact(&mut h).is_err() {
            return Ok(());
        }
        let fin = h[0] & 0x80 != 0;
        let opcode = h[0] & 0x0f;
        let masked = h[1] & 0x80 != 0;
        let mut len = (h[1] & 0x7f) as u64;
        if len == 126 {
            let mut b = [0u8; 2];
            reader.read_exact(&mut b).map_err(|e| e.to_string())?;
            len = u16::from_be_bytes(b) as u64;
        } else if len == 127 {
            let mut b = [0u8; 8];
            reader.read_exact(&mut b).map_err(|e| e.to_string())?;
            len = u64::from_be_bytes(b);
        }
        if len > 1 << 20 {
            return Err("trame trop grosse".into());
        }
        let mut mask = [0u8; 4];
        if masked {
            reader.read_exact(&mut mask).map_err(|e| e.to_string())?;
        }
        let mut data = vec![0u8; len as usize];
        reader.read_exact(&mut data).map_err(|e| e.to_string())?;
        if masked {
            for (i, b) in data.iter_mut().enumerate() {
                *b ^= mask[i % 4];
            }
        }
        match opcode {
            0x8 => {
                let _ = ws_send(writer, 0x8, &[]);
                return Ok(());
            }
            0x9 => {
                let _ = ws_send(writer, 0xA, &data);
                continue;
            }
            0xA => continue,
            0x0 | 0x1 | 0x2 => {
                message.extend_from_slice(&data);
                if message.len() > 1 << 20 {
                    return Err("message trop gros".into());
                }
                if !fin {
                    continue;
                }
            }
            _ => continue,
        }
        let text = std::mem::take(&mut message);
        let Ok(v) = serde_json::from_slice::<Value>(&text) else { continue };
        match v.get("c").and_then(Value::as_str) {
            Some("hello") => {
                let seq = v.get("seq").and_then(Value::as_u64).unwrap_or(0);
                let meters = v.get("meters").and_then(Value::as_bool).unwrap_or(false);
                if let Ok(mut s) = state.lock() {
                    s.meters = meters;
                    if seq > 0 && hub.events_since(seq).is_some() {
                        s.seq = Some(seq);
                    } else {
                        s.want_snap = true;
                    }
                }
            }
            Some("snap") => {
                if let Ok(mut s) = state.lock() {
                    s.want_snap = true;
                }
            }
            Some("meters") => {
                if let Ok(mut s) = state.lock() {
                    s.meters = v.get("on").and_then(Value::as_bool).unwrap_or(false);
                }
            }
            _ => command(hub, frames, &v),
        }
    }
}

/// Envoie au client les événements, l'instantané demandé et les vu-mètres (au plus `fps` par s).
fn ws_pump(hub: &Hub, writer: &Mutex<TcpStream>, state: &Mutex<WsState>) {
    let mut ms = 0u64;
    let mut last_meter = Instant::now() - Duration::from_secs(1);
    let mut last_ping = Instant::now();
    loop {
        let (alive, seq, want_snap, meters) = match state.lock() {
            Ok(s) => (s.alive, s.seq, s.want_snap, s.meters),
            Err(_) => return,
        };
        if !alive {
            return;
        }
        let mut sent = true;
        let res = (|| -> std::io::Result<()> {
            if want_snap {
                let (s, msg) = snap_msg(hub);
                ws_send(writer, 0x1, msg.to_string().as_bytes())?;
                if let Ok(mut st) = state.lock() {
                    st.want_snap = false;
                    st.seq = Some(s);
                }
                return Ok(());
            }
            if let Some(since) = seq {
                match hub.events_since(since) {
                    Some((s, evs)) if !evs.is_empty() => {
                        ws_send(writer, 0x1, json!({ "t": "ev", "seq": s, "d": evs }).to_string().as_bytes())?;
                        if let Ok(mut st) = state.lock() {
                            st.seq = Some(s);
                        }
                        return Ok(());
                    }
                    Some(_) => {}
                    None => {
                        if let Ok(mut st) = state.lock() {
                            st.want_snap = true;
                        }
                        return Ok(());
                    }
                }
            }
            let period = Duration::from_millis(1000 / meter_fps(hub) as u64);
            if meters && last_meter.elapsed() >= period {
                if let Some((m, d)) = hub.meters_since(ms) {
                    ms = m;
                    last_meter = Instant::now();
                    ws_send(writer, 0x1, json!({ "t": "mtr", "ms": m, "d": d }).to_string().as_bytes())?;
                    return Ok(());
                }
            }
            if last_ping.elapsed() >= Duration::from_secs(15) {
                last_ping = Instant::now();
                ws_send(writer, 0x9, b"")?;
            }
            sent = false;
            Ok(())
        })();
        if res.is_err() {
            if let Ok(mut s) = state.lock() {
                s.alive = false;
            }
            return;
        }
        if sent {
            continue;
        }
        // Rien à envoyer : on attend un événement, ou le prochain relevé des vu-mètres.
        let wait_meters = meters && last_meter.elapsed() >= Duration::from_millis(1000 / meter_fps(hub) as u64);
        let timeout = if meters && !wait_meters {
            Duration::from_millis(1000 / meter_fps(hub) as u64).saturating_sub(last_meter.elapsed()).max(Duration::from_millis(5))
        } else {
            // Court : un « hello » ou un changement de vu-mètres du client est pris en compte vite.
            Duration::from_millis(100)
        };
        hub.wait(seq.unwrap_or(u64::MAX), wait_meters.then_some(ms), timeout);
    }
}

/* ---------- SHA-1 et base64 (poignée de main WebSocket) ---------- */

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h: [u32; 5] = [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476, 0xC3D2E1F0];
    let mut msg = data.to_vec();
    let bits = (data.len() as u64).wrapping_mul(8);
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bits.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (i, wi) in w.iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let t = a.rotate_left(5).wrapping_add(f).wrapping_add(e).wrapping_add(k).wrapping_add(*wi);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = [0u8; 20];
    for (i, v) in h.iter().enumerate() {
        out[4 * i..4 * i + 4].copy_from_slice(&v.to_be_bytes());
    }
    out
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn websocket_accept_key() {
        // Exemple de la RFC 6455, section 1.3.
        let k = "dGhlIHNhbXBsZSBub25jZQ==258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
        assert_eq!(base64(&sha1(k.as_bytes())), "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b""), "");
    }

    #[test]
    fn page_files_are_embedded() {
        assert!(FILES.iter().any(|f| f.0 == "/" && f.2.contains("transport.js")));
        for (p, _, body) in FILES {
            assert!(!body.is_empty(), "{p} vide");
        }
    }
}
