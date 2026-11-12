/* OldStar : onglet Applis. Boutons OSC vers CueStar et JingleStar (définis dans les réglages
 * d'OldStar), retour d'état de chaque appli quand elle en envoie.
 */
(function () {
    'use strict';

    var U = OS.u;
    var pane = null;
    var cards = [];   /* par cible : { dot, values: { adresse: el } } */
    var bigs = [];    /* gros boutons, pour ajuster leur hauteur à l'écran */
    var shown = false;

    /* Boutons aussi hauts que possible sans faire défiler l'onglet. */
    function fit() {
        if (!pane || !shown || !bigs.length) { return; }
        var sizes = [72, 64, 56, 48, 42, 36, 32], i, j;
        for (i = 0; i < sizes.length; i++) {
            for (j = 0; j < bigs.length; j++) {
                bigs[j].style.minHeight = sizes[i] + 'px';
                bigs[j].style.lineHeight = (sizes[i] - 2) + 'px';
            }
            if (pane.scrollHeight <= pane.clientHeight) { return; }
        }
    }

    function fmt(v) {
        if (v === null || v === undefined || v === '') { return '–'; }
        if (v === true) { return OS.t('oui', 'yes'); }
        if (v === false) { return OS.t('non', 'no'); }
        return String(v);
    }

    function card(t, ti, alive) {
        var c = U.el('div', 'card');
        var head = U.el('div', 'card-head');
        var dot = U.el('div', 'dot' + (alive ? ' ok' : ''));
        head.appendChild(dot);
        head.appendChild(U.el('div', 'card-title', t.name));
        c.appendChild(head);

        var values = {};
        if (t.feedback && t.feedback.length) {
            var fb = U.el('div', 'feedback');
            U.each(t.feedback, function (f) {
                var row = U.el('div', 'fb-row');
                row.appendChild(U.el('div', 'fb-label', f.label));
                var v = U.el('div', 'fb-value', fmt(OS.oscValue(ti, f.a)));
                row.appendChild(v);
                fb.appendChild(row);
                values[f.a] = v;
            });
            c.appendChild(fb);
        }

        var grid = U.el('div', 'grid');
        U.each(t.buttons || [], function (b, bi) {
            var cell = U.el('div', 'cell' + (b.wide ? ' wide' : ''));
            var btn = U.el('div', 'btn big' + (b.style ? ' ' + b.style : ''), b.label);
            bigs.push(btn);
            U.tap(btn, function () {
                OS.transport.send({ c: 'osc', t: ti, b: bi });
                U.setClass(btn, 'flash', true);
                window.setTimeout(function () { U.setClass(btn, 'flash', false); }, 180);
            });
            cell.appendChild(btn);
            grid.appendChild(cell);
        });
        c.appendChild(grid);
        cards[ti] = { dot: dot, values: values };
        return c;
    }

    OS.tabs = OS.tabs || [];
    OS.tabs.push({
        id: 'applis',
        label: OS.t('Applis', 'Apps'),
        init: function (p) {
            pane = p;
            U.setClass(pane, 'applis', true);
            window.addEventListener('resize', fit, false);
            window.addEventListener('orientationchange', function () { window.setTimeout(fit, 300); }, false);
        },
        show: function (on) {
            shown = on;
            if (on) { fit(); }
        },
        build: function (snap) {
            U.empty(pane);
            cards = [];
            bigs = [];
            var targets = snap.targets || [];
            if (!targets.length) {
                pane.appendChild(U.el('div', 'empty', OS.t('Aucune appli dans les réglages d’OldStar.', 'No app in OldStar settings.')));
                return;
            }
            var alive = snap.osc_alive || [];
            U.each(targets, function (t, i) {
                var c = card(t, i, !!alive[i]);
                if (i === targets.length - 1) { U.setClass(c, 'last', true); }
                pane.appendChild(c);
            });
            fit();
        },
        ev: function (e) {
            if (e.e === 'of' && cards[e.t] && cards[e.t].values[e.a]) {
                U.text(cards[e.t].values[e.a], fmt(e.v));
            } else if (e.e === 'os') {
                U.each(cards, function (c, i) {
                    if (c) { c.dot.className = 'dot' + (e.alive[i] ? ' ok' : ''); }
                });
            }
        }
    });
}());
