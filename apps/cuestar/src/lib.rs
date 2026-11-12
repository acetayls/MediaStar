//! CueStar : un seul exécutable, deux rôles.
//! - sans argument : la régie (interface egui) ;
//! - `--output <config JSON>` : le moteur de sortie, processus séparé lancé par la régie.
//!   Il possède la fenêtre plein écran et libmpv ; si la régie rame, l'image continue.
//!
//! Bibliothèque : `run` est appelée par l'exe `cuestar` et par l'exe `mediastar`.

mod display_names;
mod link;
mod media_info;
mod osc;
mod output;
mod prefs;
mod preview;
mod project;
mod protocol;
mod remote;
mod stats;
mod thumb;
mod ui;

/// Version de CueStar (indépendante de celle de MediaStar).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Lance CueStar selon ses arguments (`args[0]` est le chemin de l'exe, comme `std::env::args`).
/// Les rôles `--output`, `--preview`, `--info` et `--thumb` sont ceux des processus que la régie
/// relance avec `std::env::current_exe()` ; sans l'un d'eux, c'est la régie.
pub fn run(args: &[String]) {
    // Journal commun à la régie et à la sortie (cuestar.log, à côté de la session).
    star_core::log::init(project::APP);
    if args.get(1).map(String::as_str) == Some("--output") {
        star_core::log::install_panic_hook(&format!("Démarrage de la sortie de CueStar {}", env!("CARGO_PKG_VERSION")));
        let cfg = args.get(2).and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default();
        let monitor_port = args.get(3).and_then(|s| s.parse().ok());
        output::run(cfg, monitor_port);
        return;
    }
    if args.get(1).map(String::as_str) == Some("--preview") {
        star_core::log::install_panic_hook("Démarrage de l'aperçu de CueStar");
        preview::run();
        return;
    }
    if args.get(1).map(String::as_str) == Some("--info") {
        if let Some(media) = args.get(2) {
            media_info::run(media, args.get(3).map(String::as_str) == Some("complet"));
        }
        return;
    }
    if args.get(1).map(String::as_str) == Some("--thumb") {
        if let (Some(media), Some(dir)) = (args.get(2), args.get(3)) {
            thumb::extract(media, dir);
        }
        return;
    }
    // Régie : journal neuf à chaque lancement (la session précédente reste en cuestar.old.log).
    star_core::log::start_fresh();
    star_core::log::install_panic_hook(&format!("Démarrage de CueStar {}", env!("CARGO_PKG_VERSION")));
    // Fenêtre de projet retenue à côté des autres réglages de la machine. eframe recentre une
    // fenêtre restaurée si on lui demande : le centrage est donc réservé au premier lancement.
    let window_state = project::APP.config_file("fenetre.ron");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("CueStar")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 560.0])
            .with_drag_and_drop(true)
            .with_icon(eframe::icon_data::from_png_bytes(include_bytes!("../assets/icons/cuestar-fenetre-256.png")).expect("icône PNG valide")),
        // Premier lancement : fenêtre de projet centrée sur l'écran principal (la sortie prend
        // le premier écran secondaire). Ensuite, sa position et sa taille sont retenues
        // (ramenées sur un écran présent si celui d'avant est débranché).
        centered: !window_state.as_ref().is_some_and(|p| p.exists()),
        persist_window: true,
        persistence_path: window_state,
        ..Default::default()
    };
    if let Err(e) = eframe::run_native("CueStar", options, Box::new(|cc| Ok(Box::new(ui::CueStarApp::new(cc))))) {
        star_core::log::critical(&format!("Arrêt sur erreur : {e}"));
    }
}
