/* OldStar : onglet Fenêtres. Image d'une fenêtre du PC (raccourci des réglages, ou n'importe
 * quelle fenêtre ouverte choisie dans la liste), rafraîchie dès que la précédente est affichée,
 * ajustée à la place disponible.
 *
 * Toucher l'image clique dans la fenêtre. Deux façons de cliquer :
 *  - arrière-plan (postmessage) : la fenêtre reçoit le clic sans passer devant ; certaines
 *    applis l'ignorent (DirectX, certaines applis Qt ou .NET) ;
 *  - premier plan (sendinput) : vrais clics souris, la fenêtre passe devant et la souris du
 *    PC bouge ; marche partout.
 * Page ouverte sur le PC lui-même : arrière-plan par défaut, sinon la fenêtre passerait devant
 * le navigateur à chaque clic. « Fermer » (toujours en haut) ou Échap reviennent à la liste.
 * Glisser à deux doigts (ou la molette de la souris) fait défiler. « Plein écran » masque
 * tout le reste ; le ✕ dans un coin ou Échap en sortent.
 */
(function () {
    'use strict';

    var U = OS.u;
    var pane = null, listBox = null, shortcuts = null, freeBox = null, freeList = null;
    var bar = null, title = null, modeBtn = null, fitBtn = null, hint = null, exitBtn = null;
    var view = null, img = null, msg = null;
    var windows = [];
    var freeAllowed = false;
    var sel = null;          /* { w, z } (raccourci) ou { h, name } (fenêtre libre) ; null : liste */
    var control = true;      /* les touchers cliquent toujours */
    var full = false;        /* plein écran */
    var background = true;   /* clics en arrière-plan */
    var fit = true;          /* image ajustée à la place disponible */
    var visible = false;
    var loading = false;
    var counter = 0;
    var timer = null;
    var lastPoint = { x: 0.5, y: 0.5 };
    var buttons = [];
    var natural = { w: 0, h: 0 };

    /* Page ouverte sur le PC qui fait tourner OldStar ? */
    var local = /^(localhost|127\.|\[?::1\]?$)/.test(window.location.hostname);

    function load(key, def) {
        try {
            var v = window.localStorage.getItem('oldstar.' + key);
            return v === null ? def : v === '1';
        } catch (e) { return def; }
    }
    function save(key, on) {
        try { window.localStorage.setItem('oldstar.' + key, on ? '1' : '0'); } catch (e) { /* rien */ }
    }

    background = load('arriere-plan', local);
    fit = load('ajuster', true);

    function selName() {
        if (!sel) { return ''; }
        if (sel.h !== undefined) { return sel.name; }
        var win = windows[sel.w];
        return win.name + (sel.z >= 0 ? ' · ' + win.zones[sel.z].name : '');
    }

    function fps() {
        var w = sel && sel.h === undefined ? windows[sel.w] : null;
        return w && w.fps ? U.clamp(w.fps, 1, 30) : 4;
    }

    function stop() {
        if (timer !== null) {
            window.clearTimeout(timer);
            timer = null;
        }
    }

    function next(delay) {
        stop();
        if (!visible || !sel) { return; }
        timer = window.setTimeout(fetchFrame, delay);
    }

    function frameUrl() {
        counter++;
        if (sel.h !== undefined) { return '/frame?h=' + sel.h + '&n=' + counter; }
        return '/frame?w=' + sel.w + (sel.z >= 0 ? '&z=' + sel.z : '') + '&n=' + counter;
    }

    function fetchFrame() {
        timer = null;
        if (!visible || !sel || loading) { return; }
        loading = true;
        var started = U.now();
        var want = sel;
        var probe = new Image();
        probe.onload = function () {
            loading = false;
            if (want !== sel) { next(0); return; }
            img.src = probe.src;
            img.style.display = 'block';
            msg.style.display = 'none';
            if (probe.width !== natural.w || probe.height !== natural.h) {
                natural.w = probe.width;
                natural.h = probe.height;
                layout();
            }
            var wait = Math.round(1000 / fps()) - (U.now() - started);
            next(wait > 0 ? wait : 0);
        };
        probe.onerror = function () {
            loading = false;
            if (want !== sel) { next(0); return; }
            img.style.display = 'none';
            U.text(msg, 'Fenêtre « ' + selName() + ' » introuvable ou réduite sur le PC. Nouvel essai…');
            msg.style.display = 'block';
            next(2000);
        };
        probe.src = frameUrl();
    }

    /* ---------- Barre d'outils ---------- */

    function setBackground(on) {
        background = !!on;
        save('arriere-plan', background);
        U.text(modeBtn, background ? 'Clic : arrière-plan' : 'Clic : premier plan');
        drawHint();
    }

    function setFit(on) {
        fit = !!on;
        save('ajuster', fit);
        U.text(fitBtn, fit ? 'Taille réelle' : 'Ajuster');
        layout();
    }

    function drawHint() {
        var t = '';
        if (background) {
            t = 'Clics en arrière-plan : la fenêtre ne passe pas devant. Si elle ne réagit pas ' +
                '(DirectX, certaines applis Qt ou .NET), passez en premier plan.';
        } else if (local) {
            t = 'Premier plan sur le PC lui-même : chaque clic passe la fenêtre devant le navigateur. ' +
                'Échap ou « Fermer » pour revenir.';
        } else {
            t = 'Clics en premier plan : la fenêtre passe devant sur le PC et la souris bouge.';
        }
        U.text(hint, t);
        layout();
    }

    /* L'image occupe la place sous la barre, sans défilement de la page. */
    function layout() {
        if (!view || !bar || !sel) { return; }
        view.style.top = full ? '0' : bar.offsetHeight + 'px';
        if (!natural.w || !natural.h) { return; }
        if (fit) {
            var pad = full ? 0 : 16;
            var aw = view.clientWidth - pad, ah = view.clientHeight - pad;
            var ratio = natural.w / natural.h;
            var w = Math.min(aw, ah * ratio);
            if (w < 40) { w = 40; }
            img.style.width = Math.floor(w) + 'px';
            img.style.height = Math.floor(w / ratio) + 'px';
            U.setClass(view, 'fit', true);
        } else {
            img.style.width = natural.w + 'px';
            img.style.height = natural.h + 'px';
            U.setClass(view, 'fit', false);
        }
    }

    /* ---------- Plein écran ---------- */

    function nativeFullElement() {
        return document.fullscreenElement || document.webkitFullscreenElement || null;
    }

    /* Plein écran « dans la page » (barres et onglets masqués), et celui du navigateur quand
     * il existe (pas pour les éléments sur iOS 9 ; l'iPad ajouté à l'écran d'accueil est déjà
     * en plein écran). */
    function enterFull() {
        if (!sel || full) { return; }
        full = true;
        U.setClass(document.body, 'full', true);
        exitBtn.style.display = 'block';
        var root = document.documentElement;
        try {
            if (root.requestFullscreen) { root.requestFullscreen(); } else if (root.webkitRequestFullscreen) { root.webkitRequestFullscreen(); }
        } catch (e) { /* refusé : le plein écran dans la page suffit */ }
        layout();
        window.setTimeout(layout, 300);
    }

    function exitFull() {
        if (!full) { return; }
        full = false;
        U.setClass(document.body, 'full', false);
        exitBtn.style.display = 'none';
        if (nativeFullElement()) {
            try {
                if (document.exitFullscreen) { document.exitFullscreen(); } else if (document.webkitExitFullscreen) { document.webkitExitFullscreen(); }
            } catch (e) { /* rien */ }
        }
        layout();
        window.setTimeout(layout, 300);
    }

    /* Sortie du plein écran du navigateur (touche Échap gérée par lui) : on sort aussi du nôtre. */
    function onNativeChange() {
        if (full && !nativeFullElement()) { exitFull(); }
        window.setTimeout(layout, 100);
    }

    /* ---------- Choix de la fenêtre ---------- */

    function choose(s) {
        sel = s;
        natural = { w: 0, h: 0 };
        U.text(title, selName());
        listBox.style.display = 'none';
        bar.style.display = 'block';
        view.style.display = 'block';
        img.style.display = 'none';
        U.text(msg, 'Chargement…');
        msg.style.display = 'block';
        drawHint();
        next(0);
    }

    /* Retour à la liste des fenêtres : plus de capture, plus de clics. */
    function close() {
        sendMove.cancel();
        exitFull();
        sel = null;
        stop();
        bar.style.display = 'none';
        view.style.display = 'none';
        img.style.display = 'none';
        listBox.style.display = 'block';
        if (freeAllowed && visible) { refreshFree(); }
    }

    function refreshFree() {
        U.empty(freeList);
        freeList.appendChild(U.el('div', 'empty small', 'Recherche des fenêtres…'));
        OS.transport.get('/windows', function (r) {
            U.empty(freeList);
            if (!r) {
                freeList.appendChild(U.el('div', 'empty small', 'Liste indisponible (PC injoignable ?).'));
                return;
            }
            if (!r.available) {
                freeList.appendChild(U.el('div', 'empty small', 'Capture des fenêtres non disponible sur ce système.'));
                return;
            }
            if (!r.list.length) {
                freeList.appendChild(U.el('div', 'empty small', 'Aucune fenêtre ouverte trouvée.'));
                return;
            }
            U.each(r.list, function (w) {
                var b = U.el('div', 'win-item' + (w.min ? ' min' : ''));
                b.appendChild(U.el('div', 'win-item-title', w.title));
                b.appendChild(U.el('div', 'win-item-exe', w.exe + (w.min ? ' · réduite : à rouvrir sur le PC' : '')));
                U.tap(b, function () { choose({ h: w.id, name: w.title }); });
                freeList.appendChild(b);
            });
        });
    }

    /* Point du doigt → position relative (0 à 1) dans l'image affichée. */
    function rel(p) {
        var r = img.getBoundingClientRect();
        var w = r.right - r.left, h = r.bottom - r.top;
        if (w <= 0 || h <= 0) { return null; }
        return { x: U.clamp((p.x - r.left) / w, 0, 1), y: U.clamp((p.y - r.top) / h, 0, 1) };
    }

    function input(kind, q, wheel) {
        if (!sel || !q || !control) { return; }
        var c = { c: 'input', k: kind, x: q.x, y: q.y, wheel: wheel || 0, m: background ? 'postmessage' : 'sendinput' };
        if (sel.h !== undefined) {
            c.h = sel.h;
        } else {
            c.w = sel.w;
            c.z = sel.z >= 0 ? sel.z : null;
        }
        OS.transport.send(c);
    }

    var sendMove = U.throttle(function (q) { input('move', q, 0); }, 50);

    function button(label, cls, fn) {
        var b = U.el('div', 'btn ' + (cls || ''), label);
        U.tap(b, fn);
        return b;
    }

    function build(snap) {
        windows = snap.windows || [];
        freeAllowed = !!snap.free_windows;
        U.empty(shortcuts);
        buttons = [];
        if (windows.length) {
            shortcuts.appendChild(U.el('div', 'list-title', 'Raccourcis'));
            U.each(windows, function (w, wi) {
                var b = button(w.name, 'seg', function () { choose({ w: wi, z: -1 }); });
                shortcuts.appendChild(b);
                U.each(w.zones || [], function (z, zi) {
                    shortcuts.appendChild(button(w.name + ' · ' + z.name, 'seg zone', function () { choose({ w: wi, z: zi }); }));
                });
            });
        }
        freeBox.style.display = freeAllowed ? 'block' : 'none';
        if (!windows.length && !freeAllowed) {
            shortcuts.appendChild(U.el('div', 'empty', 'Aucune fenêtre dans les réglages d’OldStar.'));
        }
        close();
    }

    OS.tabs = OS.tabs || [];
    OS.tabs.push({
        id: 'fenetres',
        label: 'Fenêtres',
        init: function (p) {
            pane = p;
            U.setClass(pane, 'fenetres', true);
            U.setClass(pane, 'fixed', true);

            /* Liste : raccourcis des réglages, puis toutes les fenêtres ouvertes. */
            listBox = U.el('div', 'win-list scroll');
            shortcuts = U.el('div', 'win-shortcuts');
            listBox.appendChild(shortcuts);
            freeBox = U.el('div', 'win-free');
            var fh = U.el('div', 'win-row');
            fh.appendChild(U.el('div', 'list-title grow', 'Choisir une fenêtre ouverte du PC'));
            fh.appendChild(button('Actualiser', '', refreshFree));
            freeBox.appendChild(fh);
            freeList = U.el('div', 'win-items');
            freeBox.appendChild(freeList);
            listBox.appendChild(freeBox);
            pane.appendChild(listBox);

            /* Barre du haut de la vue : jamais recouverte par l'image. */
            bar = U.el('div', 'win-bar');
            var row = U.el('div', 'win-row');
            row.appendChild(button('◀ Fermer', 'close-btn big-close', close));
            title = U.el('div', 'win-title', '');
            row.appendChild(title);
            bar.appendChild(row);
            var tools = U.el('div', 'win-row');
            modeBtn = button('', '', function () { setBackground(!background); });
            fitBtn = button('', '', function () { setFit(!fit); });
            tools.appendChild(button('Plein écran', 'accent', enterFull));
            tools.appendChild(modeBtn);
            tools.appendChild(fitBtn);
            tools.appendChild(U.el('div', 'win-note', 'Défiler : glisser à deux doigts (molette sur PC)'));
            bar.appendChild(tools);
            hint = U.el('div', 'win-hint', '');
            bar.appendChild(hint);
            bar.style.display = 'none';
            pane.appendChild(bar);

            view = U.el('div', 'shot scroll');
            img = U.el('img', 'shot-img');
            img.style.display = 'none';
            img.setAttribute('alt', '');
            msg = U.el('div', 'empty', '');
            msg.style.display = 'none';
            view.appendChild(img);
            view.appendChild(msg);
            view.style.display = 'none';
            pane.appendChild(view);

            /* ✕ du plein écran : par-dessus l'image, dans un coin ; ses touchers ne vont pas à l'image. */
            exitBtn = U.el('div', 'full-exit', '✕');
            exitBtn.style.display = 'none';
            U.tap(exitBtn, exitFull);
            pane.appendChild(exitBtn);
            document.addEventListener('fullscreenchange', onNativeChange, false);
            document.addEventListener('webkitfullscreenchange', onNativeChange, false);
            setBackground(background);
            setFit(fit);

            /* Gestes : appui court = clic (envoyé après TAP_WAIT faute de second appui),
             * deux appuis rapprochés = double-clic (une seule commande, au point du premier),
             * appui long relâché sans bouger = clic droit, glisser (même après un appui long,
             * comme on prend un module dans Lake) = bouton enfoncé pendant le déplacement. */
            var TAP_WAIT = 300, DOUBLE_MS = 400, DOUBLE_PX = 20, LONG_MS = 650, MOVE_PX = 8;
            var g = { q: null, p: null, dragging: false, longDone: false, longTimer: null, multi: false };
            var pendingTap = null; /* { q, p, t, timer } */

            function farther(a, b, px) {
                return Math.abs(a.x - b.x) > px || Math.abs(a.y - b.y) > px;
            }

            U.drag(img, {
                start: function (p) {
                    if (g.multi) { return; }
                    g.q = rel(p);
                    g.p = { x: p.x, y: p.y };
                    g.dragging = false;
                    g.longDone = false;
                    if (g.q) { lastPoint = g.q; }
                    if (g.longTimer !== null) { window.clearTimeout(g.longTimer); }
                    g.longTimer = window.setTimeout(function () {
                        g.longTimer = null;
                        if (!g.dragging) { g.longDone = true; }
                    }, LONG_MS);
                },
                move: function (p) {
                    var q = rel(p);
                    if (!q || g.multi) { return; }
                    lastPoint = q;
                    if (!g.dragging && farther(p, g.p, MOVE_PX)) {
                        g.dragging = true;
                        if (g.longTimer !== null) { window.clearTimeout(g.longTimer); g.longTimer = null; }
                        if (pendingTap) {
                            window.clearTimeout(pendingTap.timer);
                            input('click', pendingTap.q, 0);
                            pendingTap = null;
                        }
                        input('down', g.q, 0);
                    }
                    if (g.dragging) { sendMove(q); }
                },
                end: function (p) {
                    if (g.longTimer !== null) { window.clearTimeout(g.longTimer); g.longTimer = null; }
                    if (g.multi) { return; }
                    if (g.dragging) {
                        sendMove.cancel();
                        input('up', rel(p) || lastPoint, 0);
                        return;
                    }
                    if (g.longDone) {
                        if (pendingTap) {
                            window.clearTimeout(pendingTap.timer);
                            input('click', pendingTap.q, 0);
                            pendingTap = null;
                        }
                        input('rclick', g.q, 0);
                        return;
                    }
                    var now = U.now();
                    if (pendingTap && now - pendingTap.t < DOUBLE_MS && !farther(g.p, pendingTap.p, DOUBLE_PX)) {
                        window.clearTimeout(pendingTap.timer);
                        input('dblclick', pendingTap.q, 0);
                        pendingTap = null;
                        return;
                    }
                    if (pendingTap) {
                        window.clearTimeout(pendingTap.timer);
                        input('click', pendingTap.q, 0);
                    }
                    var tap = { q: g.q, p: g.p, t: now, timer: null };
                    tap.timer = window.setTimeout(function () {
                        if (pendingTap === tap) { pendingTap = null; }
                        input('click', tap.q, 0);
                    }, TAP_WAIT);
                    pendingTap = tap;
                }
            });

            /* Deux doigts : défilement (molette), un cran tous les WHEEL_PX de glissement. */
            var WHEEL_PX = 30, twoY = 0, twoAcc = 0;
            function midY(ev) { return (ev.touches[0].clientY + ev.touches[1].clientY) / 2; }
            img.addEventListener('touchstart', function (ev) {
                if (ev.touches.length < 2) { return; }
                if (!g.multi) {
                    g.multi = true;
                    if (g.longTimer !== null) { window.clearTimeout(g.longTimer); g.longTimer = null; }
                    if (g.dragging) {
                        sendMove.cancel();
                        input('up', lastPoint, 0);
                        g.dragging = false;
                    }
                }
                twoY = midY(ev);
                twoAcc = 0;
                ev.preventDefault();
            }, false);
            img.addEventListener('touchmove', function (ev) {
                if (!g.multi || ev.touches.length < 2) { return; }
                ev.preventDefault();
                var y = midY(ev);
                twoAcc += y - twoY;
                twoY = y;
                while (twoAcc >= WHEEL_PX) { twoAcc -= WHEEL_PX; input('wheel', lastPoint, 120); }
                while (twoAcc <= -WHEEL_PX) { twoAcc += WHEEL_PX; input('wheel', lastPoint, -120); }
            }, false);
            img.addEventListener('touchend', function (ev) {
                if (g.multi && ev.touches.length === 0) { g.multi = false; }
            }, false);

            /* Molette de la souris sur l'image : transmise telle quelle (un cran par événement). */
            function onWheel(ev) {
                var q = rel({ x: ev.clientX, y: ev.clientY });
                var d = ev.deltaY !== undefined ? -ev.deltaY : (ev.wheelDelta || 0);
                if (!q || !d) { return; }
                ev.preventDefault();
                lastPoint = q;
                input('wheel', q, d > 0 ? 120 : -120);
            }
            if ('onwheel' in img) {
                img.addEventListener('wheel', onWheel, false);
            } else {
                img.addEventListener('mousewheel', onWheel, false);
            }

            window.addEventListener('resize', layout, false);
            window.addEventListener('orientationchange', function () { window.setTimeout(layout, 300); }, false);

            /* Échap : arrête les clics, puis revient à la liste. */
            document.addEventListener('keydown', function (ev) {
                if (ev.keyCode !== 27 || !visible || !sel) { return; }
                if (full) { exitFull(); } else { close(); }
            }, false);
        },
        build: build,
        show: function (on) {
            visible = on;
            if (!on) { exitFull(); }
            if (on) {
                if (sel) { next(0); } else if (freeAllowed) { refreshFree(); }
            } else {
                stop();
            }
        }
    });
}());
