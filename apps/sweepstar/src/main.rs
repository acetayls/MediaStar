// Pas de console sous Windows en release.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

fn main() -> eframe::Result {
    sweepstar::run()
}
