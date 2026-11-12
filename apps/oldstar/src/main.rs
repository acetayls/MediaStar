//! OldStar : télécommande web pour vieilles tablettes (iOS 9+, Android 4.4+).
//! Arguments et fonctionnement : voir `oldstar::run` (src/lib.rs).

// Pas de fenêtre de console noire sous Windows en release : le journal est dans la fenêtre
// d'OldStar et dans oldstar.log.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    oldstar::run(&std::env::args().collect::<Vec<_>>());
}
