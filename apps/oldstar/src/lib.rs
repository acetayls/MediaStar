//! OldStar : télécommande web pour vieilles tablettes (iOS 9+, Android 4.4+).
//!
//! Le PC fait tourner cet exécutable ; la tablette ouvre `http://<ip du PC>:<port>/`.
//!
//! ```text
//! oldstar [--console yamaha|m32|aucune] [--lang en|fr] [--simulateur [port]] [--simulateur-m32 [port]]
//!         [--config <fichier>] [--port <port http>] [--sans-fenetre]
//! ```
//!
//! - `--console` choisit la console pour ce lancement (sinon celle de oldstar.json).
//! - `--simulateur` lance une fausse console (QL1, ou M32 si la console choisie est la M32)
//!   sur ce PC et y connecte OldStar, pour essayer sans matériel ; `--simulateur-m32` force
//!   la M32.
//! - `--sans-fenetre` : pas de fenêtre, le journal s'écrit dans la console.
//!
//! Bibliothèque : `run` est appelée par l'exe `oldstar` et par l'exe `mediastar` (`--old`).

mod aide_web;
mod capture;
mod config;
mod frames;
mod hub;
mod journal;
mod midas;
mod osc;
mod server;
mod simulator;
mod simulator_m32;
mod ui;
mod yamaha;

use std::path::PathBuf;
use std::sync::Arc;

/// Version d'OldStar (indépendante de celle de MediaStar).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub static APP: star_core::app::App = star_core::app::App::new("OldStar");

struct Args {
    simulator: Option<u16>,
    console: Option<config::ConsoleKind>,
    config: Option<PathBuf>,
    port: Option<u16>,
    no_window: bool,
    lang: Option<star_core::i18n::Lang>,
}

const USAGE: &str = "oldstar [--console yamaha|m32|aucune] [--lang en|fr] [--simulateur [port]] [--simulateur-m32 [port]] [--config <fichier>] [--port <port http>] [--sans-fenetre]";

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut a = Args { simulator: None, console: None, config: None, port: None, no_window: false, lang: None };
    let mut it = args.iter().skip(1).cloned().peekable();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--simulateur" | "--simulator" => {
                let port = it.peek().and_then(|p| p.parse::<u16>().ok());
                if port.is_some() {
                    it.next();
                }
                a.simulator = Some(port.unwrap_or(0));
            }
            "--simulateur-m32" | "--simulator-m32" => {
                let port = it.peek().and_then(|p| p.parse::<u16>().ok());
                if port.is_some() {
                    it.next();
                }
                a.simulator = Some(port.unwrap_or(0));
                a.console = Some(config::ConsoleKind::M32);
            }
            "--console" => a.console = Some(it.next().ok_or("--console attend yamaha, m32 ou aucune")?.parse()?),
            "--lang" => {
                a.lang = Some(match it.next().as_deref() {
                    Some("en") => star_core::i18n::Lang::En,
                    Some("fr") => star_core::i18n::Lang::Fr,
                    _ => return Err("--lang attend en ou fr".into()),
                })
            }
            "--config" => a.config = Some(PathBuf::from(it.next().ok_or("--config attend un fichier")?)),
            "--port" => a.port = Some(it.next().and_then(|p| p.parse().ok()).ok_or("--port attend un numéro")?),
            "--sans-fenetre" | "--headless" => a.no_window = true,
            "-h" | "--help" | "/?" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("argument inconnu : {other}\n{USAGE}")),
        }
    }
    Ok(a)
}

