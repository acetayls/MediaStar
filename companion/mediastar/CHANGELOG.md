# Module Companion MediaStar : journal des modifications

## 0.0.4 (26 novembre 2026)

- Premier module Bitfocus Companion pour piloter JingleStar et CueStar depuis Companion ou un Stream Deck.
- Une connexion par appli : JingleStar ou CueStar, adresse du PC, ports OSC et OSCQuery, préfixe.
- JingleStar : arrêt général, GO, volume et mode des modules, jingles (lancer, arrêter, basculer, volume), playlist (lecture, morceau n°, pause, suivant, précédent, aléatoire, boucle), cuelist (GO, cue suivant ou précédent, cue n° en attente, pause).
- CueStar : noir, transport de la cuelist, cue en attente, aller au cue, boucle, transition ; transport et réglages du diaporama.
- Boutons qui reflètent l'état : couleur du jingle (vive quand il joue), nom et temps restant, titre du morceau, cue en attente, pause, noir.
- Boutons tout prêts (presets) : grille de 12 jingles par module, transport playlist, GO avec le nom du cue, transport CueStar.
- Compatible avec Companion 4.0 à 5.1 : le paquet d'essai 0.0.1 visait une API que Companion 5.1 ne connaît pas encore et n'apparaissait pas dans la liste des connexions.
- Boutons tout prêts : textes (PLAY, PAUSE, STOP, PREV, NEXT) à la place des symboles que Companion n'affiche pas.
- Boutons tout prêts : un retour d'état (pause, lecture…) ne fait plus disparaître le texte du bouton (le bouton lecture/pause de CueStar devenait vide en pause).
- Barre d'avancement en bas des boutons : jingle, morceau de la playlist, cue qui sonne, média de CueStar (feedback « Progress bar », couleur et hauteur réglables), ajoutée aux boutons tout prêts.
