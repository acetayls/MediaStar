"""Recette du moteur de sortie de CueStar : rejoue les scénarios d'un spectacle et dit ce qui passe.

    python recette.py <cuestar> <dossier des médias> [--rapport fichier.txt] [--seulement mot,mot] [--prefs JSON] [--secours]

`<cuestar>` : l'exécutable (target/release/cuestar.exe, CueStar.app/Contents/MacOS/cuestar,
l'AppImage…). Les médias se fabriquent avec `medias.py`. Le moteur s'ouvre en plein écran sur
l'écran de sortie (le second s'il y en a deux), son coupé ; ne pas toucher au clavier ni à la
souris pendant l'essai (une dizaine de minutes).

Code de sortie 0 si tout passe, 1 sinon. Le rapport écrit est celui à renvoyer avec le journal.
"""
import os
import platform
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from moteur import charger, demarrer, preparer  # noqa: E402

VIDEOS = [  # fichier, repère, durée, cadence
    ("a_h264_1080p30.mp4", "rouge", 10, 30), ("b_h264_1080p25.mp4", "vert", 10, 25), ("c_h264_1080p60.mp4", "bleu", 10, 60),
    ("d_h264_1080p2398.mp4", "jaune", 10, 23.976), ("e_hevc_4k30.mp4", "magenta", 10, 30), ("f_prores_1080p30.mov", "orange", 10, 30),
    ("g_hap_1080p30.mov", "cyan", 10, 30), ("k_court_1080p30.mp4", "blanc", 3, 30),
]


