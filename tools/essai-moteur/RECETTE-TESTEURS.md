# Essayer CueStar sous macOS ou Linux

Merci de prendre vingt minutes pour cet essai. CueStar n'a encore jamais tourné sur un Mac ni
sous Linux : tout ce que vous verrez nous est utile, y compris « ça ne démarre pas ».

Il vous faut : un second écran (téléviseur, moniteur ou vidéoprojecteur), et deux ou trois
vidéos à vous, si possible de formats différents (un fichier de téléphone, un MP4 en 1080p,
un fichier 4K si vous en avez).

## À renvoyer

1. Vos réponses à la liste ci-dessous (un simple « oui / non / remarque » par ligne suffit).
2. Le journal de CueStar : `cuestar.log` et `cuestar.old.log`.
   - macOS : `~/Library/Application Support/com.acetayls.CueStar/`
   - Linux : `~/.config/cuestar/`
3. Votre machine : modèle, système et version, carte graphique. Sous Linux, précisez la
   distribution et le type de session (`echo $XDG_SESSION_TYPE` : `x11` ou `wayland`).
4. Une photo ou une capture si quelque chose s'affiche mal.

## Installation

- macOS : ouvrez le `.dmg`, glissez CueStar dans Applications. Au premier lancement, suivez le
  fichier « LISEZ-MOI - Ouvrir CueStar » fourni dans le `.dmg` (l'application n'est pas signée).
- Linux : rendez l'AppImage exécutable (`chmod +x CueStar-*.AppImage`) puis lancez-la.

## Liste d'essai

### Démarrage

- [ ] CueStar s'ouvre sans message d'erreur.
- [ ] Avec le second écran branché, il devient noir et plein écran (c'est la sortie).
- [ ] En bas de la fenêtre, l'état de la sortie indique la bonne taille et la bonne fréquence.
- [ ] Le message « affichage de secours » apparaît-il quelque part ? (Notez-le : ce n'est pas
      bloquant, mais nous devons le savoir.)

### Lecture

Glissez vos vidéos dans la Cuelist, puis GO sur chacune.

- [ ] L'image est **à l'endroit** (ni retournée, ni en miroir) et aux bonnes couleurs.
- [ ] Le mouvement est fluide, sans saccade ni déchirure horizontale.
- [ ] Le son sort, et il est calé sur l'image.
- [ ] Dans le Monitoring, le retour image correspond à l'écran de sortie, à l'endroit lui aussi.
- [ ] Dans le Monitoring ou les Informations, quel décodage est indiqué (videotoolbox, vaapi,
      nvdec, ou « no ») ? Le compteur d'images perdues monte-t-il ?

### Enchaînements

- [ ] Transition **Coupe** entre deux cues : pas d'image noire entre les deux.
- [ ] Transition **Fondu enchaîné** de 2 s : les deux images se mélangent, sans à-coup.
- [ ] Transition **Noir** de 2 s : descente au noir puis remontée.
- [ ] Un cue avec un point d'entrée et un point de sortie (clic droit, Éditer) démarre et
      s'arrête aux bons endroits.
- [ ] Un cue avec un fondu d'entrée et un fondu de sortie de 1 s : les deux se voient.
- [ ] Un cue en boucle : notez si l'image marque un arrêt à chaque retour.
- [ ] Rotation d'un cue à 90° : l'image tourne, bandes noires sur les côtés.
- [ ] Vingt GO très rapides de suite : le dernier cue lancé est bien celui à l'écran.

### Fenêtre de sortie

- [ ] Cliquez sur l'écran de sortie puis appuyez sur **F** : la sortie passe dans une fenêtre,
      la lecture continue. **F** de nouveau : retour au plein écran, sans plantage.
- [ ] En plein écran, aucune barre de menus, aucun dock, aucune notification ne passe devant.
- [ ] Le curseur de la souris ne se voit pas sur la sortie.
- [ ] Mires (onglet Sortie) : barres, grille, mire de synchro avec son timecode lisible.

### Robustesse

- [ ] Déplacez ou renommez un fichier déjà dans la Cuelist, puis GO dessus : un message
      d'erreur apparaît et l'image précédente reste à l'écran.
- [ ] Débranchez puis rebranchez le second écran pendant une lecture : que se passe-t-il ?
- [ ] Laissez tourner une boucle 30 minutes : toujours fluide ? Le ventilateur s'emballe-t-il ?
- [ ] Quittez CueStar : l'écran de sortie se libère tout de suite.

## Essai automatique (facultatif)

Si Python 3 et ffmpeg sont installés, cet essai rejoue une centaine de vérifications en dix
minutes et écrit un rapport à nous renvoyer. Depuis le dossier `tools/essai-moteur` du projet :

```bash
python3 medias.py ~/cuestar-medias
python3 recette.py /Applications/CueStar.app/Contents/MacOS/cuestar ~/cuestar-medias      # macOS
python3 recette.py ./CueStar-*.AppImage ~/cuestar-medias                                   # Linux
```

Renvoyez le fichier `recette-….txt` et les fichiers `…-moteur1.txt` écrits à côté.
