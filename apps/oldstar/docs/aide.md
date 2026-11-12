# Aide d'OldStar

OldStar transforme une vieille tablette ou un vieux téléphone en télécommande, sans rien
installer dessus : le PC qui fait tourner OldStar sert une page web sur le réseau local, et la
tablette l'ouvre dans son navigateur. Depuis cette page, on pilote une console **Yamaha CL/QL**
ou **Midas M32 / Behringer X32**, les boutons de **CueStar** et de **JingleStar**, et on voit
(et touche) des **fenêtres du PC**, comme Lake Controller. La console est facultative : sans elle,
OldStar ne propose que les onglets Applis et Fenêtres.

## Prise en main

1. Lancez OldStar sur le PC. Au premier lancement, Windows demande d'autoriser OldStar dans le
   pare-feu : acceptez, sinon les tablettes ne pourront pas le joindre.
2. Branchez la tablette sur le même réseau (Wi-Fi) que le PC.
3. Sur la tablette, ouvrez le navigateur et tapez l'adresse affichée en grand dans l'onglet
   **Connexion** (par exemple `http://192.168.1.20:8080/`), ou scannez le **QR code**.
4. En haut à droite de la page, **En ligne** (vert) indique que la tablette parle au PC.
5. Dans **Paramètres**, choisissez la langue, la console (ou **Aucune**) et son adresse IP,
   puis **Enregistrer**.

Astuce : dans Safari, **Partager → Sur l'écran d'accueil** ajoute OldStar comme une appli ; elle
s'ouvre alors en plein écran, sans barre d'adresse.

Appareils pris en charge : iOS 9 et plus (iPad 2, iPad 3, iPad mini 1, iPhone 4S, iPod touch 5)
et Android 4.4 et plus (navigateur d'origine ou Chrome).

## La fenêtre du PC

### Barre du haut

| Élément | Rôle |
|---|---|
| **Connexion** | Adresse à taper sur la tablette, QR code, état de la console et des appareils |
| **Journal** | Tous les messages d'OldStar, horodatés |
| **Paramètres** | Langue, port de la page, console et son adresse, choix libre des fenêtres |
| Livre | Ouvre cette aide (touche **F1**) |
| **Quitter** | Arrête OldStar |

Fermer la fenêtre arrête OldStar : les tablettes passent alors **Hors ligne**.

### Connexion

- L'adresse à taper sur la tablette, en grand, et son **QR code**.
- **Copier l'adresse** et **Ouvrir la page sur ce PC** (pour vérifier la page sans tablette).
- **Carte réseau** : si le PC en a plusieurs (Wi-Fi, câble, carte virtuelle…), choisissez celle
  du réseau de la tablette. **Actualiser les adresses** après avoir branché un câble.
- L'état de la console (connectée ou hors ligne, modèle) et le nombre d'appareils connectés.

### Journal

Démarrage, adresses, appareils qui se connectent, console, erreurs, touchers reçus. **Copier**
met tout le journal dans le presse-papiers (utile pour signaler un problème), **Effacer** le
vide. Il est aussi écrit dans `oldstar.log`, dans le dossier de réglages.

### Paramètres

| Réglage | Rôle |
|---|---|
| Langue | Anglais ou français, pour cette fenêtre et pour la page de la tablette |
| Port de la page | Port HTTP de la page (8080 par défaut) |
| Console | **Aucune**, Yamaha CL/QL ou Midas M32 / X32 |
| Adresse IP et port | Où joindre la console |
| Fenêtres du PC | La page peut afficher n'importe quelle fenêtre ouverte, ou seulement les raccourcis |

La langue s'applique dès qu'on **Enregistre** : les pages ouvertes sur les tablettes se
rechargent dans la nouvelle langue. Le port et le choix de la console prennent effet au
lancement suivant ; une nouvelle adresse de console est prise aussitôt. Tout est enregistré dans `oldstar.json`, dans le dossier de
réglages d'OldStar (le chemin est rappelé en bas de l'onglet).

## Sur la tablette

### Bandeau du haut

Les onglets **Console** (absent quand la console est **Aucune**), **Applis**, **Fenêtres** et
**Aide**, puis l'état de la liaison :

| Voyant | Signification |
|---|---|
| **En ligne** (vert) | Liaison en temps réel |
| **En ligne (lent)** (vert) | Le réseau ou le navigateur bloque le temps réel : liaison plus lente qui passe partout |
| **Connexion…** (jaune) | Recherche du PC |
| **Hors ligne** (rouge) | PC injoignable ; la page se reconnecte seule et reprend où elle en était |