/// Lance OldStar selon ses arguments (`args[0]` est le chemin de l'exe, comme `std::env::args`).
/// Ne rend pas la main : OldStar termine le processus à la fermeture de sa fenêtre.
pub fn run(args: &[String]) {
    star_core::log::init(APP);
    star_core::log::install_panic_hook(&format!("Démarrage d'OldStar {}", env!("CARGO_PKG_VERSION")));
    let args = match parse_args(args) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    journal::set_console(args.no_window || cfg!(debug_assertions));
    capture::dpi_aware();
    journal::info(&format!("OldStar {} ({} du {})", env!("CARGO_PKG_VERSION"), env!("STAR_GIT"), env!("STAR_BUILD_DATE")));

    let path = args.config.clone().or_else(config::default_path).unwrap_or_else(|| PathBuf::from("oldstar.json"));
    let mut cfg = match config::Config::load_or_create(&path) {
        Ok(c) => c,
        Err(e) => {
            journal::warn(&format!("Réglages illisibles, valeurs par défaut utilisées : {e}"));
            config::Config::default()
        }
    };
    journal::info(&format!("Réglages : {}", path.display()));
    if let Some(p) = args.port {
        cfg.http_port = p;
    }
    if let Some(c) = args.console {
        cfg.console = c;
    }
    // Langue : celle des réglages, sauf si --lang la donne (MediaStar, essais).
    if let Some(l) = args.lang {
        cfg.language = l;
    }
    star_core::i18n::set_lang(cfg.language);
    if let (Some(_), config::ConsoleKind::None) = (args.simulator, cfg.console) {
        // --simulateur sans console choisie : la Yamaha simulée.
        cfg.console = config::ConsoleKind::Yamaha;
    }
    if let Some(p) = args.simulator {
        // Le simulateur tourne sur ce PC ; ces changements ne sont pas enregistrés.
        match cfg.console {
            config::ConsoleKind::Yamaha => {
                let port = if p == 0 { cfg.yamaha.port } else { p };
                cfg.yamaha.enabled = true;
                cfg.yamaha.host = "127.0.0.1".into();
                cfg.yamaha.port = port;
                std::thread::Builder::new().name("simulateur".into()).spawn(move || simulator::run(port)).expect("thread simulateur");
            }
            config::ConsoleKind::M32 => {
                let port = if p == 0 { cfg.m32.port } else { p };
                cfg.m32.host = "127.0.0.1".into();
                cfg.m32.port = port;
                std::thread::Builder::new().name("simulateur-m32".into()).spawn(move || simulator_m32::run(port)).expect("thread simulateur");
            }
            config::ConsoleKind::None => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    let port = cfg.http_port;
    let hub = Arc::new(hub::Hub::new(cfg, path));
    match hub.cfg().console {
        config::ConsoleKind::Yamaha => yamaha::start(hub.clone()),
        config::ConsoleKind::M32 => midas::start(hub.clone()),
        config::ConsoleKind::None => journal::info("Aucune console : onglets Applis et Fenêtres seulement"),
    }
    osc::start(hub.clone());
    let frames = Arc::new(frames::Frames::default());

    let server_error = match server::bind(port) {
        Ok(listener) => {
            server::log_addresses(port);
            let (hub, frames) = (hub.clone(), frames.clone());
            std::thread::Builder::new().name("serveur".into()).spawn(move || server::serve(listener, hub, frames)).expect("thread serveur");
            None
        }
        Err(e) => {
            journal::error(&e);
            if args.no_window {
                std::process::exit(1);
            }
            Some(e)
        }
    };

    if args.no_window {
        loop {
            std::thread::park();
        }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("OldStar")
            .with_inner_size([820.0, 520.0])
            .with_min_inner_size([560.0, 380.0])
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/oldstar-fenetre-256.png")).expect("icône PNG valide")),
        ..Default::default()
    };
    let res = eframe::run_native("OldStar", options, Box::new(move |cc| Ok(Box::new(ui::OldStarUi::new(cc, hub, port, server_error)))));
    if let Err(e) = res {
        journal::error(&format!("Fenêtre impossible ({e}) : relancez avec --sans-fenetre"));
        std::process::exit(1);
    }
    // Fenêtre fermée : on arrête tout (serveur, console, simulateur).
    journal::info("OldStar arrêté");
    std::process::exit(0);
}
