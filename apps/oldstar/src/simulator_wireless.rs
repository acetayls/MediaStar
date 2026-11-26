//! Faux récepteurs HF (`oldstar --simulateur-micros`) pour essayer l'onglet Micros sans matériel :
//! un Sennheiser EM 2000 (UDP) et un Shure AD4D à deux voies (TCP), qui répondent comme décrit
//! dans wireless.rs. Les niveaux bougent, la pile descend, une voie perd parfois son émetteur.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, UdpSocket};
use std::time::{Duration, Instant};

/// Petit bruit déterministe, suffisant pour faire bouger les niveaux.
struct Noise(u32);
impl Noise {
    fn next(&mut self, max: u32) -> u32 {
        self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (self.0 >> 16) % max.max(1)
    }
}

/// Sennheiser EM 2000 : répond à Push, Name, Frequency, Mute ; attributs cycliques au rythme demandé.
pub fn run_sennheiser(port: u16) {
    let sock = match UdpSocket::bind(("0.0.0.0", port)) {
        Ok(s) => s,
        Err(e) => {
            crate::journal::error(&format!("Simulateur Sennheiser : port UDP {port} indisponible ({e})"));
            return;
        }
    };
    let _ = sock.set_read_timeout(Some(Duration::from_millis(20)));
    crate::journal::info(&format!("Simulateur de récepteur Sennheiser EM 2000 à l'écoute sur le port UDP {port}"));
    let mut client: Option<(SocketAddr, Instant, Duration, Duration)> = None; // adresse, fin, cycle
    let mut last = Instant::now();
    let mut name = String::from("Chant");
    let mut mute = 0;
    let start = Instant::now();
    let mut rnd = Noise(7);
    let mut buf = [0u8; 1600];
    loop {
        if let Ok((n, from)) = sock.recv_from(&mut buf) {
            let text = String::from_utf8_lossy(&buf[..n]).into_owned();
            for line in text.split('\r').map(str::trim).filter(|l| !l.is_empty()) {
                let mut it = line.split_whitespace();
                let reply = match it.next() {
                    Some("Push") => {
                        let v: Vec<u64> = it.filter_map(|s| s.parse().ok()).collect();
                        if v.len() != 3 {
                            format!("1040: Invalid numbers of parameter [ {line} ]\r")
                        } else {
                            client = Some((from, Instant::now() + Duration::from_secs(v[0]), Duration::from_millis(v[1].max(100)), Duration::ZERO));
                            let mut r = format!("{line}\r");
                            if v[2] > 0 {
                                r.push_str(&format!("Name {name}\rFrequency 606500 0 0\rMute {mute}\r"));
                            }
                            r
                        }
                    }
                    Some("Name") => {
                        let rest = line[4..].trim();
                        if !rest.is_empty() {
                            name = rest.to_string();
                        }
                        format!("Name {name}\r")
                    }
                    Some("Frequency") => "Frequency 606500 0 0\r".to_string(),
                    Some("Mute") => {
                        if let Some(v) = it.next().and_then(|s| s.parse().ok()) {
                            mute = v;
                        }
                        format!("Mute {mute}\r")
                    }
                    _ => format!("1000: Invalid command [ {line} ]\r"),
                };
                let _ = sock.send_to(reply.as_bytes(), from);
            }
        }
        if let Some((to, until, cycle, _)) = client {
            if Instant::now() > until {
                client = None;
            } else if last.elapsed() >= cycle {
                last = Instant::now();
                let t = start.elapsed().as_secs();
                // Toutes les 40 s, l'émetteur s'éteint 6 s.
                let tx = t % 40 >= 6;
                let bat = match t / 60 % 4 {
                    0 => "100",
                    1 => "70",
                    2 => "30",
                    _ => "0",
                };
                let (a, b) = if tx { (60 + rnd.next(40), 40 + rnd.next(40)) } else { (rnd.next(5), rnd.next(5)) };
                let af = if tx { 20 + rnd.next(70) } else { 0 };
                let mut warn = vec![];
                if bat == "0" {
                    warn.push("Low_Battery");
                }
                if !tx {
                    warn.push("Low_RF_Signal");
                    warn.push("RF_Mute");
                }
                if af > 85 {
                    warn.push("AF_Peak");
                }
                let msg = if warn.is_empty() { "OK".to_string() } else { warn.join(" ") };
                let mute_bits = if tx { 0 } else { 0b101 } | if mute == 1 { 0b1001 } else { 0 };
                let text = format!(
                    "RF1 {} {} {}\rRF2 {} {} {}\rStates {} {}\rRF {} {} {}\rAF {af} {} {mute_bits}\rBat {}\rMsg {msg}\rConfig 1\r",
                    a / 2,
                    a,
                    (a >= b) as u8,
                    b / 2,
                    b,
                    (b > a) as u8,
                    mute_bits,
                    tx as u8,
                    a.max(b),
                    if a >= b { 1 } else { 2 },
                    tx as u8,
                    (af + 8).min(100),
                    if tx { bat } else { "?" },
                );
                let _ = sock.send_to(text.as_bytes(), to);
            }
        }
    }
}

