# Module Companion MediaStar

Module [Bitfocus Companion](https://bitfocus.io/companion) qui pilote JingleStar et CueStar par leur OSC, avec l'état lu par OSCQuery. Le Stream Deck passe par Companion.

## Installer dans Companion (sans Node.js)

1. Construire le paquet (voir plus bas) ou prendre `mediastar-<version>.tgz`.
2. Companion 4 : page **Modules**, **Import module package**, choisir le `.tgz`.
3. **Connections**, ajouter « MediaStar », choisir l'appli, l'adresse du PC.

Le module vise l'API 1.12 de Companion (Node 22) : Companion 4.0 à 5.1 (tableau de compatibilité dans le README de `@companion-module/base`). La 1.15 n'est pas reconnue par Companion 5.1. Ne pas monter `@companion-module/base` sans vérifier la version de Companion utilisée.

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
