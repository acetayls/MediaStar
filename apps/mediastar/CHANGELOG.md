# Journal des modifications

## 0.0.1 — en cours

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
- **Paquets prêts pour Windows et Mac** : installeur et version portable pour Windows, et une
  image disque pour chaque type de Mac (Intel, ou Apple Silicon : M1, M2…). Lecteur vidéo et
  MediaInfo sont inclus, il n'y a rien d'autre à installer. Comme l'appli n'est pas encore
  signée par Apple, l'image disque contient un LISEZ-MOI qui explique comment l'ouvrir la
  première fois (clic droit > Ouvrir, ou « Ouvrir quand même » dans Confidentialité et
  sécurité sous macOS 15). MediaStar demande l'accès au micro pour SweepStar.
- **Icône MediaStar définitive** : quatre tuiles autour d'une étoile, pour l'exécutable, la
  fenêtre, le paquet macOS et le raccourci Linux.