/// Shure AD4D : deux voies, REP à la connexion et sur GET, SAMPLE au rythme de METER_RATE.
pub fn run_shure(port: u16) {
    let listener = match TcpListener::bind(("0.0.0.0", port)) {
        Ok(l) => l,
        Err(e) => {
            crate::journal::error(&format!("Simulateur Shure : port TCP {port} indisponible ({e})"));
            return;
        }
    };
    crate::journal::info(&format!("Simulateur de récepteur Shure AD4D à l'écoute sur le port TCP {port}"));
    for stream in listener.incoming().flatten() {
        std::thread::spawn(move || shure_client(stream));
    }
}

fn shure_client(mut s: std::net::TcpStream) {
    let _ = s.set_read_timeout(Some(Duration::from_millis(20)));
    let names = ["Présentateur", "Invité"];
    let freqs = ["0578350", "0581125"];
    let mut rate = [0u64; 3];
    let mut last = [Instant::now(); 3];
    let start = Instant::now();
    let mut rnd = Noise(11);
    let mut pending = String::new();
    let mut buf = [0u8; 2048];
    let pad = |v: &str, n: usize| format!("{{{v:<n$}}}");
    loop {
        match s.read(&mut buf) {
            Ok(0) => return,
            Ok(n) => pending.push_str(&String::from_utf8_lossy(&buf[..n])),
            Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
            Err(_) => return,
        }
        let mut out = String::new();
        while let (Some(a), Some(b)) = (pending.find('<'), pending.find('>')) {
            let msg: Vec<String> = pending[a + 1..b.max(a + 1)].split_whitespace().map(String::from).collect();
            pending.drain(..=b.max(a));
            let w: Vec<&str> = msg.iter().map(String::as_str).collect();
            match w.as_slice() {
                ["GET", "MODEL"] => out.push_str(&format!("< REP MODEL {} >", pad("AD4D", 32))),
                ["GET", "0", "ALL"] => {
                    out.push_str(&format!("< REP MODEL {} >", pad("AD4D", 32)));
                    for ch in 1..=2usize {
                        let t = start.elapsed().as_secs();
                        out.push_str(&format!("< REP {ch} CHAN_NAME {} >", pad(names[ch - 1], 31)));
                        out.push_str(&format!("< REP {ch} FREQUENCY {} >", freqs[ch - 1]));
                        out.push_str(&format!("< REP {ch} AUDIO_MUTE OFF >"));
                        out.push_str(&format!("< REP {ch} TX_MODEL AD2 >"));
                        out.push_str(&format!("< REP {ch} TX_BATT_BARS {:03} >", 5 - (t / 60 % 5)));
                        out.push_str(&format!("< REP {ch} TX_BATT_MINS {:05} >", 300 - t / 60));
                        out.push_str(&format!("< REP {ch} TX_BATT_CHARGE_PERCENT {:03} >", 100 - (t / 30).min(100)));
                        out.push_str(&format!("< REP {ch} INTERFERENCE_STATUS NONE >"));
                    }
                }
                ["SET", ch, "METER_RATE", v] => {
                    if let (Ok(c), Ok(v)) = (ch.parse::<usize>(), v.parse::<u64>()) {
                        if (1..=2).contains(&c) {
                            rate[c] = v;
                            out.push_str(&format!("< REP {c} METER_RATE {v:05} >"));
                        } else {
                            out.push_str("< REP ERR >");
                        }
                    }
                }
                _ => out.push_str("< REP ERR >"),
            }
        }
        for ch in 1..=2usize {
            if rate[ch] >= 100 && last[ch].elapsed() >= Duration::from_millis(rate[ch]) {
                last[ch] = Instant::now();
                let t = start.elapsed().as_secs();
                // La voie 2 perd son émetteur 5 s toutes les 30 s.
                let tx = ch == 1 || t % 30 >= 5;
                let peak = if tx { 70 + rnd.next(45) } else { 0 };
                let rms = peak.saturating_sub(10);
                let bitmap = if peak >= 114 { 0xff } else { (1u32 << (peak.saturating_sub(60) / 7).min(7)) - 1 };
                let (ra, rb) = if tx { (50 + rnd.next(35), 45 + rnd.next(35)) } else { (rnd.next(15), rnd.next(15)) };
                let leds = |r: u32| (1u32 << (r / 17).min(5)) - 1;
                out.push_str(&format!(
                    "< SAMPLE {ch} ALL 005 {bitmap:03} {peak:03} {rms:03} {}{} {:02} {ra:03} {:02} {rb:03} >",
                    if ra >= rb { 'B' } else { 'X' },
                    if rb > ra { 'B' } else { 'X' },
                    leds(ra),
                    leds(rb)
                ));
                if t % 30 == 0 || t % 30 == 5 {
                    out.push_str(&format!("< REP {ch} TX_MODEL {} >", if tx { "AD2" } else { "UNKNOWN" }));
                }
            }
        }
        if !out.is_empty() && s.write_all(out.as_bytes()).is_err() {
            return;
        }
    }
}
