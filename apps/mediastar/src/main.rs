// Évite l'ouverture d'une console sous Windows en mode release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! MediaStar : JingleStar et CueStar dans un seul exécutable.
//! - sans argument : l'accueil, qui lance chaque module dans son propre processus ;
//! - `--jingle` : JingleStar ;
//! - `--cue` : la régie CueStar ;
//! - `--old [arguments d'OldStar]` : OldStar (ex. `--old --simulateur`) ;
//! - `--sweep` : SweepStar ;
//! - `--output`, `--preview`, `--info`, `--thumb` : les processus que la régie CueStar relance
//!   avec `std::env::current_exe()`, donc ce même exe ; ils vont tels quels à `cuestar::run`.

mod home;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--jingle") => {
            let _ = jinglestar::run();
        }
        Some("--cue") => {
            // La régie, comme `cuestar` sans argument (les arguments suivants sont gardés).
            let mut rest = vec![args[0].clone()];
            rest.extend(args.iter().skip(2).cloned());
            cuestar::run(&rest);
        }
        Some("--old") => {
            // OldStar avec ses propres arguments (`--simulateur`, `--sans-fenetre`…).
            let mut rest = vec![args[0].clone()];
            rest.extend(args.iter().skip(2).cloned());
            oldstar::run(&rest);
        }
        Some("--sweep") => {
            let _ = sweepstar::run();
        }
        Some("--output" | "--preview" | "--info" | "--thumb") => cuestar::run(&args),
        _ => home::run(),
    }
}
