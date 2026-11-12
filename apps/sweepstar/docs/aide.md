# SweepStar

SweepStar mesure une sonorisation ou une salle : réponse en fréquence, phase, réponse
impulsionnelle et délai, avec un sweep (balayage sinusoïdal). Un analyseur par bandes (RTA)
affiche le spectre du micro en direct.

## Brancher

- **Sortie** : la sortie de la carte son vers le système à mesurer (console, ampli, enceinte).
- **Micro** : un micro de mesure sur une entrée de la même carte.
- **Boucle de référence** (conseillée) : un câble d'une sortie de la carte (la même que le
  signal) vers une autre entrée. La mesure s'en sert pour annuler la latence et la couleur de
  la carte : le délai affiché est alors le vrai temps de vol jusqu'au micro.

Choisissez la carte, les sorties du signal, l'entrée du micro et celle de la boucle dans les
**Préférences**.

## Régler le niveau

Lancez le **bruit rose** (panneau de gauche, Jouer), montez le niveau jusqu'à entendre
nettement le système, en gardant le vu-mètre du micro loin du rouge. Le niveau par défaut est
de −20 dBFS. **Échap** ou **ARRÊTER** coupe le signal à tout moment.

## Mesurer

Réglez le sweep dans les **Préférences** : durée (1 à 16 s, 3 s par défaut : plus long =
moins de bruit), fréquences et traîne (silence enregistré après, pour la réverbération). Puis
cliquez sur **MESURER**, en haut du panneau Mesures.

- **Magnitude** : lissage de 1/1 à 1/48 d'octave (menu en haut à droite).
- **Phase** : le délai du pic est retiré, pour lire la phase propre du système.
- **Réponse impulsionnelle** : en ETC (dB) ou linéaire.
- **Délai** : en millisecondes et en mètres, selon la température de l'air réglée.
- **Zoom sur le pic** : montre la réponse impulsionnelle de 1 ms avant à 4 ms après le pic.

**Graphes** : molette = zoom vertical, **Ctrl + molette** (ou Maj + molette, ou pincement) =
zoom en fréquence (ou en temps), glisser = déplacer, double-clic = vue complète. La magnitude
et la phase zooment ensemble.

Sans boucle de référence, un avertissement rappelle que le délai comprend la latence de la
carte son.

## Mesures enregistrées

Le panneau **Mesures**, à droite, garde vos mesures comme un analyseur : **Capturer la
mesure** (vue Sweep et IR) ou **Capturer le RTA** (vue RTA) l'ajoute à la liste et l'écrit
aussitôt dans le dossier des mesures (Documents/SweepStar/Mesures par défaut, **Dossier…**
pour en changer). Cochez pour afficher et superposer, cliquez sur la pastille pour changer de
couleur (24 teintes ou une couleur libre), renommez dans le champ. La vue Transfert capture
la fonction de transfert de la même façon. Le menu **⋯** exporte en **WAV** (réponse impulsionnelle,
flottant 32 bits) ou en **texte REW** (fréquence, magnitude, phase, avec le lissage choisi) et
supprime le fichier. **Importer…** ou un glisser-déposer ajoute des fichiers `.ssm`. Les
mesures affichées le restent au lancement suivant. **»** replie le panneau.

## RTA

Spectre du micro par bandes de 1/1 à 1/48 d'octave, pondérations A, C ou Z, moyenne rapide,
moyenne ou lente, et maintien des crêtes. **Courbe** trace une bande glissante de la largeur
choisie (un point tous les 1/48 d'octave), sans les marches des **Barres** ; **Lissage**
adoucit encore le spectre (1/24 à 1/1 d'octave). Les **Barres** sont des barres pleines, une
par bande.

**Calibrer…** : posez un calibreur (94 ou 114 dB SPL à 1 kHz) sur le micro et cliquez sur
**Calibrer** ; le RTA et le niveau global (LZ, LA ou LC) passent en dB SPL. La calibration vaut
pour ce micro, ce canal et ce gain de préampli ; elle se règle aussi à la main ou se retire.

## Transfert

Fonction de transfert en direct entre la **boucle de référence** et le **micro**, avec
n'importe quel signal (bruit rose, musique) : magnitude, phase et cohérence. **Trouver le
délai** aligne la référence sur le micro (temps de vol, affiché aussi en mètres) ; le retard
se règle aussi à la main. Taille de FFT (4096 à 65 536), moyenne (1 à 64 blocs ou infinie),
lissage et **Remettre à zéro**. Il faut une entrée de boucle choisie dans les Préférences.

**Suivi auto** relance la recherche du délai toutes les secondes et le corrige quand il bouge
d'au moins un échantillon (micro déplacé, température). La recherche compare les 2,7
dernières secondes de la référence et du micro par intercorrélation (GCC-PHAT) : le pic
donne le retard du micro. Il faut un signal large bande (bruit rose, musique).

## Panneaux

**«** replie le panneau Signal à gauche, **»** le panneau Mesures à droite ; la bande qui reste
les rouvre.

## Playlist

Double-clic pour jouer, glisser pour réordonner. La barre de position se clique ou se glisse
pour aller dans le morceau. Le bouton de boucle au bout d'une ligne (ou **Boucle** pour le
morceau sélectionné) fait tourner ce morceau seul.

## Raccourcis

- **Échap** : arrête le signal et la mesure.
- **F1** : ouvre ou ferme l'aide.
