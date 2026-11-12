// Évite l'ouverture d'une console sous Windows en mode release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! CueStar : un seul exécutable, plusieurs rôles (voir `cuestar::run`).

fn main() {
    cuestar::run(&std::env::args().collect::<Vec<_>>());
}
