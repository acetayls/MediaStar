#!/usr/bin/env bash
# Compile une libmpv récente pour l'AppImage de CueStar, avec FFmpeg, libplacebo et libass liés
# dedans (mpv-build, https://github.com/mpv-player/mpv-build).
#
#   apps/cuestar/packaging/linux/build-libmpv.sh <dossier de sortie>
#
# Pourquoi : CueStar demande mpv 0.38 au minimum, et les distributions stables livrent plus
# ancien (0.34 sur Ubuntu 22.04, 0.37 sur Ubuntu 24.04). On compile donc la même version que
# celle livrée sous Windows, sur une distribution ancienne pour que l'AppImage tourne partout.
#
# Résultat : <sortie>/libmpv.so.2 et <sortie>/mpv-build.txt (versions exactes, pour les licences).
# Prévu pour Ubuntu 22.04 ; demande sudo pour installer les paquets de développement.
# Versions réglables : MPV_REF, FFMPEG_REF, DAV1D_REF, NVCODEC_REF.
set -euo pipefail

OUT="${1:?dossier de sortie}"
MPV_REF="${MPV_REF:-v0.42.0}"
FFMPEG_REF="${FFMPEG_REF:-release}"        # « release » : la dernière version publiée de FFmpeg
DAV1D_REF="${DAV1D_REF:-1.5.1}"
NVCODEC_REF="${NVCODEC_REF:-n12.1.14.0}"   # en-têtes NVIDIA : pilotes 530 et plus récents

mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
WORK="${WORK:-$OUT/.travail}"
mkdir -p "$WORK"
JOBS="$(nproc)"

echo "== Paquets de développement"
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential git pkg-config nasm yasm ninja-build autoconf automake libtool python3-pip \
  libfribidi-dev libfreetype-dev libfontconfig1-dev libharfbuzz-dev \
  libx11-dev libxext-dev libxpresent-dev libxrandr-dev libxss-dev libxinerama-dev libxkbcommon-dev \
  libegl-dev libgl-dev libdrm-dev libgbm-dev libwayland-dev wayland-protocols \
  libva-dev libvdpau-dev \
  libasound2-dev libpulse-dev \
  libjpeg-dev liblcms2-dev libsnappy-dev zlib1g-dev
# mpv demande meson 1.3 au minimum ; libplacebo demande jinja2. Installés pour tout le système
# (/usr/local) : « sudo ninja install » relance meson en root, qui ne voit pas ~/.local.
sudo python3 -m pip install --upgrade 'meson>=1.3' jinja2 markupsafe
hash -r
meson --version

# Bibliothèques compilées ici et liées en statique dans FFmpeg : installées dans /usr/local, que
# pkg-config consulte toujours (mpv-build règle lui-même PKG_CONFIG_PATH pendant sa compilation).
PREFIX=/usr/local

echo "== En-têtes NVIDIA (décodage NVDEC) $NVCODEC_REF"
rm -rf "$WORK/nv-codec-headers"
git clone --depth 1 --branch "$NVCODEC_REF" https://github.com/FFmpeg/nv-codec-headers.git "$WORK/nv-codec-headers"
sudo make -C "$WORK/nv-codec-headers" install PREFIX="$PREFIX"

echo "== dav1d $DAV1D_REF (AV1 ; celui d'Ubuntu 22.04 est trop ancien pour FFmpeg)"
rm -rf "$WORK/dav1d"
git clone --depth 1 --branch "$DAV1D_REF" https://code.videolan.org/videolan/dav1d.git "$WORK/dav1d"
meson setup "$WORK/dav1d/build" "$WORK/dav1d" --prefix "$PREFIX" --libdir lib --buildtype release \
  --default-library static -Denable_tools=false -Denable_tests=false
ninja -C "$WORK/dav1d/build"
sudo "$(command -v ninja)" -C "$WORK/dav1d/build" install

echo "== mpv-build : mpv $MPV_REF, FFmpeg $FFMPEG_REF"
rm -rf "$WORK/mpv-build"
git clone --depth 1 https://github.com/mpv-player/mpv-build.git "$WORK/mpv-build"
cd "$WORK/mpv-build"
./use-mpv-custom "$MPV_REF"
if [ "$FFMPEG_REF" = release ]; then ./use-ffmpeg-release; else ./use-ffmpeg-custom "$FFMPEG_REF"; fi
# Une ligne = un argument, sans guillemets. libfreetype et libharfbuzz : filtre drawtext (mire de synchro).
printf '%s\n' --enable-pic --enable-libfreetype --enable-libharfbuzz --enable-libfontconfig --enable-libfribidi --enable-libdav1d --enable-libsnappy --enable-vaapi --enable-vdpau --enable-ffnvcodec --enable-nvdec > ffmpeg_options
# La bibliothèque seule : pas de lecteur en ligne de commande, pas de scripts Lua ni JavaScript.
printf '%s\n' -Dlibmpv=true -Dcplayer=false -Dlua=disabled -Djavascript=disabled -Dmanpage-build=disabled \
  -Dx11=enabled -Degl=enabled -Dvaapi=enabled -Dalsa=enabled -Dpulse=enabled > mpv_options
./rebuild "-j$JOBS"

LIB="$(find "$WORK/mpv-build/mpv/build" -maxdepth 1 -name 'libmpv.so.2.*' -type f | head -1)"
[ -n "$LIB" ] || { echo "libmpv.so introuvable après la compilation"; exit 1; }
cp -f "$LIB" "$OUT/libmpv.so.2"
strip --strip-unneeded "$OUT/libmpv.so.2"

{
  echo "libmpv compilée par mpv-build (https://github.com/mpv-player/mpv-build) le $(date -u +%Y-%m-%d)"
  echo "mpv       : $MPV_REF ($(git -C "$WORK/mpv-build/mpv" rev-parse HEAD))"
  echo "ffmpeg    : $(git -C "$WORK/mpv-build/ffmpeg" describe --tags --always) ($(git -C "$WORK/mpv-build/ffmpeg" rev-parse HEAD))"
  echo "libplacebo: $(git -C "$WORK/mpv-build/libplacebo" rev-parse HEAD)"
  echo "libass    : $(git -C "$WORK/mpv-build/libass" rev-parse HEAD)"
  echo "dav1d     : $DAV1D_REF"
  echo "nv-codec-headers : $NVCODEC_REF"
  echo "Options FFmpeg : $(tr '\n' ' ' < "$WORK/mpv-build/ffmpeg_options")"
  echo "Options mpv    : $(tr '\n' ' ' < "$WORK/mpv-build/mpv_options")"
} > "$OUT/mpv-build.txt"
cat "$OUT/mpv-build.txt"

echo "== Contrôle"
# Les fonctions dont CueStar a besoin, dont l'API de rendu.
for sym in mpv_create mpv_command_ret mpv_render_context_create mpv_render_context_render; do
  nm -D --defined-only "$OUT/libmpv.so.2" | grep -q " $sym\$" || { echo "symbole manquant : $sym"; exit 1; }
done
# Décodeurs et accélérations attendus dans le FFmpeg lié.
for mot in drawtext libdav1d hap prores vaapi nvdec; do
  grep -q -a "$mot" "$OUT/libmpv.so.2" || { echo "absent de la bibliothèque : $mot"; exit 1; }
done
echo "Bibliothèques système dont elle dépend :"
ldd "$OUT/libmpv.so.2" | awk '{print "  " $1}'
echo "Créé : $OUT/libmpv.so.2 ($(du -h "$OUT/libmpv.so.2" | cut -f1))"
