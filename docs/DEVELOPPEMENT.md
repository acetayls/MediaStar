# Développer MediaStar

Les applications de la suite et leurs briques communes, dans un seul dépôt (workspace Cargo).

| Dossier | Contenu | Releases |
| --- | --- | --- |
| [`apps/jinglestar`](../apps/jinglestar) | JingleStar, lecteur de jingles et de playlists multi-sorties | tags `jinglestar-vX.Y.Z` |
| [`apps/cuestar`](../apps/cuestar) | CueStar, diffusion vidéo sur écran de sortie | tags `cuestar-vX.Y.Z` |
| [`apps/oldstar`](../apps/oldstar) | OldStar, télécommande web pour vieilles tablettes | dans MediaStar |
| [`apps/sweepstar`](../apps/sweepstar) | SweepStar, mesure acoustique | dans MediaStar |
| [`apps/mediastar`](../apps/mediastar) | MediaStar, l'exécutable qui regroupe les quatre | tags `mediastar-vX.Y.Z` |
| [`companion/mediastar`](../companion/mediastar) | Module Bitfocus Companion (Node.js) pour JingleStar et CueStar | suit MediaStar |
| [`crates/star-core`](../crates/star-core) | Sans interface : chemins de l'appli, journal, enregistrement sûr des projets et de la session, préférences, langue, EQ, réglages de son | aucune, suit le dépôt |
| [`crates/star-ui`](../crates/star-ui) | Interface egui : thème, barre du haut, widgets, aide, EQ, éditeur de son, panneau Texte, boîtes de fichiers | aucune, suit le dépôt |
| [`crates/star-remote`](../crates/star-remote) | Télécommandes : MIDI, consoles Yamaha et Midas, correspondances et mode apprendre, serveur OSC / OSCQuery / mDNS ; option `ui` pour la fenêtre Télécommandes | aucune, suit le dépôt |
| [`crates/star-measure`](../crates/star-measure) | Calculs de mesure acoustique de SweepStar (FFT, sweep, réponse impulsionnelle, délai, transfert, RTA) | aucune, suit le dépôt |
| [`crates/star-pdf`](../crates/star-pdf) | Rendu des PDF en images, pour le module Présentation de CueStar | aucune, suit le dépôt |
| [`crates/star-build`](../crates/star-build) | `build.rs` commun : version git, date, icône Windows | aucune, suit le dépôt |
| [`crates/star-pptx`](../crates/star-pptx) | Rendu des présentations PowerPoint (.pptx) en images, en Rust pur (plus utilisé par CueStar) | aucune, suit le dépôt |

Les crates communes ne connaissent pas les applis : chaque appli garde ses adresses OSC, ses
actions et le format de ses fichiers, que des tests de format (`tests/fixtures`) protègent.
Chaque appli est aussi une bibliothèque (`run`, `VERSION`) que l'exécutable MediaStar lance
dans un processus par module ; les exécutables séparés restent construits.

Les releases de JingleStar antérieures à la fusion portent les tags `v0.1.x`.

## Construire

Installer Rust : <https://rustup.rs> (Windows : avec les « Build Tools for Visual Studio » C++ ;
Linux : `sudo apt install libasound2-dev libxkbcommon-dev libgtk-3-dev`).

```sh
cargo run --release -p mediastar
cargo run --release -p jinglestar
cargo run --release -p cuestar
cargo run --release -p oldstar
cargo run --release -p sweepstar
cargo test --workspace
```

Toutes les applications partagent la même version d'egui, fixée dans le `Cargo.toml` racine
(`[workspace.dependencies]`). CueStar charge libmpv et MediaInfo à l'exécution : voir
[`apps/cuestar/README.md`](../apps/cuestar/README.md) et
[`apps/mediastar/README.md`](../apps/mediastar/README.md). ASIO sous Windows et autres
détails : [`apps/jinglestar/README.md`](../apps/jinglestar/README.md).

## Publier une version

Chaque application a sa propre version et son propre calendrier. Avec
[cargo-release](https://github.com/crate-ci/cargo-release) (`cargo install cargo-release`) :

```sh
cargo release -p jinglestar patch            # simulation
cargo release -p jinglestar patch --execute  # bump, commit, tag jinglestar-vX.Y.Z, push
```

Puis lancer le workflow de build de l'application (onglet Actions) avec ce tag pour remplir la
release GitHub : `build.yml` (JingleStar), `build-cuestar.yml`, `build-mediastar.yml`.

## Conventions

- Messages de commit préfixés par la zone : `jinglestar: …`, `cuestar: …`, `commun: …`.
- Labels d'issues : `jinglestar`, `cuestar`, `commun`.
- Chaque appli tient son `CHANGELOG.md`, écrit pour l'utilisateur.
