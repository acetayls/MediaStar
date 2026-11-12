# Journal des modifications

## 0.0.1 — en cours

Première version d'OldStar : une télécommande qui s'ouvre dans le navigateur d'une vieille
tablette ou d'un vieux téléphone, sans rien installer dessus. Le PC qui fait tourner OldStar sert
la page sur le réseau local.

### Barre du haut

- **Barre du haut commune à la suite** : logo d'OldStar, nom (« Star » en jaune) et onglets
  Connexion, Journal, Paramètres en contrôle segmenté, aux mêmes tailles que dans les autres
  applis.

### Langue et console facultative

- **OldStar en anglais ou en français**, au choix dans **Paramètres → Langue** : fenêtre du PC,
  page de la tablette (onglets, boutons, messages) et aide. Anglais par défaut. Le changement
  s'applique dès l'enregistrement ; les tablettes connectées se rechargent seules dans la
  nouvelle langue. Les libellés par défaut des boutons CueStar et JingleStar et des raccourcis
  de fenêtres suivent la langue ; ceux modifiés à la main restent tels quels. Le Journal reste
  en français. `--lang en` ou `--lang fr` choisit la langue pour un lancement.
- **Aucune console** : nouveau choix dans **Paramètres → Console**, pour n'utiliser que les
  onglets Applis et Fenêtres. OldStar ne cherche alors aucune console et l'onglet Console
  disparaît de la tablette. Aussi au lancement : `--console aucune`.

### Aide

- **Aide** complète d'OldStar, en français et en anglais : prise en main, fenêtre du PC, page de
  la tablette onglet par onglet, essai sans console, en cas de problème.
- Sur le PC : bouton **Aide** (livre) dans la barre du haut, ou touche **F1**. La version
  d'OldStar (« OldStar v0.0.1 ») est dans le titre de la fenêtre d'aide et en haut à droite.
- Sur la tablette : onglet **Aide**, avec la même aide et la version d'OldStar. Sur les très
  petits écrans (iPhone 4S), l'état de la liaison se réduit à sa pastille de couleur pour
  laisser la place aux quatre onglets.

### Sur le PC

- **Fenêtre OldStar** : l'adresse à taper sur la tablette, en grand, avec son **QR code** à
  scanner ; le choix de la carte réseau s'il y en a plusieurs ; l'état de la console
  (connectée ou non, modèle) et le nombre d'appareils connectés ; boutons **Copier
  l'adresse** et **Ouvrir la page sur ce PC**. Fermer la fenêtre arrête OldStar.
- Onglet **Journal** : tous les messages d'OldStar (démarrage, adresses, appareils qui se
  connectent, console, erreurs, touchers reçus), horodatés, avec **Copier** et **Effacer**.
  Ils sont aussi écrits dans `oldstar.log`, dans le dossier de réglages.
- Onglet **Paramètres** : port de la page, console (Yamaha ou M32) et son adresse IP, choix
  libre des fenêtres. Le port et le type de console prennent effet au lancement suivant.
- Les réglages sont dans `oldstar.json`, créé au premier lancement dans le dossier de
  réglages d'OldStar. Au premier lancement, Windows demande d'autoriser OldStar dans le
  pare-feu : il faut accepter pour que les tablettes le joignent.
- `oldstar --sans-fenetre` : sans fenêtre, le journal s'affiche dans la console.
- **Essai sans console** : `oldstar --simulateur` lance une fausse QL1 sur le PC et s'y
  connecte (faders qui bougent tout seuls, vu-mètres factices).
