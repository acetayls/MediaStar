#!/usr/bin/env bash
# Construit MediaStar-<version>-linux-x86_64.AppImage, avec libmpv, libmediainfo et leurs dépendances.
# À lancer sur Linux après `cargo build --release -p mediastar` :
#   apps/mediastar/packaging/linux/appimage.sh dist
# Il faut libmediainfo installé (paquet libmediainfo0v5) et une libmpv 0.38 ou plus récente :
# celle compilée par apps/cuestar/packaging/linux/build-libmpv.sh, donnée par LIBMPV=<chemin de libmpv.so.2> (à défaut, celle
# du système, trop ancienne sur Ubuntu 22.04 et 24.04). linuxdeploy est téléchargé s'il manque.
# Construire sur une distribution ancienne (Ubuntu 22.04) pour que l'AppImage tourne partout.
set -euo pipefail

OUT="${1:-dist}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"          # apps/mediastar
WORKSPACE="$(cd "$ROOT/../.." && pwd)"     # racine du dépôt MediaStar
VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d'"' -f2)"
BIN="$WORKSPACE/target/release/mediastar"
[ -x "$BIN" ] || { echo "Binaire introuvable : $BIN (lancer cargo build --release -p mediastar)"; exit 1; }

# libmpv chargé à l'exécution : linuxdeploy ne le voit pas, on le lui donne.
LIBMPV="${LIBMPV:-$(ldconfig -p | awk '/libmpv\.so\.[12] /{print $NF; exit}')}"
[ -n "$LIBMPV" ] && [ -f "$LIBMPV" ] || { echo "libmpv introuvable (lancer apps/cuestar/packaging/linux/build-libmpv.sh et donner LIBMPV=…/libmpv.so.2)"; exit 1; }
LIBMPV="$(cd "$(dirname "$LIBMPV")" && pwd)/$(basename "$LIBMPV")"
echo "libmpv : $LIBMPV"
LIBMI="$(ldconfig -p | awk '/libmediainfo\.so\.0 /{print $NF; exit}')"
[ -n "$LIBMI" ] || { echo "libmediainfo introuvable (apt install libmediainfo0v5)"; exit 1; }
echo "libmediainfo : $LIBMI"

mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
TOOLS="$OUT/.outils"
mkdir -p "$TOOLS"
LINUXDEPLOY="$(command -v linuxdeploy-x86_64.AppImage || true)"
if [ -z "$LINUXDEPLOY" ]; then
  LINUXDEPLOY="$TOOLS/linuxdeploy-x86_64.AppImage"
  [ -x "$LINUXDEPLOY" ] || {
    curl -fsSL -o "$LINUXDEPLOY" https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
    chmod +x "$LINUXDEPLOY"
  }
fi
# Pas de FUSE dans les conteneurs ni sur les runners : l'AppImage de linuxdeploy s'extrait.
export APPIMAGE_EXTRACT_AND_RUN=1

APPDIR="$OUT/MediaStar.AppDir"
rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/share/doc/mediastar/licences"
cp "$WORKSPACE/apps/cuestar/packaging/licences/"* "$APPDIR/usr/share/doc/mediastar/licences/"
cp "$HERE/../LICENCES.txt" "$APPDIR/usr/share/doc/mediastar/licences/"
# Versions exactes de la libmpv compilée (mpv, FFmpeg, libplacebo…), à côté des licences.
if [ -f "$(dirname "$LIBMPV")/mpv-build.txt" ]; then
  cp "$(dirname "$LIBMPV")/mpv-build.txt" "$APPDIR/usr/share/doc/mediastar/licences/"
fi

cd "$OUT"
LDAI_OUTPUT="MediaStar-$VERSION-linux-x86_64.AppImage" \
LINUXDEPLOY_OUTPUT_VERSION="$VERSION" \
"$LINUXDEPLOY" --appdir "$APPDIR" \
  --executable "$BIN" \
  --library "$LIBMPV" \
  --library "$LIBMI" \
  --desktop-file "$HERE/mediastar.desktop" \
  --icon-file "$ROOT/assets/icons/mediastar-256.png" \
  --icon-filename mediastar \
  --output appimage
echo "Créé : $OUT/MediaStar-$VERSION-linux-x86_64.AppImage"
