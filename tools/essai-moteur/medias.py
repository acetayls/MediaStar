"""Fabrique les médias d'essai du moteur de sortie (environ 250 Mo).

    python medias.py <dossier> [chemin de ffmpeg]

Chaque vidéo porte un carré de couleur en haut à gauche, propre au fichier (le pilote sait ainsi
quel média est à l'écran), et un petit rectangle blanc qui traverse une bande noire en bas au fil
de la lecture (il donne la position lue à l'écran). Les formats dont l'encodeur manque dans le
ffmpeg utilisé sont sautés ; la recette saute alors les essais qui en dépendent.
"""
import os
import shutil
import subprocess
import sys

FFMPEG_CONNUS = [r"C:\Program Files\Shutter Encoder\Library\ffmpeg.exe", "/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg", "/usr/bin/ffmpeg"]

# nom, couleur du repère, taille, cadence, durée (s), options d'encodage
VIDEOS = [
    ("a_h264_1080p30.mp4", "red", "1920x1080", "30", 10, "-c:v libx264 -preset fast -b:v 12M -pix_fmt yuv420p -c:a aac"),
    ("b_h264_1080p25.mp4", "green", "1920x1080", "25", 10, "-c:v libx264 -preset fast -b:v 12M -pix_fmt yuv420p -c:a aac"),
    ("c_h264_1080p60.mp4", "blue", "1920x1080", "60", 10, "-c:v libx264 -preset fast -b:v 20M -pix_fmt yuv420p -c:a aac"),
    ("d_h264_1080p2398.mp4", "yellow", "1920x1080", "24000/1001", 10, "-c:v libx264 -preset fast -b:v 12M -pix_fmt yuv420p -c:a aac"),
    ("e_hevc_4k30.mp4", "magenta", "3840x2160", "30", 10, "-c:v libx265 -preset ultrafast -b:v 40M -pix_fmt yuv420p -tag:v hvc1 -c:a aac"),
    ("f_prores_1080p30.mov", "orange", "1920x1080", "30", 10, "-c:v prores_ks -profile:v 3 -pix_fmt yuv422p10le -c:a pcm_s16le"),
    ("g_hap_1080p30.mov", "cyan", "1920x1080", "30", 10, "-c:v hap -format hap -c:a pcm_s16le"),
    ("k_court_1080p30.mp4", "white", "1920x1080", "30", 3, "-c:v libx264 -preset fast -b:v 12M -pix_fmt yuv420p -c:a aac"),
]


def trouver_ffmpeg(arg):
    if arg:
        return arg
    f = shutil.which("ffmpeg")
    if f:
        return f
    for c in FFMPEG_CONNUS:
        if os.path.exists(c):
            return c
    sys.exit("ffmpeg introuvable : donnez son chemin en second argument")


def lancer(ffmpeg, args):
    return subprocess.run([ffmpeg, "-hide_banner", "-loglevel", "error", "-y"] + args, capture_output=True, text=True)


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    dossier = sys.argv[1]
    ffmpeg = trouver_ffmpeg(sys.argv[2] if len(sys.argv) > 2 else None)
    os.makedirs(dossier, exist_ok=True)
    for nom, coul, taille, cadence, duree, enc in VIDEOS:
        cible = os.path.join(dossier, nom)
        if os.path.exists(cible):
            print("déjà là :", nom)
            continue
        w, h = (int(v) for v in taille.split("x"))
        carre, bande, marque = w * 400 // 1920, h // 10, w // 96
        graphe = (f"[0:v]drawbox=x=0:y=0:w={carre}:h={carre}:color={coul}:t=fill,drawbox=x=0:y=ih-{bande}:w=iw:h={bande}:color=black:t=fill[b];"
                  f"[b][2:v]overlay=x='(W-w)*t/{duree}':y=H-h[v]")
        r = lancer(ffmpeg, ["-f", "lavfi", "-i", f"testsrc2=s={taille}:r={cadence}:d={duree}", "-f", "lavfi", "-i", f"sine=f=440:d={duree}:r=48000",
                            "-f", "lavfi", "-i", f"color=c=white:s={marque}x{bande}:r={cadence}:d={duree}", "-filter_complex", graphe,
                            "-map", "[v]", "-map", "1:a", "-shortest"] + enc.split() + [cible])
        if r.returncode == 0:
            print("fabriqué :", nom)
        else:
            if os.path.exists(cible):
                os.remove(cible)
            print("sauté   :", nom, "(", r.stderr.strip().splitlines()[-1] if r.stderr.strip() else "erreur", ")")
    image = os.path.join(dossier, "i_image.png")
    if not os.path.exists(image):
        lancer(ffmpeg, ["-f", "lavfi", "-i", "color=c=teal:s=1920x1080", "-frames:v", "1", image])
        print("fabriqué : i_image.png")
    son = os.path.join(dossier, "j_son.wav")
    if not os.path.exists(son):
        lancer(ffmpeg, ["-f", "lavfi", "-i", "sine=f=440:d=5:r=48000", son])
        print("fabriqué : j_son.wav")
    # Fichiers illisibles : bruit, et un MP4 coupé (son index est à la fin du fichier).
    bruit = os.path.join(dossier, "x_bruit.mp4")
    if not os.path.exists(bruit):
        with open(bruit, "wb") as f:
            f.write(os.urandom(200_000))
    coupe = os.path.join(dossier, "x_coupe.mp4")
    source = os.path.join(dossier, "a_h264_1080p30.mp4")
    if not os.path.exists(coupe) and os.path.exists(source):
        with open(source, "rb") as f, open(coupe, "wb") as g:
            g.write(f.read(300_000))
    print("Médias prêts dans", os.path.abspath(dossier))


if __name__ == "__main__":
    main()
