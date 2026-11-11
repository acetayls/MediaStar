/* OldStar : état partagé de la page, onglets et aiguillage des messages du serveur.
 * Chargé en dernier : util.js, transport.js, puis les onglets (console.js, applis.js,
 * fenetres.js) qui s'inscrivent dans OS.tabs.
 */
(function () {
    'use strict';

    var U = OS.u;

    /* ---------- Paramètres de la console ---------- */

    var params = {};    /* 'adresse|x|y' → { v, s } */
    var watchers = {};  /* 'adresse|x|y' → [ { fn, g } ] */

    function key(a, x, y) { return a + '|' + x + '|' + y; }

    OS.param = function (a, x, y) {
        return params[key(a, x, y)] || null;
    };

    /* fn(v, s) à chaque changement ; `group` permet de tout oublier d'un coup (OS.unwatch). */
    OS.watch = function (a, x, y, fn, group) {
        var k = key(a, x, y);
        if (!watchers[k]) { watchers[k] = []; }
        watchers[k].push({ fn: fn, g: group || '' });
        var p = params[k];
        if (p) { fn(p.v, p.s); }
    };

    OS.unwatch = function (group) {
        var k, list, i, keep;
        for (k in watchers) {
            if (watchers.hasOwnProperty(k)) {
                list = watchers[k];
                keep = [];
                for (i = 0; i < list.length; i++) {
                    if (list[i].g !== group) { keep.push(list[i]); }
                }
                watchers[k] = keep;
            }
        }
    };

    function setLocal(a, x, y, v, s) {
        var k = key(a, x, y);
        params[k] = { v: v, s: s };
        var list = watchers[k], i;
        if (!list) { return; }
        for (i = 0; i < list.length; i++) { list[i].fn(v, s); }
    }

    /* Change un paramètre : affiché tout de suite, envoyé à la console par le serveur. */
    OS.setParam = function (a, x, y, v) {
        var old = params[key(a, x, y)];
        setLocal(a, x, y, v, old && old.v === v ? old.s : '');
        OS.transport.send({ c: 'set', a: a, x: x, y: y, v: v });
    };

    /* ---------- Retour d'état OSC ---------- */

    var oscValues = {}; /* 'cible|adresse' → valeur */
    OS.oscValue = function (t, a) {
        var v = oscValues[t + '|' + a];
        return v === undefined ? null : v;
    };

    /* ---------- Onglets ---------- */

    OS.tabs = OS.tabs || [];  /* { id, label, build(snap), show(oui), ev(e), meters(d) } */
    var active = '';
    var snap = null;

    function tabById(id) {
        var i;
        for (i = 0; i < OS.tabs.length; i++) {
            if (OS.tabs[i].id === id) { return OS.tabs[i]; }
        }
        return null;
    }

    function selectTab(id) {
        var t = tabById(id);
        if (!t) { return; }
        active = id;
        U.each(OS.tabs, function (o) {
            U.setClass(o.button, 'on', o.id === id);
            o.pane.style.display = o.id === id ? 'block' : 'none';
            if (o.show) { o.show(o.id === id); }
        });
        try { window.localStorage.setItem('oldstar.onglet', id); } catch (e) { /* navigation privée */ }
    }
    OS.activeTab = function () { return active; };

    function buildTabs() {
        var bar = U.byId('tabs');
        var main = U.byId('main');
        U.each(OS.tabs, function (t) {
            var b = U.el('div', 'tab', t.label);
            U.tap(b, function () { selectTab(t.id); });
            bar.appendChild(b);
            t.button = b;
            var pane = U.el('div', 'pane scroll');
            pane.style.display = 'none';
            main.appendChild(pane);
            t.pane = pane;
            if (t.init) { t.init(pane); }
        });
        var saved = '';
        try { saved = window.localStorage.getItem('oldstar.onglet') || ''; } catch (e) { saved = ''; }
        selectTab(tabById(saved) ? saved : OS.tabs[0].id);
    }

    /* ---------- État de la liaison ---------- */

    function status(text, cls) {
        var s = U.byId('link');
        U.text(s, text);
        s.className = 'link ' + cls;
    }

    /* ---------- Messages du serveur ---------- */

    function onSnap(d) {
        snap = d;
        params = {};
        oscValues = {};
        U.each(d.params || [], function (p) {
            params[key(p[0], p[1], p[2])] = { v: p[3], s: p[4] };
        });
        U.each(d.osc || [], function (o) { oscValues[o[0] + '|' + o[1]] = o[2]; });
        U.each(OS.tabs, function (t) {
            if (t.build) { t.build(d); }
        });
    }

    function onEvent(e) {
        if (e.e === 'p') {
            setLocal(e.a, e.x, e.y, e.v, e.s);
            return;
        }
        if (e.e === 'of') { oscValues[e.t + '|' + e.a] = e.v; }
        if (e.e === 'cfg') {
            OS.transport.resync();
            return;
        }
        U.each(OS.tabs, function (t) {
            if (t.ev) { t.ev(e); }
        });
    }

    function onMessage(m) {
        if (m.t === 'snap') {
            onSnap(m.d);
        } else if (m.t === 'ev') {
            U.each(m.d || [], onEvent);
        } else if (m.t === 'mtr') {
            U.each(OS.tabs, function (t) {
                if (t.meters) { t.meters(m.d); }
            });
        }
    }

    OS.snap = function () { return snap; };

    /* ---------- Démarrage ---------- */

    function start() {
        /* Pas de défilement élastique de toute la page (iOS) : seules les zones .scroll défilent. */
        document.addEventListener('touchmove', function (ev) {
            var n = ev.target;
            while (n && n !== document.body) {
                if (n.className && U.hasClass(n, 'scroll')) { return; }
                n = n.parentNode;
            }
            ev.preventDefault();
        }, false);

        buildTabs();
        status('Connexion…', 'wait');
        OS.transport.connect({
            open: function (mode) {
                status(mode === 'ws' ? 'En ligne' : 'En ligne (lent)', 'ok');
            },
            message: onMessage,
            close: function (reason) {
                status('Hors ligne' + (reason ? ' : ' + reason : ''), 'bad');
            }
        });
    }

    if (document.readyState === 'complete' || document.readyState === 'interactive') {
        start();
    } else {
        document.addEventListener('DOMContentLoaded', start, false);
    }
}());
