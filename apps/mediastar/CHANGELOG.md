# Journal des modifications

## 0.0.2 — 26 novembre 2026

### Nouvelle numérotation

- **Toutes les applis de la suite repartent du même numéro de version** : MediaStar, JingleStar,
  CueStar, OldStar et SweepStar sortent ensemble en 0.0.2, publiées dans la même release
  MediaStar. Chaque appli garde son exécutable et son propre journal des modifications.
- **JingleStar passe de la 0.1.4 à la 0.0.2** : ce n'est pas un retour en arrière. Cette
  version contient tout ce qui était dans la 0.1.4 publiée, plus les nouveautés de son journal.

### Modules

- Les nouveautés de chaque module sont détaillées dans son journal : CueStar 0.0.2 (transitions
  Pousser, Volet, Iris, Zoom… dans la Cuelist, le Diaporama et la Présentation, points
  d'entrée et de sortie à l'image près), JingleStar 0.0.2 (boucle d'un morceau ou d'un cue,
  pilotage OSC depuis OldStar et Companion), OldStar 0.0.2 (micros HF), SweepStar 0.0.2
  (transfert en direct, sonomètre).

### Nouveautés

- **OldStar : suivi des micros HF** : nouvel onglet **Micros** sur la tablette, une carte par
  micro avec son nom, sa fréquence, la pile, la HF des deux antennes, le niveau audio et les
  alertes (pile faible, HF faible, saturation, interférence…). Récepteurs Sennheiser EM 300/500 G3
  et EM 2000/2050, Shure Axient Digital AD4D et AD4Q, à ajouter dans **Paramètres → Micros HF**.
  Essai sans matériel : `mediastar --old --simulateur-micros`.

- **Module Stream Deck (Bitfocus Companion)** joint à la release : `MediaStar-Companion-<version>.tgz`,
  à importer dans Companion pour piloter JingleStar et CueStar depuis un Stream Deck (marche à
  suivre dans le README du dépôt).

## 0.0.1 — 12/11/2026

- **Nouvel exécutable MediaStar** : il regroupe JingleStar, CueStar, OldStar et SweepStar. Chaque module
  s'ouvre dans sa propre fenêtre et son propre processus ; l'accueil reste ouvert pour en lancer
  un autre (JingleStar et CueStar en même temps, par exemple). Si un module ne peut pas être
  lancé, l'accueil le dit.
- **Accueil composé comme l'icône** : quatre tuiles en carré sur fond dégradé, chacune avec son
  bandeau de couleur, son symbole, une phrase sur ce que fait le module et sa version (JingleStar
  en bleu, CueStar en vert, OldStar en violet, SweepStar en turquoise). Un clic sur la tuile
  lance le module ; au survol, la tuile s'éclaire d'un liseré de sa couleur qui suit
  exactement son contour, bandeau du titre compris.
- **SweepStar** dans l'accueil : la mesure acoustique se lance comme les autres modules
  (`mediastar --sweep` en ligne de commande).
- **OldStar** dans l'accueil : la télécommande web pour « vieille » tablette se lance comme les
  autres modules. En ligne de commande, `mediastar --old` accepte les mêmes options qu'OldStar
  (par exemple `mediastar --old --simulateur`).
- **Barre du haut commune à la suite** : logo et nom de MediaStar, comme dans les autres
  applis ; la version de MediaStar et la version de build sont en petit au pied de l'accueil.
- **Anglais ou français** : l'accueil est en anglais par défaut ; le choix English / Français,
  à droite de la barre du haut, est retenu pour les lancements suivants.
- Toutes les versions sont affichées précédées d'un « v » (v0.1.4…).
- Les exécutables JingleStar, CueStar, OldStar et SweepStar restent disponibles séparément, avec leurs
  propres numéros de version.
- **Paquets prêts pour Windows, Mac et Linux** : installeur et version portable pour Windows, une
  image disque pour chaque type de Mac (Intel, ou Apple Silicon : M1, M2…) et une AppImage
  pour Linux (à rendre exécutable, puis à lancer). Lecteur vidéo et
  MediaInfo sont inclus, il n'y a rien d'autre à installer. Comme l'appli n'est pas encore
  signée par Apple, l'image disque contient un LISEZ-MOI qui explique comment l'ouvrir la
  première fois (clic droit > Ouvrir, ou « Ouvrir quand même » dans Confidentialité et
  sécurité sous macOS 15). MediaStar demande l'accès au micro pour SweepStar.
- **Icône MediaStar définitive** : quatre tuiles autour d'une étoile, pour l'exécutable, la
  fenêtre, le paquet macOS et le raccourci Linux.