class Recette:
    def __init__(self, exe, medias, prefs, rapport, env=None):
        self.exe, self.dossier, self.prefs, self.env = exe, medias, prefs, env
        self.lignes = []
        self.echecs = 0
        self.rapport = rapport
        self.m = None
        self.secours = False
        self.n_moteur = 0

    # --- compte rendu
    def dire(self, texte=""):
        print(texte, flush=True)
        self.lignes.append(texte)

    def titre(self, texte):
        self.dire()
        self.dire("== " + texte)

    def verifier(self, quoi, ok, detail=""):
        if not ok:
            self.echecs += 1
        self.dire(f"  [{'OK   ' if ok else 'ÉCHEC'}] {quoi}" + (f" : {detail}" if detail else ""))
        return ok

    def note(self, texte):
        self.dire(f"  [note ] {texte}")

    # --- moteur
    def med(self, nom):
        p = os.path.join(self.dossier, nom)
        return p if os.path.exists(p) else None

    def lancer(self):
        self.n_moteur += 1
        journal = os.path.splitext(self.rapport)[0] + f"-moteur{self.n_moteur}.txt"
        self.m, pret = demarrer(self.exe, self.prefs, env=self.env, journal=journal)
        return pret

    def assurer(self):
        """Relance le moteur s'il s'est arrêté pendant un essai (les essais suivants restent valables)."""
        if self.m is None or not self.m.vivant():
            self.note("moteur arrêté : relance pour la suite")
            self.lancer()

    def a_l_ecran(self, delai=1.5):
        """Dernière vignette après `delai` secondes de calme."""
        time.sleep(delai)
        return self.m.derniere()

    def jouer(self, nom, **kw):
        """GO sur un média, attend sa première image. Rend (heure du GO, heure de la première image)."""
        m = self.m
        t = m.envoyer(charger(self.med(nom), **kw))
        on = m.attendre("OnScreen", 6, depuis=t) if not self.secours else m.attendre("Loaded", 6, depuis=t)
        return t, (on[0] if on else None)

    def plateau(self, depuis, jusque):
        """Luminosité habituelle de l'image entre deux instants (médiane)."""
        l = sorted(v.lum for v in self.m.vues(depuis, jusque))
        return l[len(l) // 2] if l else 0.0

    # --- essais
    def e_demarrage(self):
        self.titre("Démarrage")
        pret = self.lancer()
        m = self.m
        err = m.erreurs(0)
        self.secours = any("secours" in e for e in err)
        ecran = pret["monitor"]
        self.dire(f"  mpv : {pret['mpv']}")
        self.dire(f"  écran de sortie : {ecran['name']} {ecran['width']}x{ecran['height']} à {ecran['refresh_hz']:.2f} Hz, plein écran : {pret['fullscreen']}, écrans vus : {len(pret['monitors'])}")
        self.verifier("libmpv chargée", pret["mpv"] is not None, "" if pret["mpv"] else str(m.evs(0, 1e9, {"MpvMissing"})))
        if self.env and "CUESTAR_SECOURS" in self.env:
            self.verifier("affichage de secours demandé et actif", self.secours, "; ".join(err))
        else:
            self.verifier("affichage composé (deux lecteurs, fondu enchaîné)", not self.secours, "; ".join(err))
        if self.secours:
            self.note("affichage de secours : les essais de fondu enchaîné, de préparation et de retour image fin sont allégés")
        autres = [e for e in err if "secours" not in e]
        self.verifier("aucune erreur au démarrage", not autres, "; ".join(autres))
        return pret["mpv"] is not None

    def e_lecture(self):
        self.titre("Lecture de chaque format, fin au noir")
        m = self.m
        for nom, coul, duree, cadence in VIDEOS:
            if not self.med(nom):
                self.note(f"{nom} absent : sauté")
                continue
            self.assurer()
            t, on = self.jouer(nom)
            fin = m.attendre("Ended", duree + 8, depuis=t)
            t_fin = fin[0] if fin else m.maintenant()
            vues = [v for v in m.vues(on or t, t_fin) if v.marque is not None]
            hw = [ev["Hwdec"]["name"] for _, ev in m.evs(t, t_fin, {"Hwdec"})]
            pertes = [ev["Drops"]["dropped"] for _, ev in m.evs(t, t_fin, {"Drops"})]
            perdu = max(pertes, default=0)
            bons = sum(1 for v in vues if v.id == coul)
            derniere = self.a_l_ecran(0.6)
            detail = f"GO→image {((on - t) * 1000 if on else float('nan')):.0f} ms, décodage {hw[-1] if hw else '?'}, images perdues {perdu}, lu {(t_fin - (on or t)):.2f} s sur {duree}"
            ok = fin is not None and on is not None and not m.erreurs(t) and abs((t_fin - on) - duree) < 0.6
            self.verifier(f"{nom} : lu en entier", ok, detail + ("" if not m.erreurs(t) else " ; erreurs " + "; ".join(m.erreurs(t))))
            if vues:
                self.verifier(f"{nom} : bonne image à l'écran", bons >= len(vues) * 0.9, f"{bons} vignettes sur {len(vues)} portent le repère {coul} (vu : {vues[len(vues) // 2].id} {vues[len(vues) // 2].rgb})")
            if derniere is not None:
                self.verifier(f"{nom} : noir après la fin", derniere.lum < 5, f"luminosité {derniere.lum:.0f}")
            if perdu > 2:
                self.note(f"{nom} : {perdu} images perdues (à surveiller)")

    def e_derniere_image(self):
        self.titre("Fin de cue sur la dernière image")
        m = self.m
        if not self.med("k_court_1080p30.mp4"):
            return self.note("média court absent : sauté")
        t, on = self.jouer("k_court_1080p30.mp4", garder=True, signaler_fin=True)
        fin = m.attendre("Held", 8, depuis=t)
        v = self.a_l_ecran(0.6)
        self.verifier("fin signalée, média gardé", fin is not None and not m.evs(t, 1e9, {"Ended", "Idle"}))
        if v is not None:
            self.verifier("dernière image à l'écran", v.id == "blanc" and v.marque is not None and v.marque > 0.9, f"repère {v.id}, position {v.marque}")

    def e_points(self):
        self.titre("Points d'entrée et de sortie")
        m = self.m
        if not self.med("a_h264_1080p30.mp4"):
            return self.note("média absent : sauté")
        t, on = self.jouer("a_h264_1080p30.mp4", clip={"start": 2.0, "end": 5.0})
        fin = m.attendre("Ended", 9, depuis=t)
        vues = [v for v in m.vues(on or t, fin[0] if fin else 1e9) if v.marque is not None and v.id == "rouge"]
        if not (fin and on and vues):
            return self.verifier("lecture entre 2 s et 5 s", False, f"première image {on}, fin {fin}, vignettes {len(vues)}")
        # En secours, le retour image n'a que 5 vignettes par seconde.
        self.verifier("entrée à 2 s", abs(vues[0].marque * 10 - 2.0) < (0.3 if self.secours else 0.12), f"première image à {vues[0].marque * 10:.2f} s")
        self.verifier("sortie à 5 s", 4.8 < vues[-1].marque * 10 < 5.05, f"dernière image à {vues[-1].marque * 10:.2f} s")
        self.verifier("durée à l'écran de 3 s", abs((fin[0] - on) - 3.0) < 0.25, f"{fin[0] - on:.2f} s")

    def e_fondus(self):
        self.titre("Fondus d'entrée et de sortie d'un cue")
        m = self.m
        if not self.med("a_h264_1080p30.mp4"):
            return self.note("média absent : sauté")
        if self.secours:
            return self.note("non mesuré en affichage de secours")
        t, on = self.jouer("a_h264_1080p30.mp4", clip={"start": 2.0, "end": 6.0, "fade_in": 1.0, "fade_out": 1.0})
        fin = m.attendre("Ended", 10, depuis=t)
        if not (on and fin):
            return self.verifier("lecture avec fondus", False, f"première image {on}, fin {fin}")
        plein = self.plateau(on + 1.5, fin[0] - 1.5)
        debut = self.plateau(on, on + 0.3)
        sortie = self.plateau(fin[0] - 0.3, fin[0])
        self.verifier("fondu d'entrée", debut < plein * 0.5, f"luminosité {debut:.0f} au départ, {plein:.0f} en plein")
        self.verifier("fondu de sortie", sortie < plein * 0.5, f"luminosité {sortie:.0f} sur les 0,3 dernières secondes, {plein:.0f} en plein")
        # Sans point de sortie : le fondu se cale sur la fin du fichier.
        if self.med("k_court_1080p30.mp4"):
            t, on = self.jouer("k_court_1080p30.mp4", clip={"fade_out": 1.0})
            fin = m.attendre("Ended", 8, depuis=t)
            if on and fin:
                plein = self.plateau(on + 0.5, fin[0] - 1.3)
                sortie = self.plateau(fin[0] - 0.3, fin[0])
                self.verifier("fondu de sortie calé sur la fin du fichier", sortie < plein * 0.5, f"luminosité {sortie:.0f} à la fin, {plein:.0f} en plein")

    def e_transitions(self):
        self.titre("Transitions entre deux cues")
        m = self.m
        if not (self.med("a_h264_1080p30.mp4") and self.med("b_h264_1080p25.mp4")):
            return self.note("médias absents : sauté")
        self.jouer("a_h264_1080p30.mp4", garder=True)
        time.sleep(1.5)
        t = m.envoyer(charger(self.med("b_h264_1080p25.mp4"), garder=True, transition="Black", secondes=2.0))
        time.sleep(3.2)
        vues = m.vues(t, t + 3.2)
        if vues and not self.secours:
            self.verifier("par le noir : passage au noir", min(v.lum for v in vues) < 15, f"luminosité minimale {min(v.lum for v in vues):.0f}")
        v = self.a_l_ecran(0.3)
        self.verifier("par le noir : arrivée sur le nouveau cue", v is not None and v.id == "vert" and v.lum > 50, f"repère {v.id if v else None}")
        t = m.envoyer(charger(self.med("a_h264_1080p30.mp4"), garder=True, transition="Crossfade", secondes=2.0))
        time.sleep(3.2)
        vues = m.vues(t, t + 3.2)
        if vues and not self.secours:
            plein = self.plateau(t + 2.6, t + 3.2)
            self.verifier("fondu enchaîné : jamais sombre", min(v.lum for v in vues) > plein * 0.7, f"luminosité minimale {min(v.lum for v in vues):.0f} pour {plein:.0f} en plein")
            milieu = [v for v in vues if 0.9 < v.t - t < 1.5]
            self.verifier("fondu enchaîné : les deux images mélangées", any(v.rgb[0] > 40 and v.rgb[1] > 35 and v.rgb[0] < 240 for v in milieu), f"repère au milieu : {milieu[len(milieu) // 2].rgb if milieu else None}")
        v = self.a_l_ecran(0.3)
        self.verifier("fondu enchaîné : arrivée sur le nouveau cue", v is not None and v.id == "rouge", f"repère {v.id if v else None}")

    def e_preparation(self):
        self.titre("Cue préparé : délai du GO")
        m = self.m
        if self.secours:
            return self.note("pas de préparation en affichage de secours")
        for nom, coul in (("b_h264_1080p25.mp4", "vert"), ("e_hevc_4k30.mp4", "magenta"), ("f_prores_1080p30.mov", "orange"), ("a_h264_1080p30.mp4", "rouge")):
            if not self.med(nom):
                continue
            m.envoyer(preparer(self.med(nom)))
            time.sleep(1.5)
            t, on = self.jouer(nom, garder=True)
            v = self.a_l_ecran(0.5)
            ms = (on - t) * 1000 if on else float("nan")
            self.verifier(f"{nom} préparé", on is not None and ms < 100 and v is not None and v.id == coul, f"GO→image {ms:.0f} ms, repère {v.id if v else None}")

    def e_boucle(self):
        self.titre("Boucle")
        m = self.m
        if not self.med("a_h264_1080p30.mp4"):
            return self.note("média absent : sauté")
        clip = {"looping": True, "loop_start": 1.0, "loop_end": 2.0, "end": 4.0}
        t, on = self.jouer("a_h264_1080p30.mp4", boucle=True, garder=True, signaler_fin=True, clip=clip)
        time.sleep(3.6)
        vues = [v for v in m.vues(on or t, m.maintenant()) if v.marque is not None]
        retours = [(b.t - a.t) * 1000 for a, b in zip(vues, vues[1:]) if b.marque < a.marque - 0.03]
        self.verifier("boucle entre 1 s et 2 s", len(retours) >= 2 and max((v.marque for v in vues), default=0) * 10 < 2.2, f"{len(retours)} retours, arrêt de l'image au retour : {', '.join(f'{r:.0f} ms' for r in retours)}")
        t2 = m.envoyer({"SetClip": {"clip": dict(clip, looping=False)}})
        fin = m.attendre("Held", 8, depuis=t2)
        self.verifier("boucle décochée en lecture : le moteur reste en marche", m.vivant(), "" if m.vivant() else f"moteur arrêté, code {m.p.returncode}")
        self.assurer()
        if fin is not None:
            v = self.a_l_ecran(0.5)
            self.verifier("boucle décochée : arrêt sur le point de sortie (4 s)", v is not None and v.marque is not None and 3.8 < v.marque * 10 < 4.2, f"fin signalée après {fin[0] - t2:.2f} s, image à {v.marque * 10 if v and v.marque is not None else None} s")
        else:
            self.verifier("boucle décochée : fin signalée", False, "pas d'événement de fin en 8 s")

    def e_rotation(self):
        self.titre("Rotation")
        m = self.m
        if not self.med("a_h264_1080p30.mp4"):
            return self.note("média absent : sauté")
        t, on = self.jouer("a_h264_1080p30.mp4", garder=True)
        time.sleep(0.8)
        plein = self.plateau(t, m.maintenant())
        m.envoyer({"SetClip": {"clip": {"rotate": 90}}})
        v = self.a_l_ecran(1.0)
        # Une image 16/9 tournée d'un quart de tour laisse deux bandes noires : elle éclaire moins.
        self.verifier("rotation 90° à chaud", v is not None and v.lum < plein * 0.6 and not m.erreurs(t), f"luminosité {v.lum if v else 0:.0f} contre {plein:.0f} à plat")
        m.envoyer({"SetClip": {"clip": {"rotate": 0}}})
        v = self.a_l_ecran(1.0)
        self.verifier("retour à plat", v is not None and v.lum > plein * 0.85, f"luminosité {v.lum if v else 0:.0f}")
        if self.med("k_court_1080p30.mp4"):
            t, on = self.jouer("k_court_1080p30.mp4", garder=True, signaler_fin=True)
            m.attendre("Held", 8, depuis=t)
            avant = self.a_l_ecran(0.4)
            m.envoyer({"SetClip": {"clip": {"rotate": 90}}})
            v = self.a_l_ecran(1.0)
            self.verifier("rotation sur un cue arrêté sur sa dernière image", v is not None and avant is not None and v.lum < avant.lum * 0.6, f"luminosité {v.lum if v else 0:.0f} contre {avant.lum if avant else 0:.0f}")

    def e_pause(self):
        self.titre("Pause et déplacement")
        m = self.m
        if not self.med("a_h264_1080p30.mp4"):
            return self.note("média absent : sauté")
        t, on = self.jouer("a_h264_1080p30.mp4", garder=True)
        time.sleep(1.0)
        m.envoyer("Pause")
        a = self.a_l_ecran(0.6)
        b = self.a_l_ecran(0.8)
        self.verifier("pause : image fixe", a is not None and b is not None and a.marque == b.marque, f"position {a.marque * 10 if a and a.marque is not None else None} s")
        for s in (7.0, 0.5, 3.333):
            m.envoyer({"Seek": {"seconds": s}})
            v = self.a_l_ecran(0.8)
            self.verifier(f"déplacement à {s} s en pause", v is not None and v.marque is not None and abs(v.marque * 10 - s) < 0.1, f"image à {v.marque * 10 if v and v.marque is not None else None} s")
        m.envoyer("Play")
        v1 = self.a_l_ecran(0.5)
        v2 = self.a_l_ecran(0.5)
        self.verifier("reprise de la lecture", v1 is not None and v2 is not None and v1.marque is not None and v2.marque is not None and v2.marque > v1.marque)

    def e_image_son(self):
        self.titre("Image fixe et son seul")
        m = self.m
        if self.med("i_image.png"):
            t, on = self.jouer("i_image.png", clip={"image_secs": 2.0})
            fin = m.attendre("Ended", 6, depuis=t)
            self.verifier("image affichée 2 s", on is not None and fin is not None and abs((fin[0] - on) - 2.0) < 0.3, f"{(fin[0] - on) if fin and on else None} s")
            t, on = self.jouer("i_image.png", garder=True)
            v = self.a_l_ecran(1.0)
            self.verifier("image sans durée : elle reste", v is not None and v.id == "sarcelle" and not m.evs(t, 1e9, {"Ended"}), f"repère {v.id if v else None}")
        if self.med("j_son.wav"):
            t = m.envoyer(charger(self.med("j_son.wav")))
            charge = m.attendre("Loaded", 5, depuis=t)
            v = self.a_l_ecran(1.0)
            fin = m.attendre("Ended", 8, depuis=t)
            self.verifier("son seul : écran noir, fin signalée", charge is not None and fin is not None and v is not None and v.lum < 5, f"luminosité {v.lum if v else None}")

    def e_illisibles(self):
        self.titre("Fichiers absents ou illisibles")
        m = self.m
        if not self.med("a_h264_1080p30.mp4"):
            return self.note("média absent : sauté")
        for nom in ("absent.mp4", "x_bruit.mp4", "x_coupe.mp4"):
            chemin = os.path.join(self.dossier, nom)
            if nom != "absent.mp4" and not os.path.exists(chemin):
                continue
            self.assurer()
            self.jouer("a_h264_1080p30.mp4", garder=True)
            time.sleep(0.6)
            t = m.envoyer(charger(chemin, garder=True))
            time.sleep(2.5)
            self.verifier(f"{nom} : erreur signalée à la régie", bool(m.erreurs(t)), "; ".join(m.erreurs(t)) or "aucun message")
            if not self.secours:
                v1 = m.derniere()
                v2 = self.a_l_ecran(0.5)
                reste = v1 is not None and v2 is not None and v2.id == "rouge" and v2.marque is not None and v1.marque is not None and v2.marque > v1.marque
                self.verifier(f"{nom} : le cue précédent reste à l'écran et continue", reste, f"repère {v2.id if v2 else None}")
            self.verifier(f"{nom} : moteur en marche", m.vivant())
            t = m.envoyer(preparer(chemin))
            time.sleep(1.0)
            self.verifier(f"{nom} : préparation sans dégât", m.vivant())
        self.assurer()
        if self.med("b_h264_1080p25.mp4"):
            t, on = self.jouer("b_h264_1080p25.mp4", garder=True)
            v = self.a_l_ecran(0.8)
            self.verifier("GO suivant normal après les erreurs", on is not None and v is not None and v.id == "vert", f"repère {v.id if v else None}")

    def e_navigation(self):
        self.titre("Navigation rapide entre cues puis GO")
        m = self.m
        if self.secours:
            return self.note("pas de préparation en affichage de secours")
        liste = [(n, c) for n, c, _, _ in VIDEOS[:7] if self.med(n)]
        if len(liste) < 3:
            return self.note("pas assez de médias : sauté")
        cible, c_cible = liste[0]
        depart, c_depart = liste[2]
        mauvais = noirs = intrus = 0
        essais = 10
        for k in range(essais):
            self.assurer()
            self.jouer(depart, garder=True)
            time.sleep(0.8)
            for i in range(3 + k % 5):
                m.envoyer(preparer(self.med(liste[(i + 1) % len(liste)][0])))
                time.sleep(0.02 + 0.01 * (k % 4))
            t = m.envoyer(charger(self.med(cible), garder=True))
            time.sleep(1.8)
            vues = m.vues(t, m.maintenant())
            v = m.derniere()
            if v is None or v.id != c_cible or v.lum < 30:
                mauvais += 1
                if v is not None and v.lum < 5:
                    noirs += 1
            # Aucune autre image que le cue de départ et le cue lancé ne doit passer à l'écran.
            if any(x.id not in (c_cible, c_depart) and x.lum > 30 for x in vues):
                intrus += 1
        self.verifier("le cue lancé est à l'écran", mauvais == 0, f"{essais - mauvais} essais bons sur {essais}" + (f", dont {noirs} écrans noirs" if noirs else ""))
        self.verifier("aucune image d'un autre cue ne passe", intrus == 0, f"{intrus} essais avec une image étrangère sur {essais}")

    def e_rafales(self):
        self.titre("GO en rafale")
        m = self.m
        liste = [(n, c) for n, c, _, _ in VIDEOS[:7] if self.med(n)]
        if len(liste) < 2:
            return self.note("pas assez de médias : sauté")
        for transition, pas, n in (("Cut", 0.08, 30), ("Crossfade", 0.15, 30), ("Black", 0.2, 20)):
            self.assurer()
            t = m.maintenant()
            for i in range(n):
                m.envoyer(charger(self.med(liste[i % len(liste)][0]), garder=True, transition=transition, secondes=1.0))
                time.sleep(pas)
            v = self.a_l_ecran(3.0)
            attendu = liste[(n - 1) % len(liste)][1]
            nom = {"Cut": "coupe", "Crossfade": "fondu enchaîné", "Black": "par le noir"}[transition]
            self.verifier(f"{n} GO en {nom} toutes les {pas * 1000:.0f} ms", m.vivant() and v is not None and v.id == attendu and v.lum > 30 and not m.erreurs(t),
                          f"repère {v.id if v else None}, attendu {attendu}" + ("" if not m.erreurs(t) else " ; erreurs " + "; ".join(m.erreurs(t)[:2])))
        self.assurer()
        if self.med("b_h264_1080p25.mp4"):
            m.envoyer(charger(self.med("b_h264_1080p25.mp4"), garder=True, transition="Crossfade", secondes=2.0))
            time.sleep(0.8)
            t = m.envoyer("Stop")
            v = self.a_l_ecran(1.0)
            self.verifier("arrêt en plein fondu : écran noir", v is not None and v.lum < 5 and m.vivant(), f"luminosité {v.lum if v else None}")

    def e_mires(self):
        self.titre("Mires, mapping, plein écran")
        m = self.m
        t = m.maintenant()
        for mire in ("Bars", "Sync25", "Grid", "White"):
            m.envoyer({"Pattern": {"kind": mire}})
            v = self.a_l_ecran(1.2)
            attendu = v is not None and (v.lum > 20 or mire == "Grid")
            self.verifier(f"mire {mire}", attendu and not m.erreurs(t), f"luminosité {v.lum if v else None:.0f}" if v else "pas d'image")
        m.envoyer({"Pattern": {"kind": None}})
        v = self.a_l_ecran(0.8)
        self.verifier("mire retirée : noir", v is not None and v.lum < 5)
        if self.med("a_h264_1080p30.mp4"):
            t, on = self.jouer("a_h264_1080p30.mp4", garder=True, boucle=True)
            m.envoyer({"Warp": {"geom": {"corners": [[0.1, 0.1], [0.9, 0.05], [0.95, 0.9], [0.05, 0.95]], "mesh": None, "feather": 3.0}, "guides": None}})
            v = self.a_l_ecran(0.8)
            self.verifier("mapping appliqué sans erreur", m.vivant() and not m.erreurs(t) and v is not None and v.lum > 20)
            m.envoyer({"Warp": {"geom": None, "guides": None}})
            for plein in (False, True):
                m.envoyer({"Fullscreen": {"on": plein}})
                time.sleep(1.5)
                a = m.derniere()
                b = self.a_l_ecran(0.5)
                continue_ = a is not None and b is not None and a.marque is not None and b.marque is not None and a.marque != b.marque
                self.verifier(f"bascule {'plein écran' if plein else 'fenêtre'} : la lecture continue", m.vivant() and continue_ and not m.erreurs(t), "; ".join(m.erreurs(t)))

    def e_fermeture(self):
        self.titre("Fermeture")
        self.assurer()
        code, duree = self.m.quitter()
        self.verifier("le moteur se ferme sur demande", code == 0 and duree < 3, f"code {code} en {duree:.2f} s")
        self.lancer()
        if self.med("a_h264_1080p30.mp4"):
            self.m.envoyer(charger(self.med("a_h264_1080p30.mp4"), boucle=True))
            time.sleep(1.0)
        self.m.p.stdin.close()
        t = time.perf_counter()
        try:
            self.m.p.wait(6)
            self.verifier("le moteur se ferme si la régie disparaît", True, f"{time.perf_counter() - t:.2f} s")
        except Exception:
            self.m.p.kill()
            self.verifier("le moteur se ferme si la régie disparaît", False, "toujours en marche après 6 s")


ESSAIS = ["lecture", "derniere_image", "points", "fondus", "transitions", "preparation", "boucle", "rotation", "pause", "image_son", "illisibles", "navigation", "rafales", "mires", "fermeture"]


def main():
    args = [a for a in sys.argv[1:]]
    # --secours : essaie l'affichage de secours (mpv seul dans la fenêtre) au lieu de l'affichage composé.
    secours = "--secours" in args
    if secours:
        args.remove("--secours")
    opt = {}
    for cle in ("--rapport", "--seulement", "--prefs"):
        if cle in args:
            i = args.index(cle)
            opt[cle] = args[i + 1]
            del args[i:i + 2]
    if len(args) < 2:
        sys.exit(__doc__)
    exe, medias = args[0], args[1]
    import json
    rapport = opt.get("--rapport", time.strftime("recette-%Y%m%d-%H%M%S.txt"))
    r = Recette(exe, medias, json.loads(opt["--prefs"]) if "--prefs" in opt else None, rapport, {"CUESTAR_SECOURS": "1"} if secours else None)
    r.dire(f"Recette du moteur de sortie de CueStar, {time.strftime('%Y-%m-%d %H:%M')}")
    r.dire(f"Système : {platform.platform()} ({platform.machine()}) ; session : {os.environ.get('XDG_SESSION_TYPE', '-')}")
    r.dire(f"Exécutable : {exe}")
    choix = [e for e in ESSAIS if "--seulement" not in opt or any(mot in e for mot in opt["--seulement"].split(","))]
    try:
        if r.e_demarrage():
            for nom in choix:
                r.assurer()
                try:
                    getattr(r, "e_" + nom)()
                except Exception as e:  # un essai qui casse ne doit pas cacher les suivants
                    r.verifier(f"essai « {nom} » mené jusqu'au bout", False, repr(e))
    finally:
        if r.m is not None and r.m.vivant():
            r.m.quitter()
    r.dire()
    r.dire(f"RÉSULTAT : {'tout passe' if r.echecs == 0 else str(r.echecs) + ' échec(s)'}")
    with open(rapport, "w", encoding="utf-8") as f:
        f.write("\n".join(r.lignes) + "\n")
    print("Rapport écrit :", os.path.abspath(rapport))
    sys.exit(0 if r.echecs == 0 else 1)


if __name__ == "__main__":
    main()
