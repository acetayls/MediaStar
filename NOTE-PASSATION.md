# Note de passation : projet Claude « CueStar » → projet « MediaStar »

Rédigée le 11 novembre 2026, à la fermeture du projet Claude « CueStar ». Elle sert de point de
départ au nouveau projet global « MediaStar », pour reprendre sans rien redemander à Pierrick.
Fichier local, non commité (à commiter seulement si Pierrick le veut).

## 1. Règles de travail de Pierrick

- **Local uniquement**, sur le PC de Pierrick : `E:\GitHub\MediaStar`, en Remote Control.
- **Pas de GitHub sans demande explicite** : ni push, ni PR, ni release, ni lancement de workflow
  avec un tag. Le dépôt `acetayls/MediaStar` est **public** : une release se fait seulement avec
  l'accord écrit de Pierrick (d'abord un build sans tag, puis la release si tout est vert).
- **Build release** seulement quand Pierrick le demande, ou en fin de série de tâches, une fois
  les autres fils terminés.
- **Réponses en français.**
- **Fils en Opus 5.5** (pas Fable).
- Plusieurs fils travaillent en même temps dans le même dossier, sans worktree :
  - ne pas changer de branche dans l'arbre de travail ;
  - vérifier avec `cargo check -p <appli>` et un `CARGO_TARGET_DIR` à soi (pas `target/`) ;
  - committer sans toucher HEAD (index temporaire + `git commit-tree` + `git branch -f`).
- Pierrick lance souvent CueStar depuis `target\release\cuestar.exe` et le laisse ouvert : avant un
  build release, renommer l'exe en cours (ex. `cuestar-ancien.exe`) plutôt que tuer le processus.
- Empêcher ou déclencher la veille du PC est refusé côté outil : c'est Pierrick qui le règle.
- Attention : `release.toml` (cargo-release) a `push = true`. Ne pas lancer `cargo release
  --execute` sans accord, il pousserait commit et tag.

## 2. Le dépôt

Monorepo Rust (workspace Cargo, edition 2021, licence MIT), interface **eframe / egui 0.37**.

| Dossier | Rôle |
|---|---|
| `apps/jinglestar` | JingleStar : lecteur de jingles / sons pour le spectacle (version 0.1.4) |
| `apps/cuestar` | CueStar : diffusion vidéo, images, sons, PowerPoint et PDF sur un écran de sortie (libmpv + MediaInfo) |
| `apps/oldstar` | OldStar 0.0.1, télécommande web pour iPad (console Yamaha CL/QL, CueStar et JingleStar en OSC, fenêtres Windows capturées). Construit à part avec son propre `[workspace]` et son `Cargo.lock`, hors du workspace racine. **Non suivi par git** (`??`) : à clarifier avec Pierrick (garder, commiter, intégrer). |
| `crates/star-core` | Socle sans interface : langue, journal, enregistrement, session, préférences |
| `crates/star-ui` | Interface commune : thème, widgets, aide, fenêtres flottantes |
| `crates/star-remote` | Télécommandes : MIDI, consoles Yamaha et Midas, assignations, OSC |
| `crates/star-build` | Script de build commun : icône Windows, numéro de build |
| `crates/star-pptx` | Rendu de .pptx simples en images, en Rust pur |
| `crates/star-pdf` | Rendu de pages PDF (moteur hayro) |
| `tools/bench-sortie` | Mesure de la sortie CueStar (Windows), hors workspace |
| `tools/essai-moteur` | Recette automatique du moteur de sortie (`recette.py`), fiche testeurs `RECETTE-TESTEURS.md` |
| `mpv/` | libmpv locale pour le développement Windows |

CueStar : moteur de sortie dans un processus à part (`apps/cuestar/src/output/`), lié à la régie
par `link.rs`. Après toute modification de `output/`, lancer
`python tools/essai-moteur/recette.py <cuestar> <médias>` : `cargo test` ne lance jamais le moteur.
libmpv exigée : mpv 0.42 (API client 2.6) pour le rendu Direct3D 11 sous Windows.

### Workflows CI (`.github/workflows`, lancés seulement à la main)