Sur un très petit écran, l'état se réduit à sa pastille de couleur.

### Console

- Une tranche par voie d'entrée, ou par mix (bouton **Entrées / Mix**) : nom, vu-mètre, fader,
  valeur en dB et bouton **ON**. Les tranches défilent de gauche à droite.
- Le fader se déplace en glissant le doigt depuis n'importe où sur la tranche : le poser ne fait
  pas sauter le niveau. Sa course suit celle d'un fader de console (−∞ en bas, 0 dB aux trois
  quarts, +10 dB en haut).
- **EQ / Envois** ouvre le détail d'une voie : courbe de l'égaliseur, points des bandes à faire
  glisser (fréquence et gain), encodeurs **GAIN**, **FREQ** et **Q** à tourner en glissant le
  doigt vers le haut ou le bas, type des bandes LOW et HIGH, **HPF**, **EQ ON** et **FLAT**.
  En dessous, les envois vers chaque mix avec leur bouton ON.
- **SENDS ON FADER** : choisissez un mix (un bus sur la M32) ; les faders et les ON règlent
  alors l'envoi de chaque voie vers ce mix. Un bandeau jaune rappelle le mode ; **Retour aux
  faders** revient aux faders principaux.
- Ce qui change sur la console ou sur une autre tablette s'affiche aussitôt. Les vu-mètres ne
  tournent que quand l'onglet Console est affiché, pour ménager le Wi-Fi et la batterie.

### Applis

Les boutons de CueStar et de JingleStar (GO, Stop, Pause, cue précédent ou suivant, Noir…), en
grand. Le voyant vert indique que l'appli répond. Pour CueStar : le cue en attente, le média à
l'écran et le temps restant.

### Fenêtres

- Choisissez un **raccourci** (Lake Controller, éditeur CL/QL, zones agrandies) ou n'importe
  quelle fenêtre ouverte du PC (**Actualiser** pour mettre la liste à jour). Une fenêtre
  réduite dans la barre des tâches ne peut pas s'afficher : elle est signalée.
- Toucher l'image clique dans la fenêtre : appui court = clic, deux appuis rapprochés =
  double-clic, appui long = clic droit, glisser = glisser-déposer. Glisser à **deux doigts**
  fait défiler (molette sur un PC).
- **Clic : premier plan** (vrais clics souris, la fenêtre passe devant) ou **Clic :
  arrière-plan** (la fenêtre reçoit le clic sans passer devant). Certaines applis ignorent les
  clics en arrière-plan : passez alors au premier plan.
- **Taille réelle / Ajuster** : image à sa taille, ou ajustée à l'écran. **Plein écran** ne
  laisse que l'image ; la **✕** dans le coin en sort.
- **◀ Fermer**, toujours en haut, revient à la liste (touche Échap sur un PC).

La capture des fenêtres marche sous Windows et sous macOS (pas sous Linux). Sur Mac, il faut
autoriser OldStar deux fois, dans **Réglages Système › Confidentialité et sécurité** :
**Enregistrement de l'écran** pour voir les fenêtres (puis relancer OldStar), et
**Accessibilité** pour que les touchers cliquent. macOS les demande au premier usage.

### Aide

Cette aide, lisible sur la tablette.

## Essai sans console

- `oldstar --simulateur` lance une fausse Yamaha QL1 sur le PC et s'y connecte (faders qui
  bougent seuls, vu-mètres factices).
- `oldstar --simulateur-m32` fait de même avec une fausse M32.
- `oldstar --console aucune` démarre sans console, `oldstar --lang fr` (ou `en`) choisit la
  langue pour ce lancement.
- `oldstar --sans-fenetre` tourne sans fenêtre ; le journal s'affiche dans la console.

## En cas de problème

| Symptôme | À vérifier |
|---|---|
| La tablette n'ouvre pas la page | Même réseau que le PC ? Bonne carte réseau dans **Connexion** ? OldStar autorisé dans le pare-feu ? |
| **Hors ligne** sur la tablette | OldStar tourne-t-il encore ? Le Wi-Fi de la tablette est-il en veille ? |
| Console **hors ligne** | Adresse IP et port dans **Paramètres** ; la console est-elle sur le même réseau ? |
| Pas d'onglet Console | La console est sur **Aucune** dans **Paramètres** |
| Une fenêtre reste noire | Elle est réduite dans la barre des tâches (ou le Dock) : rouvrez-la sur le PC. Sur Mac : autorisation Enregistrement de l'écran |
| Les touchers ne font rien | Essayez **Clic : premier plan**. Sur Mac : autorisation Accessibilité |

Le **Journal** dit presque toujours ce qui se passe.
