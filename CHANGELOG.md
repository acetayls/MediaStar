# Journal des modifications de la suite MediaStar

Chaque application garde aussi son propre journal (apps/jinglestar, apps/cuestar, apps/oldstar, apps/sweepstar, apps/mediastar).

## 0.0.2 — 26 novembre 2026

### Nouvelle numérotation

- **Toutes les applis repartent du même numéro de version** : MediaStar, JingleStar, CueStar,
  OldStar et SweepStar sortent ensemble en 0.0.2 dans la même release. JingleStar passe de la
  0.1.4 à la 0.0.2 (tout le contenu de la 0.1.4 y est). Chaque appli garde son exécutable et
  son journal.

### Suite

- **README refait pour les utilisateurs** : à quoi sert chaque appli, ses avantages, où la
  télécharger et comment l'installer sur Windows, macOS et Linux, avec un lien vers l'aide de
  chacune. La partie développeur est dans docs/DEVELOPPEMENT.md.
- **Module Stream Deck (Bitfocus Companion)** joint à la release MediaStar
  (`MediaStar-Companion-<version>.tgz`), avec la marche à suivre pour l'importer dans le README.

### SweepStar

- SweepStar : nouvelle appli de mesure acoustique (0.0.2) : sweep et réponse
  impulsionnelle, délai en ms et en mètres, RTA, générateur de bruits et playlist de
  soundcheck. Voir apps/sweepstar/CHANGELOG.md.
