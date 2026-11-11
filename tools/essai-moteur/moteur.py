"""Pilote du moteur de sortie de CueStar, pour les essais.

Lance `cuestar --output` comme le fait la régie : commandes JSON sur son entrée standard,
événements horodatés sur sa sortie standard, vignettes du retour image sur une connexion TCP
locale. Chaque vignette est analysée à l'arrivée (luminosité, carré repère, position du marqueur
de temps des médias d'essai, voir `medias.py`).

Python 3.8 ou plus récent, sans dépendance. Fonctionne sous Windows, macOS et Linux.
"""
import json
import os
import socket
import struct
import subprocess
import sys
import threading
import time

# Couleurs du carré repère des médias d'essai (voir medias.py).
IDS = {"rouge": (255, 0, 0), "vert": (0, 128, 0), "bleu": (0, 0, 255), "jaune": (255, 255, 0), "magenta": (255, 0, 255),
       "orange": (255, 165, 0), "cyan": (0, 255, 255), "blanc": (255, 255, 255), "violet": (128, 0, 128), "sarcelle": (0, 128, 128),
       "noir": (0, 0, 0)}


def couleur(rgb):
    """Nom de la couleur repère la plus proche."""
    return min(IDS.items(), key=lambda kv: sum((a - b) ** 2 for a, b in zip(kv[1], rgb)))[0]


class Vignette:
    """Une image du retour image, réduite à ce que les essais regardent."""
    __slots__ = ("t", "lum", "id", "rgb", "marque", "w", "h")

    def __init__(self, t, w, h, px):
        self.t, self.w, self.h = t, w, h
        tot = n = 0
        for y in range(4, h, 16):
            ligne = y * w * 4
            for x in range(4, w, 16):
                i = ligne + x * 4
                tot += px[i] * 0.2126 + px[i + 1] * 0.7152 + px[i + 2] * 0.0722
                n += 1
        # Luminosité moyenne, 0 (noir) à 255.
        self.lum = tot / max(n, 1)
        i = (int(h * 0.1) * w + int(w * 0.05)) * 4
        self.rgb = (px[i], px[i + 1], px[i + 2])
        self.id = couleur(self.rgb)
        # Marqueur de temps : petit rectangle blanc qui traverse la bande noire du bas.
        ligne = int(h * 0.95) * w * 4
        vals = [px[ligne + x * 4 + 1] for x in range(w)]
        mx = max(vals)
        self.marque = None
        if mx > 40:
            xs = [x for x, v in enumerate(vals) if v > mx * 0.6]
            lm = w / 96.0
            if len(xs) <= lm * 3:
                c = sum(xs) / len(xs)
                # Fraction de la durée du média, de 0 à 1.
                self.marque = (c + 0.5 - lm / 2) / (w - lm)