- **Icône OldStar** sur l'exécutable, la fenêtre et la barre des tâches, et sur la page web
  (onglet du navigateur, et icône de l'écran d'accueil quand on ajoute la page sur l'iPad).
- Un seul fichier exécutable, sans DLL à côté ; fonctionne sous Windows, macOS et Linux
  (la capture des fenêtres marche sous Windows et macOS).

### Appareils pris en charge

- **iOS 9 et plus** sur iPad 2, iPad 3, iPad mini 1, iPhone 4S et iPod touch 5 (Safari), et
  **Android 4.4 et plus** (navigateur d'origine ou Chrome). Autrement dit, la plupart des
  appareils sortis depuis 2011.
- Ajoutée à l'écran d'accueil (« Sur l'écran d'accueil » dans Safari), la page s'ouvre en plein
  écran comme une appli, sans barre d'adresse.
- Les boutons réagissent dès qu'on lève le doigt, sans le petit délai habituel des pages web sur
  tablette ; la page ne rebondit plus quand on la fait glisser, seules les listes défilent.
- La liaison avec le PC passe en temps réel. Si le réseau ou un vieux navigateur la bloque,
  OldStar bascule tout seul sur une liaison plus lente mais qui passe partout (« En ligne
  (lent) » en haut à droite). Après une coupure du Wi-Fi, la page se reconnecte seule et
  reprend là où elle en était.

### Onglet Console (Yamaha CL/QL)

- Une tranche par voie d'entrée, ou par mix (bouton **Entrées / Mix**) : nom de la voie,
  vu-mètre, fader, valeur en dB et bouton **ON**. Les tranches défilent de gauche à droite.
- Le fader se déplace en glissant le doigt depuis n'importe où sur la tranche : le poser ne fait
  pas sauter le niveau. Sa course suit celle d'un fader de console (−∞ en bas, 0 dB aux trois
  quarts, +10 dB en haut).
- **EQ / Envois** ouvre le détail d'une voie. L'égaliseur ressemble à l'écran d'une CL/QL :
  courbe de réponse dessinée, points des bandes à faire glisser sur la courbe (fréquence et
  gain), et pour chaque bande LOW / LOW-MID / HIGH-MID / HIGH des encodeurs **GAIN**, **FREQ**
  et **Q** qu'on tourne en glissant le doigt vers le haut ou le bas. LOW en cloche ou en
  plateau, HIGH en cloche, en plateau ou en coupe-haut (LPF) ; **HPF** séparé (ON et
  fréquence) ; **EQ ON** et **FLAT** (tous les gains à 0). Plages de la console : gain
  ±18 dB, fréquence 20 Hz – 20 kHz, Q 0,10 – 16. Sous l'égaliseur, les niveaux des envois vers
  chaque mix avec leur bouton ON.
- **SENDS ON FADER** : on choisit un mix (un bus sur la M32), et les faders et les ON des tranches règlent alors
  l'envoi de chaque voie vers ce mix. Un bandeau jaune rappelle le mode et le mix ; **Retour
  aux faders** revient aux faders principaux.
- Les changements faits sur la console ou par une autre tablette s'affichent aussitôt. Les
  vu-mètres ne tournent que quand l'onglet Console est affiché, pour ménager le Wi-Fi et la
  batterie.

### Console Midas M32 / Behringer X32

- OldStar pilote aussi une **M32** (ou une X32), au choix de la Yamaha dans les réglages
  (`console`, ou `--console m32` au lancement). L'onglet Console marche pareil : tranches,
  faders, ON, noms, vu-mètres, égaliseur 4 bandes avec types de bande et HPF, envois vers les
  16 bus et Sends on Fader. 32 voies et 16 bus par défaut (réglables).
- **Essai sans console** : `oldstar --simulateur-m32` lance une fausse M32 sur le PC (OSC,
  port UDP 10023) et s'y connecte.

### Onglet Applis

- Les boutons de CueStar et de JingleStar (GO, Stop, Pause, cue précédent ou suivant, Noir…),
  en grand ; le voyant vert indique que l'appli répond. Pour CueStar, le nom du cue en attente,
  le média à l'écran et le temps restant.

### Onglet Fenêtres

- L'image d'une fenêtre du PC, mise à jour plusieurs fois par seconde et ajustée à la place
  disponible sans faire défiler la page (bouton **Taille réelle / Ajuster**).
- **Raccourcis** en tête de liste (Lake Controller, éditeur CL/QL, zones agrandies), puis
  **n'importe quelle fenêtre ouverte du PC** (bouton **Actualiser**) ; une fenêtre réduite
  dans la barre des tâches est signalée, car elle ne peut pas s'afficher. Le choix libre se
  désactive dans `oldstar.json` (`free_windows`).
- **Lake Controller** est retrouvé quelle que soit sa version : par le nom du programme
  (`lake*.exe`), et non plus par le titre exact. Chaque raccourci peut chercher par bout de
  titre (plusieurs possibles séparés par « | »), par nom de programme (avec * et ?) et par
  dossier d'installation ; si plusieurs fenêtres conviennent, la plus grande est prise.
- L'image seule par défaut : les touchers ne cliquent dans la fenêtre qu'après **Toucher =
  clic**. Appui court = clic, deux appuis rapprochés = double-clic, appui long = clic droit,
  glisser = glisser-déposer ; boutons **Molette ▲ / ▼**.
- **Glisser** (déplacer un module dans Lake…) : toujours fait avec de vrais mouvements de
  souris, même en mode arrière-plan, et au rythme d'une main (curseur posé, bouton enfoncé et
  maintenu un instant, petits pas réguliers, pause avant de relâcher). Dans Lake, le module se
  pose ensuite d'un clic, comme à la souris. Ce rythme se règle dans
  `oldstar.json` (`drag`). Le Journal indique à la fin de chaque glisser combien de
  déplacements sont arrivés de la tablette.
- **Clic en arrière-plan** (la fenêtre reçoit le clic sans passer devant, choisi d'office
  quand la page est ouverte sur le PC lui-même) ou **au premier plan** (vrais clics souris, la
  fenêtre passe devant). Certaines applis ignorent les clics en arrière-plan (DirectX,
  certaines applis Qt ou .NET) : il faut alors le premier plan.
- Un gros bouton **◀ Fermer** reste toujours en haut, hors de l'image, et la touche Échap
  coupe les clics puis revient à la liste : on ne reste plus coincé dans une fenêtre.
- **Capture des fenêtres sur Mac** (Intel et Apple Silicon) : image des fenêtres par
  ScreenCaptureKit (macOS 14 et plus, ancienne méthode avant), clics, double-clics, clic droit,
  glisser et molette, au premier plan ou en arrière-plan. macOS demande deux autorisations au
  premier usage, dans Réglages Système › Confidentialité et sécurité : **Enregistrement de
  l'écran** (puis relancer OldStar) et **Accessibilité** ; le Journal le rappelle. Une fenêtre
  réduite dans le Dock ou posée sur un autre bureau est signalée comme réduite. Paquet
  `OldStar.app` (Intel, Apple Silicon ou universel) et image disque préparés.
- Sous Linux, pas de capture des fenêtres : l'onglet l'indique.
