# MediaStar

Un seul exécutable qui regroupe **JingleStar** (jingles, playlists) et **CueStar** (diffusion
vidéo). L'accueil lance chaque module dans son propre processus ; on peut en ouvrir plusieurs.
Les exe `jinglestar` et `cuestar` restent construits et publiés à part, chacun avec sa version.

```bash
cargo run -p mediastar            # accueil
cargo run -p mediastar -- --jingle   # JingleStar directement
cargo run -p mediastar -- --cue      # CueStar directement
```

Fonction ASIO de JingleStar sous Windows : `--features asio` (voir `apps/jinglestar/README.md`).

## Ce qu'il faut à côté de l'exe

CueStar charge libmpv et MediaInfo à l'exécution ; il les cherche à côté de l'exe (puis dans
`mpv/`), donc à côté de `mediastar` exactement comme à côté de `cuestar`. Sans libmpv, la sortie
CueStar démarre sur la mire ; JingleStar n'a besoin de rien.

| Système | À poser à côté de l'exe | Comme dans |
|---|---|---|
| Windows | `libmpv-2.dll` (build « mpv-dev » de shinchiro) et `MediaInfo.dll` (DLL x64 de mediaarea.net) | `packaging/windows/package.ps1` |
| macOS | `libmpv.2.dylib` et `libmediainfo.0.dylib` (Homebrew) avec leurs dépendances, dans `MediaStar.app/Contents/Frameworks` (dylibbundler) | `packaging/macos/bundle.sh` |
| Linux | `libmpv.so.2` (0.38 ou plus, `build-libmpv.sh`) et `libmediainfo.so.0`, dans `../lib` de l'AppImage ou installées sur le système | `apps/cuestar/packaging/linux/appimage.sh` (pas encore repris pour MediaStar) |

Paquets : workflow `.github/workflows/build-mediastar.yml` (à la demande) : installeur et zip
portable Windows, un `.dmg` macOS Intel et un `.dmg` Apple Silicon. Tant que l'app n'est pas
notarisée, le `.dmg` contient `LISEZ-MOI - Ouvrir MediaStar.txt` (passer Gatekeeper : clic
droit > Ouvrir, « Ouvrir quand même » sous macOS 15, ou `xattr -dr com.apple.quarantine`).
Licences livrées : celles de CueStar (`apps/cuestar/packaging/licences`) et `packaging/LICENCES.txt`.
Fichier de bureau Linux : `packaging/linux/mediastar.desktop`.

Icônes : `assets/icons` (ico, icns, png 1024, png 256 et png de fenêtre) et `assets/logo` (SVG),
reprises de `docs/propositions-icones/retenues/mediastar`.
