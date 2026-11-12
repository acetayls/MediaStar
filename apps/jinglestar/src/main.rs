// Évite l'ouverture d'une console sous Windows en mode release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    jinglestar::run()
}
