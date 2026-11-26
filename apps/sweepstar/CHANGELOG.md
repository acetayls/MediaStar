# Journal des modifications

## 0.0.2 — 26 novembre 2026

Première version de SweepStar, l'appli de mesure acoustique de la suite, à l'allure de
JingleStar.

### Nouvelle numérotation

- **Toutes les applis de la suite repartent du même numéro de version** : MediaStar, JingleStar,
  CueStar, OldStar et SweepStar sortent ensemble en 0.0.2, publiées dans la même release
  MediaStar. Chaque appli garde son exécutable et son propre journal des modifications.

### Barre du haut

- **Barre du haut commune à la suite** : logo, nom (« Star » en jaune) et onglets segmentés aux
  mêmes tailles que dans les autres applis. Aide et Préférences en boutons à icône seule (livre,
  engrenage). La version est affichée dans la fenêtre d'aide.

### Mesure par sweep

- **Bouton MESURER** en haut du panneau Mesures, réglages du sweep dans les Préférences : joue un sweep (1 à 16 s, 3 s par défaut, de 20 Hz à 20 kHz réglables)
  et enregistre en même temps le micro et, si elle est branchée, la boucle de référence.
- **Avec boucle de référence**, la latence et la couleur de la carte son s'annulent : le délai
  affiché est le vrai temps de vol jusqu'au micro. **Sans boucle**, un avertissement rappelle
  que le délai comprend la latence de la carte.
- **Magnitude** lissée de 1/1 à 1/48 d'octave, **phase** délai retiré, **réponse
  impulsionnelle** en ETC (dB) ou linéaire. Molette : zoom vertical ; **Ctrl + molette** : zoom en fréquence (ou en temps) sur tous les
  graphes ; **Zoom sur le pic** de la réponse impulsionnelle ; glisser : déplacer ;
  double-clic : vue par défaut ; survol : lecture des valeurs.
- **Délai** en millisecondes et en mètres, selon la température de l'air réglée.
- **Panneau Mesures**, à droite, à la manière d'un analyseur : capturer la mesure ou le RTA,
  nommer, changer de couleur, afficher et superposer, supprimer. Chaque capture est écrite
  aussitôt dans le dossier des mesures (Documents/SweepStar/Mesures par défaut, réglable),
  relue au lancement suivant ; import de fichiers `.ssm` par bouton ou glisser-déposer ;
  export de la réponse impulsionnelle en **WAV** et de la réponse en fréquence en **texte
  REW**. Le panneau se replie. Palette de 24 couleurs et couleur libre ; le nom se modifie sans
  que le panneau s'élargisse.

### Fonction de transfert

- **Vue Transfert en direct** : boucle de référence → micro, avec n'importe quel signal ;
  magnitude, phase et cohérence ; taille de FFT, moyenne (1 à 64 blocs ou infinie), lissage,
  remise à zéro. **Trouver le délai** aligne la référence sur le micro (**suivi auto** : le
  délai est recherché toutes les secondes et corrigé s'il bouge), réglage à la main en
  ms (et en mètres). Les transferts se capturent dans le panneau Mesures.

### Signal et entrées

- **Panneau Signal repliable**, comme le panneau Mesures.

- **Générateur** : bruit rose, bruit rose périodique, bruit blanc, sinus ; niveau par défaut
  −20 dBFS, montée et descente en douceur. **Échap** ou **ARRÊTER** coupe tout.
- **Sinus sans craquement** : la fréquence glisse en douceur quand on la change ; passer d'un
  signal à un autre se fait par un court fondu.
- **Vu-mètres** de la sortie, du micro et de la boucle ; compteur de pertes dans la barre
  d'état.

### RTA

- Spectre du micro par bandes de 1/1 à 1/48 d'octave, pondérations A, C, Z, moyenne rapide,
  moyenne ou lente, maintien des crêtes.
- **Affichage en courbe** (par défaut) : bande glissante, sans les marches des barres ;
  **lissage** réglable de 1/24 à 1/1 d'octave. Les RTA capturés se superposent au direct.
- **Barres pleines**, une par bande.
- **dB SPL** : calibration du micro avec un calibreur (94 ou 114 dB à 1 kHz), réglable à la
  main.
- **Sonomètre** en tête du RTA, en grand : LA rapide et lent, LAeq 1 min et 15 min, LCeq
  15 min, LA max, couleur verte, orange à 3 dB du seuil, rouge au-delà (seuils réglables,
  102 dB(A) et 118 dB(C) par défaut, ceux du décret 2017-1244). Calculé en continu, même hors
  de l'onglet RTA.
- **Fenêtre de niveau détachable**, toujours devant : valeur choisie en très grand, LC et LZ
  rapides, crête, Leq depuis la remise à zéro, historique des 15 dernières minutes avec le
  seuil.
- **Correction** : le niveau global pondéré A ou C était faux (gain de pondération appliqué
  en amplitude au lieu de la puissance).

### Playlist

- **Panneau Playlist** pour les fichiers de test du soundcheck : ajout par bouton ou en
  déposant fichiers et dossiers, double-clic pour jouer, glisser pour réordonner, précédent,
  suivant, niveau propre.
- **Barre de position** : clic ou glisser pour aller dans le morceau (avec un fondu, sans
  clic). **Boucle par morceau** : bouton au bout de chaque ligne, retenu avec la playlist. Même sortie et même ARRÊT que le
  générateur. La playlist est retrouvée au lancement suivant.

### Préférences

- Pilote, sortie, entrée et tampon ; sorties du signal ; canaux du micro et de la boucle de
  référence ; langue (français, anglais).
