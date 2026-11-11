# Essais du moteur de sortie de CueStar

Ces scripts font tourner le **vrai** moteur de sortie (`cuestar --output`) comme le fait la régie,
et vérifient ce qui arrive à l'écran : lecture de chaque format, points d'entrée et de sortie,
fondus, transitions, boucle, rotation, fichiers illisibles, GO en rafale, mires, fermeture.
Les tests `cargo test` ne lancent jamais le moteur ; c'est ici que se voient les erreurs
d'enchaînement.

Python 3.8 ou plus récent, aucune dépendance. Windows, macOS et Linux.

## Lancer

```bash
python tools/essai-moteur/medias.py <dossier des médias> [chemin de ffmpeg]     # une fois, ~250 Mo
python tools/essai-moteur/recette.py <cuestar> <dossier des médias>
```

`<cuestar>` est l'exécutable : `target/release/cuestar.exe`, `CueStar.app/Contents/MacOS/cuestar`,
l'AppImage. Le moteur s'ouvre en plein écran sur l'écran de sortie (le second s'il y en a deux),
son coupé. Ne pas toucher au clavier ni à la souris pendant l'essai, une dizaine de minutes.

Options de `recette.py` :

- `--seulement fondus,boucle` : seulement les essais dont le nom contient un de ces mots
  (`lecture`, `derniere_image`, `points`, `fondus`, `transitions`, `preparation`, `boucle`,
  `rotation`, `pause`, `image_son`, `illisibles`, `navigation`, `rafales`, `mires`, `fermeture`) ;
- `--secours` : essaie l'affichage de secours (mpv seul dans la fenêtre) au lieu de l'affichage
  composé, grâce à la variable `CUESTAR_SECOURS=1` que le moteur reconnaît ;
- `--prefs '{"decode":"Cpu"}'` : préférences machine passées au moteur (voir `prefs.rs`) ;
- `--rapport fichier.txt` : nom du rapport (par défaut `recette-<date>.txt`).

Le script rend 0 si tout passe, 1 sinon. Il écrit le rapport et, à côté, la sortie d'erreur du
moteur (`…-moteur1.txt` : une ligne de statistiques du compositeur par seconde, et le message
d'un éventuel arrêt brutal).

## Avant une release

Lancer la recette deux fois sur le build release : une fois telle quelle, une fois avec
`--secours`. Les deux doivent finir sur « tout passe ».

## Comment ça mesure

Chaque vidéo de `medias.py` porte un carré de couleur propre au fichier et un rectangle blanc qui
traverse le bas de l'image au fil de la lecture. Le pilote (`moteur.py`) reçoit le retour image du
moteur, jusqu'à 60 vignettes par seconde en affichage composé, 5 en secours, et en tire : quel
média est à l'écran, à quelle position, avec quelle luminosité. Les délais « GO → image »
viennent de l'événement `OnScreen` du moteur.

Ce que la recette ne voit pas : le son, la régularité image par image (voir `tools/bench-sortie`,
qui capture l'écran réel sous Windows), et tout ce qui touche au matériel (projecteur débranché,
changement d'écran).

Pour les testeurs qui n'ont pas l'habitude de la ligne de commande, voir `RECETTE-TESTEURS.md`.
