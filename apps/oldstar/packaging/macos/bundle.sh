#!/usr/bin/env bash
# Construit OldStar.app et OldStar-<version>-macos-<arch>.dmg (à lancer sur macOS).
# Usage : apps/oldstar/packaging/macos/bundle.sh <cible> [dossier de sortie]
#   <cible> : x86_64-apple-darwin (Intel), aarch64-apple-darwin (Apple Silicon)
#             ou universal (les deux réunis avec lipo ; construit les deux cibles avant).
#   ex. : apps/oldstar/packaging/macos/bundle.sh universal dist
set -euo pipefail

TARGET="${1:-universal}"
OUT="${2:-dist}"
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"          # apps/oldstar
WORKSPACE="$(cd "$ROOT/../.." && pwd)"  # racine du dépôt MediaStar (dossier target/)
VERSION="$(grep -m1 '^version' "$ROOT/Cargo.toml" | cut -d'"' -f2)"

bin_of() {
  local b="$WORKSPACE/target/$1/release/oldstar"
  [ -x "$b" ] || { echo "Binaire introuvable : $b (lancer cargo build --release -p oldstar --target $1)" >&2; exit 1; }
  echo "$b"
}

mkdir -p "$OUT"
case "$TARGET" in
  universal)
    ARCH=universal
    BIN="$(mktemp -d)/oldstar"
    lipo -create "$(bin_of x86_64-apple-darwin)" "$(bin_of aarch64-apple-darwin)" -output "$BIN"
    ;;
  x86_64-*) ARCH=intel; BIN="$(bin_of "$TARGET")" ;;
  aarch64-*) ARCH=apple-silicon; BIN="$(bin_of "$TARGET")" ;;
  *) echo "Cible inconnue : $TARGET" >&2; exit 1 ;;
esac
lipo -info "$BIN"

APP="$OUT/OldStar.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/oldstar"
sed "s/@VERSION@/$VERSION/g" "$HERE/Info.plist" > "$APP/Contents/Info.plist"
printf 'APPL????' > "$APP/Contents/PkgInfo"
cp "$ROOT/assets/icons/oldstar.icns" "$APP/Contents/Resources/OldStar.icns"

# Signature ad hoc (pas de compte développeur : Gatekeeper demandera une confirmation).
# Attention : chaque nouvelle signature ad hoc fait oublier à macOS les autorisations
# Enregistrement de l'écran et Accessibilité ; il faut les redonner après une mise à jour.
codesign --force --deep --sign - "$APP"
codesign --verify --verbose "$APP"

# Image disque avec raccourci vers /Applications
STAGE="$(mktemp -d)"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
DMG="$OUT/OldStar-$VERSION-macos-$ARCH.dmg"
rm -f "$DMG"
hdiutil create -volname "OldStar" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
echo "Créé : $DMG"
