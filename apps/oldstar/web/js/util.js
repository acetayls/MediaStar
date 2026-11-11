/* OldStar : outils communs de la page (DOM, touchers, curseurs).
 *
 * Plancher : Safari 9 (iOS 9) et Chrome 30 (Android 4.4). JavaScript ES5 strict seulement :
 * pas de let/const, fonctions fléchées, classes, fetch, Promise, modules ni chaînes `...`.
 * Pas de Function.prototype.bind ni de classList (absents d'iOS 5 / Android 2.3), pour
 * pouvoir descendre plus tard sans tout reprendre.
 */
var OS = window.OS || {};
window.OS = OS;

(function () {
    'use strict';

    var U = {};
    OS.u = U;

    /* ---------- DOM ---------- */

    U.el = function (tag, cls, text) {
        var e = document.createElement(tag);
        if (cls) { e.className = cls; }
        if (text !== undefined && text !== null) { e.appendChild(document.createTextNode(String(text))); }
        return e;
    };

    U.text = function (e, t) {
        t = (t === undefined || t === null) ? '' : String(t);
        if (e.firstChild && e.firstChild.nodeType === 3 && !e.firstChild.nextSibling) {
            if (e.firstChild.nodeValue !== t) { e.firstChild.nodeValue = t; }
        } else {
            U.empty(e);
            e.appendChild(document.createTextNode(t));
        }
    };

    U.empty = function (e) {
        while (e.firstChild) { e.removeChild(e.firstChild); }
    };

    U.hasClass = function (e, c) {
        return (' ' + e.className + ' ').indexOf(' ' + c + ' ') >= 0;
    };

    U.setClass = function (e, c, on) {
        var has = U.hasClass(e, c);
        if (on && !has) {
            e.className = e.className ? e.className + ' ' + c : c;
        } else if (!on && has) {
            e.className = (' ' + e.className + ' ').replace(' ' + c + ' ', ' ').replace(/^\s+|\s+$/g, '');
        }
    };

    U.byId = function (id) { return document.getElementById(id); };

    U.each = function (list, fn) {
        var i;
        for (i = 0; i < list.length; i++) { fn(list[i], i); }
    };

    U.clamp = function (v, a, b) { return v < a ? a : (v > b ? b : v); };

    U.now = function () { return new Date().getTime(); };

    /* Appel d'une méthode sans bind : U.call(obj, 'nom') rend une fonction. */
    U.call = function (obj, name) {
        return function () { return obj[name].apply(obj, arguments); };
    };

    var raf = window.requestAnimationFrame || window.webkitRequestAnimationFrame ||
        function (fn) { return window.setTimeout(fn, 16); };
    U.frame = function (fn) { raf.call(window, fn); };

    /* ---------- Touchers (avec repli souris, sans le délai de 300 ms) ---------- */

    U.touch = ('ontouchstart' in window);

    /* Position d'un événement toucher ou souris, en coordonnées de la page visible. */
    function pointOf(ev, id) {
        var list, i;
        if (ev.changedTouches) {
            list = ev.changedTouches;
            for (i = 0; i < list.length; i++) {
                if (id === null || list[i].identifier === id) {
                    return { x: list[i].clientX, y: list[i].clientY, id: list[i].identifier };
                }
            }
            return null;
        }
        return { x: ev.clientX, y: ev.clientY, id: null };
    }

    /* Glisser : start(p, ev), move(p), end(p). Le toucher suivi est celui qui a commencé sur
     * l'élément ; la page ne défile pas pendant le geste. */
    U.drag = function (e, h) {
        var active = null;
        var lastTouchEnd = 0;

        function down(ev) {
            if (active !== null) { return; }
            if (!ev.changedTouches && U.now() - lastTouchEnd < 800) { return; }
            if (!ev.changedTouches && ev.button !== undefined && ev.button !== 0) { return; }
            var p = pointOf(ev, null);
            if (!p) { return; }
            active = p.id === null ? 'souris' : p.id;
            if (ev.preventDefault) { ev.preventDefault(); }
            if (h.start) { h.start(p, ev); }
            if (!ev.changedTouches) {
                document.addEventListener('mousemove', move, false);
                document.addEventListener('mouseup', up, false);
            }
        }
        function move(ev) {
            if (active === null) { return; }
            var p = pointOf(ev, active === 'souris' ? null : active);
            if (!p) { return; }
            if (ev.preventDefault) { ev.preventDefault(); }
            if (h.move) { h.move(p); }
        }
        function up(ev) {
            if (active === null) { return; }
            var p = pointOf(ev, active === 'souris' ? null : active);
            if (!p) { return; }
            if (ev.changedTouches) {
                lastTouchEnd = U.now();
                if (ev.preventDefault) { ev.preventDefault(); }
            } else {
                document.removeEventListener('mousemove', move, false);
                document.removeEventListener('mouseup', up, false);
            }
            active = null;
            if (h.end) { h.end(p); }
        }
        e.addEventListener('touchstart', down, false);
        e.addEventListener('touchmove', move, false);
        e.addEventListener('touchend', up, false);
        e.addEventListener('touchcancel', up, false);
        e.addEventListener('mousedown', down, false);
    };

    /* Appui : fn() au relâcher si le doigt n'a presque pas bougé. Laisse la page défiler. */
    U.tap = function (e, fn) {
        var sx = 0, sy = 0, moved = false, lastTouchEnd = 0;
        e.addEventListener('touchstart', function (ev) {
            var t = ev.changedTouches[0];
            sx = t.clientX; sy = t.clientY; moved = false;
            U.setClass(e, 'down', true);
        }, false);
        e.addEventListener('touchmove', function (ev) {
            var t = ev.changedTouches[0];
            if (Math.abs(t.clientX - sx) > 12 || Math.abs(t.clientY - sy) > 12) {
                moved = true;
                U.setClass(e, 'down', false);
            }
        }, false);
        e.addEventListener('touchend', function (ev) {
            U.setClass(e, 'down', false);
            lastTouchEnd = U.now();
            if (ev.preventDefault) { ev.preventDefault(); }
            if (!moved) { fn(ev); }
        }, false);
        e.addEventListener('touchcancel', function () {
            moved = true;
            U.setClass(e, 'down', false);
        }, false);
        e.addEventListener('click', function (ev) {
            if (U.now() - lastTouchEnd < 800) { return; }
            fn(ev);
        }, false);
    };

    /* ---------- Échelles ---------- */

    /* Courbe d'un fader de console : position 0..1 ↔ dB (−∞ en bas, +10 en haut). */
    var TAPER = [[0, -138], [0.03, -80], [0.15, -60], [0.3, -40], [0.42, -30], [0.55, -20],
        [0.68, -10], [0.76, -5], [0.84, 0], [0.92, 5], [1, 10]];

    U.NEG_INF = -32768;

    U.posToRaw = function (pos) {
        var i, a, b, db;
        if (pos <= 0.005) { return U.NEG_INF; }
        for (i = 1; i < TAPER.length; i++) {
            a = TAPER[i - 1]; b = TAPER[i];
            if (pos <= b[0]) {
                db = a[1] + (b[1] - a[1]) * (pos - a[0]) / (b[0] - a[0]);
                return Math.round(db * 20) * 5; /* pas de 0,05 dB */
            }
        }
        return 1000;
    };

    U.rawToPos = function (raw) {
        var i, a, b, db;
        if (raw === null || raw === undefined || raw <= U.NEG_INF) { return 0; }
        db = raw / 100;
        if (db <= TAPER[0][1]) { return 0.006; }
        for (i = 1; i < TAPER.length; i++) {
            a = TAPER[i - 1]; b = TAPER[i];
            if (db <= b[1]) { return a[0] + (b[0] - a[0]) * (db - a[1]) / (b[1] - a[1]); }
        }
        return 1;
    };

    U.rawToDb = function (raw) {
        if (raw === null || raw === undefined) { return '–'; }
        if (raw <= U.NEG_INF) { return '-∞'; }
        var db = raw / 100;
        return (db > 0 ? '+' : '') + db.toFixed(db <= -10 || db >= 10 ? 1 : 2);
    };

    /* Échelle logarithmique 0..1 ↔ [min, max]. */
    U.logToNorm = function (v, min, max) {
        if (!(v > 0)) { return 0; }
        return U.clamp(Math.log(v / min) / Math.log(max / min), 0, 1);
    };
    U.normToLog = function (n, min, max) {
        return min * Math.pow(max / min, U.clamp(n, 0, 1));
    };

    /* ---------- Curseur (fader vertical ou horizontal) ----------
     * opts : vertical, onChange(norm) pendant le geste, onLabel(norm) → texte facultatif.
     * Renvoie { el, set(norm), dragging() }. Le geste est relatif : poser le doigt ne fait
     * pas sauter la valeur, on la déplace en glissant. */
    U.slider = function (opts) {
        var root = U.el('div', opts.vertical ? 'sl sl-v' : 'sl sl-h');
        var track = U.el('div', 'sl-track');
        var fill = U.el('div', 'sl-fill');
        var knob = U.el('div', 'sl-knob');
        track.appendChild(fill);
        track.appendChild(knob);
        root.appendChild(track);
        var value = 0, dragging = false, startP = 0, startV = 0, size = 1;

        function draw() {
            var pct = (value * 100).toFixed(2) + '%';
            if (opts.vertical) {
                knob.style.bottom = pct;
                fill.style.height = pct;
            } else {
                knob.style.left = pct;
                fill.style.width = pct;
            }
        }

        U.drag(root, {
            start: function (p) {
                dragging = true;
                U.setClass(root, 'active', true);
                size = opts.vertical ? track.offsetHeight : track.offsetWidth;
                if (size < 20) { size = 20; }
                startP = opts.vertical ? p.y : p.x;
                startV = value;
            },
            move: function (p) {
                var d = (opts.vertical ? startP - p.y : p.x - startP) / size;
                var v = U.clamp(startV + d, 0, 1);
                if (v !== value) {
                    value = v;
                    draw();
                    if (opts.onChange) { opts.onChange(value); }
                }
            },
            end: function () {
                dragging = false;
                U.setClass(root, 'active', false);
                if (opts.onEnd) { opts.onEnd(value); }
            }
        });

        draw();
        return {
            el: root,
            set: function (v) {
                if (dragging) { return; }
                value = U.clamp(v, 0, 1);
                draw();
            },
            dragging: function () { return dragging; }
        };
    };

    /* Limite un appel à une fois tous les `ms` (le dernier appel passe toujours). */
    U.throttle = function (fn, ms) {
        var last = 0, timer = null, pending = null;
        function run() {
            timer = null;
            last = U.now();
            var a = pending;
            pending = null;
            fn.apply(null, a);
        }
        var f = function () {
            pending = arguments;
            if (timer !== null) { return; }
            var wait = ms - (U.now() - last);
            if (wait <= 0) { run(); } else { timer = window.setTimeout(run, wait); }
        };
        /* Oublie l'appel en attente (par exemple un déplacement arrivé après le relâcher). */
        f.cancel = function () {
            if (timer !== null) { window.clearTimeout(timer); }
            timer = null;
            pending = null;
        };
        return f;
    };
}());