class Moteur:
    def __init__(self, exe, prefs=None, env=None, journal=None):
        self.evenements = []
        self.vignettes = []
        self.verrou = threading.Lock()
        self.t0 = time.perf_counter()
        ecoute = socket.socket()
        ecoute.bind(("127.0.0.1", 0))
        ecoute.listen(1)
        port = ecoute.getsockname()[1]
        e = dict(os.environ)
        e["CUESTAR_DEBUG"] = "1"
        e.update(env or {})
        p = {"volume": 0.0}
        p.update(prefs or {})
        self.journal = open(journal, "wb") if journal else subprocess.DEVNULL
        self.p = subprocess.Popen([exe, "--output", json.dumps(p), str(port)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.journal, env=e)
        threading.Thread(target=self._lire_evenements, daemon=True).start()
        threading.Thread(target=self._lire_vignettes, args=(ecoute,), daemon=True).start()

    def maintenant(self):
        return time.perf_counter() - self.t0

    def _lire_evenements(self):
        for ligne in self.p.stdout:
            try:
                ev = json.loads(ligne)
            except ValueError:
                continue
            with self.verrou:
                self.evenements.append((self.maintenant(), ev))

    def _lire_vignettes(self, ecoute):
        try:
            s, _ = ecoute.accept()
        except OSError:
            return

        def lire(n):
            b = bytearray()
            while len(b) < n:
                c = s.recv(n - len(b))
                if not c:
                    raise EOFError
                b += c
            return b

        try:
            while True:
                w, h = struct.unpack("<II", lire(8))
                px = lire(w * h * 4)
                v = Vignette(self.maintenant(), w, h, px)
                with self.verrou:
                    self.vignettes.append(v)
        except (EOFError, OSError):
            pass

    def envoyer(self, commande):
        """Envoie une commande ; rend l'heure d'envoi."""
        t = self.maintenant()
        self.p.stdin.write((json.dumps(commande) + "\n").encode())
        self.p.stdin.flush()
        return t

    @staticmethod
    def nom(ev):
        return ev if isinstance(ev, str) else next(iter(ev))

    def attendre(self, nom, delai=10.0, depuis=None):
        """Attend l'événement `nom` arrivé après `depuis` ; rend (heure, contenu) ou None."""
        depuis = self.maintenant() if depuis is None else depuis
        fin = time.perf_counter() + delai
        while time.perf_counter() < fin:
            with self.verrou:
                for t, ev in self.evenements:
                    if t >= depuis and self.nom(ev) == nom:
                        return t, (None if isinstance(ev, str) else ev[nom])
            if self.p.poll() is not None:
                return None
            time.sleep(0.002)
        return None

    def evs(self, depuis=0.0, jusque=1e9, noms=None):
        with self.verrou:
            return [(t, ev) for t, ev in self.evenements if depuis <= t <= jusque and (noms is None or self.nom(ev) in noms)]

    def erreurs(self, depuis=0.0):
        return [ev["Error"]["message"] for _, ev in self.evs(depuis, 1e9, {"Error"})]

    def vues(self, depuis=0.0, jusque=1e9):
        with self.verrou:
            return [v for v in self.vignettes if depuis <= v.t <= jusque]

    def derniere(self):
        with self.verrou:
            return self.vignettes[-1] if self.vignettes else None

    def vivant(self):
        return self.p.poll() is None

    def quitter(self):
        """Ferme le moteur ; rend (code de sortie ou None s'il a fallu le tuer, durée)."""
        t = time.perf_counter()
        try:
            self.envoyer("Quit")
        except OSError:
            pass
        try:
            self.p.wait(5)
            code = self.p.returncode
        except subprocess.TimeoutExpired:
            self.p.kill()
            code = None
        if self.journal is not subprocess.DEVNULL:
            self.journal.close()
        return code, time.perf_counter() - t


def charger(chemin, pause=False, boucle=False, garder=False, transition="Cut", secondes=1.0, clip=None, signaler_fin=False):
    """Commande `Load` (transition : Cut, Black ou Crossfade)."""
    return {"Load": {"path": chemin, "paused": pause, "looping": boucle, "hold": garder, "transition": {"kind": transition, "secs": secondes},
                     "clip": clip or {}, "signal_end": signaler_fin}}


def preparer(chemin, boucle=False, clip=None):
    return {"Prepare": {"path": chemin, "looping": boucle, "clip": clip or {}}}


def demarrer(exe, prefs=None, env=None, journal=None):
    """Lance le moteur, attend qu'il soit prêt, met le retour image en marche. Rend (moteur, Ready)."""
    m = Moteur(exe, prefs, env, journal)
    r = m.attendre("Ready", 30, depuis=0)
    if r is None:
        m.quitter()
        raise RuntimeError("le moteur n'a pas répondu « prêt » en 30 s")
    m.envoyer({"Monitor": {"on": True}})
    time.sleep(1.5)  # préchauffage des lecteurs
    return m, r[1]


if __name__ == "__main__":
    print(__doc__)
    sys.exit(0)
