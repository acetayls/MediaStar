# Module Companion MediaStar

Module [Bitfocus Companion](https://bitfocus.io/companion) qui pilote JingleStar et CueStar par leur OSC, avec l'état lu par OSCQuery. Le Stream Deck passe par Companion.

## Installer dans Companion (sans Node.js)

1. Télécharger `MediaStar-Companion-<version>.tgz` dans la release MediaStar (page Releases du dépôt), ou construire le paquet (voir plus bas).
2. Companion 4 : page **Modules**, **Import module package**, choisir le `.tgz`.
3. **Connections**, ajouter « MediaStar », choisir l'appli, l'adresse du PC.

## Développer

```
cd companion/mediastar
npm install
npm test                 # essais contre de fausses applis (sans Companion)
npm run package          # crée mediastar-<version>.tgz
```

Pour un essai direct : dans Companion, **Settings**, **Developer modules path** vers le dossier `companion/` du dépôt (après `npm install`), le module apparaît comme module de développement.

## Dans les applis

- Activer OSC et OSCQuery dans les préférences (télécommandes).
- CueStar sur un autre PC : autoriser l'accès réseau.

Adresses : `apps/jinglestar/src/osc.rs` et `apps/cuestar/src/osc.rs`. Si elles changent, mettre à jour `src/jinglestar.js` ou `src/cuestar.js` et `test/fake-apps.js`.

## Release

Le workflow `build-mediastar.yml` (cible `tout` ou `companion`) lance `npm ci`, `npm test` et `npm run package`, puis joint `MediaStar-Companion-<version>.tgz` à la release MediaStar quand l'entrée `release` est remplie.