- `build.yml` « Build JingleStar » : Windows (exe), macOS (.app / .dmg, Intel et Apple Silicon),
  Linux (archive .tar.gz, Ubuntu 22.04). Entrées `cible` et `release` (tag).
- `build-cuestar.yml` « Build CueStar » : Windows (installeur Inno Setup + zip portable, avec
  libmpv-2.dll shinchiro et MediaInfo.dll), macOS Intel et Apple Silicon (.dmg, libmpv et
  libmediainfo Homebrew embarquées, certificat Developer ID), Linux (AppImage, libmpv compilée par
  `build-libmpv.sh` et mise en cache). Entrées `cible`, `release`, `mpv_windows`, `mpv_linux`,
  `mediainfo_windows`. Scripts de paquet dans `apps/cuestar/packaging`.
- Tags : `jinglestar-vX.Y.Z` et `cuestar-vX.Y.Z` (convention de `release.toml`). Exception : la
  release JingleStar 0.1.4 porte le tag `v0.1.4`.

## 3. Versions publiées

| Appli | Version | Date | Release |
|---|---|---|---|
| JingleStar | 0.1.4 | 11 nov. 2026 | https://github.com/acetayls/MediaStar/releases/tag/v0.1.4 |
| CueStar | 0.0.1 | 11 nov. 2026 | https://github.com/acetayls/MediaStar/releases/tag/cuestar-v0.0.1 |

Release précédente : JingleStar 0.1.3 (tag `jinglestar-v0.1.3`, 3 nov. 2026).
Dernier commit poussé sur `origin/main` : `90dec84` (build CueStar Linux).

## 4. CueStar 0.0.2 : en cours, local, non poussé

État au 11 nov. 2026, 21 h 40 : tout est commité **en local sur `main`**, 2 commits d'avance sur
`origin/main`, **rien n'est poussé** :

- `49e5a37` CueStar : la sortie se ferme avec la régie sans laisser l'écran noir (fait d'abord
  sur la branche locale `claude/project-thread-1ky0jq`) ;
- `68bb435` CueStar 0.0.2 : fenêtres placées au premier lancement et retenues, bouton Plein
  écran, journal des modifications (aussi pointé par la branche locale
  `claude/project-thread-defvuf`).

Un premier essai de ce commit (`94a286a`, qui embarquait par erreur `apps/oldstar/`) a été
remplacé par `68bb435` ; il n'est plus sur aucune branche. Il n'y a qu'un seul commit de version
0.0.2. `apps/oldstar/` reste non suivi par git.

Contenu (voir `apps/cuestar/CHANGELOG.md`, section 0.0.2) :

- **Version montée** à 0.0.2 dans `apps/cuestar/Cargo.toml`.
- **Placement des fenêtres** : au premier lancement, fenêtre de projet centrée sur l'écran
  principal, sortie plein écran sur le premier écran secondaire (ou en fenêtre s'il n'y a qu'un
  écran) ; position et taille retenues ensuite, ramenées sur un écran présent si besoin.
- **Fenêtres flottantes** (Préférences, Monitoring, Télécommandes, Aide, édition d'un cue,
  Informations) : chacune se rouvre où elle a été fermée, même écran, même taille, y compris au
  lancement suivant (code commun dans `crates/star-ui/src/floating.rs`).
- **Bouton « Plein écran »** dans la barre du haut, à gauche de Préférences : bascule la sortie
  plein écran / fenêtre sans toucher à la lecture ; le désactiver demande une confirmation ; le
  choix est retenu. La case « Sortie dans une fenêtre » des Préférences est retirée.
- **Fermeture propre de la sortie vidéo** : la fenêtre de sortie disparaît et l'écran retrouve
  sa fréquence dès la fermeture ; si le moteur se bloque en libérant la vidéo, arrêt forcé au
  bout de 2 s ; attentes bornées dans `link.rs` et `preview.rs` (commit d'origine `49e5a37`,
  branche locale `claude/project-thread-1ky0jq`).
- **AppImage Linux** : dépendances de libmpv toujours embarquées, même avec libmpv en cache
  (déjà poussé dans `90dec84`).

Aucun build release 0.0.2 n'a été fait ni publié.

## 5. Points ouverts

1. **Mire de synchro sans textes sur Mac** : le FFmpeg de Homebrew n'a pas libfreetype, donc pas
   de `drawtext` ; la mire s'affiche sans timecode ni légendes (`DRAWTEXT` dans
   `apps/cuestar/src/protocol.rs`). Piste : embarquer une libmpv / FFmpeg compilée avec freetype
   sur macOS, comme sous Linux. Rappel Windows : tout `drawtext` doit passer par `mire_font()`
   (fontfile obligatoire, sinon libmpv refuse tout le graphe lavfi).
2. **FPS 25 et fréquence de l'écran 2** : avec FPS 25 dans les Préférences, la sortie passe
   l'écran de sortie de Pierrick en 100 Hz au démarrage puis le remet en 60 Hz à la fermeture ; ce
   changement peut faire perdre le signal (écran noir). À envisager : ne plus changer la
   fréquence de l'écran sans nécessité (garder 60 Hz quand le contenu passe, ou le proposer en
   option). Récupération côté Windows : Win + Ctrl + Maj + B, ou éteindre / rallumer l'écran.
3. **Arrêt forcé à 2 s pas encore éprouvé en vrai** : le blocage n'a pas été reproduit (fermeture
   en ~1 s à vide et en lecture). Si le noir revient : journal Windows Application ID 1001/1002
   pour `cuestar.exe`, et `cuestar.log` (ligne « fermeture bloquée, arrêt forcé du moteur »).
4. **Audit du moteur (10 nov. 2026)** : défauts 6 à 9 non traités (relance automatique, boucle
   sans à-coup, perte d'affichage, 4K60). Rapport : https://claude.ai/artifact/G11PoCXxbWtp3ebtPtf7iJ.
   Pierrick n'a ni Mac ni PC Linux : essais macOS / Linux par des testeurs, à partir des builds
   GitHub Actions et de `tools/essai-moteur/RECETTE-TESTEURS.md`.
5. **Anciens fils restés ouverts dans le projet CueStar**, état réel vérifié dans le dépôt :
   - « Remplacer le lecteur par la cuelist » : **fait**. La Cuelist a remplacé le Lecteur dans
     `c1de4a5` (onglets actuels : Cuelist, Diaporama, Présentation), sur `main` et poussé. Le fil
     attendait seulement une réponse de Pierrick sur un push déjà parti. Rien à reprendre, sauf
     que la section 0.0.1 du CHANGELOG parle encore du « Lecteur ».
   - « Ajouter un onglet Log aux Préférences » : **fait**. Onglet Log dans les Préférences
     (`c1ad1a0`), niveaux Debug / Info / Alerte / Critique et filtre (`92aca5d`), sur `main`.
     Il ne restait que la relecture par Pierrick.
6. **`apps/oldstar/`** non suivi par git : voir section 2.
7. **Branches locales** : beaucoup de branches `claude/project-thread-*` déjà fusionnées (0 commit
   d'avance sur `main`), plus `cuestar-plein-ecran`, `essai-integration`, `version-0.1.4` ; à
   nettoyer un jour, avec l'accord de Pierrick.

## 6. Fichiers à recopier dans le nouveau projet

Ces fichiers sont dans les fichiers partagés du projet Claude « CueStar », pas dans le dépôt. Ils
disparaîtront avec le projet : **à recopier dans le projet « MediaStar »** (ou dans le dépôt).

- `icones-proposition/` : propositions d'icônes CueStar et JingleStar en SVG (app, A, B, petite),
  `generer_icones.py`, planches `planche-icones.png` et `planche-pad-agrandi.png`.
- `star-pptx-samples/` : rendus de référence du moteur PowerPoint (`demo-02-puces.png`,
  `demo-05-tableau.png`, `graphiques-star-pptx-vs-libreoffice.png`, `modele-candy-02.png`).
- À la racine : `cuestar-256.png`, `cuestar2.png`, `icones-finales.png`, `mire25.png`,
  `petites-tailles.png`, `planche-superposition.png`, `ui2.png`, `ui5.png`, `warp2.png`
  (maquettes d'interface, mire, warp et icônes).

La mémoire Claude locale du dossier (`C:\Users\Stonike\.claude\projects\E--GitHub-MediaStar\memory\`)
reste sur le PC et sera relue par toute session ouverte dans ce dossier.
