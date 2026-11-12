/* OldStar : onglet Console (Yamaha CL/QL). Tranches des voies d'entrée ou des mix : nom,
 * vu-mètre, fader, ON ; détail d'une voie : EQ paramétrique 4 bandes et envois vers les mix.
 */
(function () {
    'use strict';

    var U = OS.u;
    var IN = 'MIXER:Current/InCh';
    var MIX = 'MIXER:Current/Mix';

    var pane = null, head = null, info = null, strips = null, detail = null;
    var cfg = null;          /* snap.yamaha */
    var view = 'in';         /* 'in' ou 'mix' */
    var meterEls = { i: [], m: [] };
    var meterVals = { i: [], m: [] };
    var visible = false;
    var viewButtons = {};
    var sof = -1;            /* Sends on Fader : mix choisi, -1 : arrêt */
    var sofBtn = null, sofBar = null, sofPick = null;

    function statusText() {
        if (!cfg) { return ''; }
        if (!cfg.enabled) { return OS.t('Console désactivée dans les réglages', 'Console disabled in settings'); }
        if (cfg.connected) { return OS.t('Console connectée', 'Console connected') + (cfg.model ? ' : ' + cfg.model : ''); }
        return OS.t('Console hors ligne', 'Console offline') + (cfg.error ? ' (' + cfg.error + ')' : '');
    }

    function drawStatus() {
        U.text(info, statusText());
        info.className = 'info ' + (cfg && cfg.connected ? 'ok' : 'bad');
    }

    /* Bouton ON lié à un paramètre 0/1. */
    function onButton(addr, x, y, label, group) {
        var b = U.el('div', 'btn on-btn', label || 'ON');
        var cur = 0;
        OS.watch(addr, x, y, function (v) {
            cur = v ? 1 : 0;
            U.setClass(b, 'lit', cur === 1);
        }, group);
        U.tap(b, function () { OS.setParam(addr, x, y, cur ? 0 : 1); });
        return b;
    }

    /* Fader lié à un niveau (centièmes de dB). */
    function fader(addr, x, y, vertical, group) {
        var val = U.el('div', 'db', '–');
        var send = U.throttle(function (raw) { OS.setParam(addr, x, y, raw); }, 50);
        var s = U.slider({
            vertical: vertical,
            onChange: function (n) {
                var raw = U.posToRaw(n);
                U.text(val, U.rawToDb(raw));
                send(raw);
            }
        });
        OS.watch(addr, x, y, function (v) {
            if (s.dragging()) { return; }
            s.set(U.rawToPos(v));
            U.text(val, U.rawToDb(v));
        }, group);
        return { slider: s, value: val };
    }

    function strip(base, ch, isInput) {
        var st = U.el('div', 'strip');
        var name = U.el('div', 'name', (isInput ? 'CH ' : 'MIX ') + (ch + 1));
        OS.watch(base + '/Label/Name', ch, 0, function (v) {
            U.text(name, v || ((isInput ? 'CH ' : 'MIX ') + (ch + 1)));
        }, 'strips');
        st.appendChild(U.el('div', 'num', String(ch + 1)));
        st.appendChild(name);

        var body = U.el('div', 'strip-body');
        var meter = U.el('div', 'meter');
        var fill = U.el('div', 'meter-fill');
        meter.appendChild(fill);
        if (cfg.meters) { body.appendChild(meter); }
        /* Sends on Fader : le fader et le ON pilotent l'envoi de la voie vers le mix choisi. */
        var useSend = isInput && sof >= 0;
        var f = useSend ? fader(IN + '/ToMix/Level', ch, sof, true, 'strips') : fader(base + '/Fader/Level', ch, 0, true, 'strips');
        body.appendChild(f.slider.el);
        st.appendChild(body);
        st.appendChild(f.value);
        st.appendChild(useSend ? onButton(IN + '/ToMix/On', ch, sof, 'ON', 'strips') : onButton(base + '/Fader/On', ch, 0, 'ON', 'strips'));
        if (useSend) { U.setClass(st, 'sof', true); }
        if (isInput) {
            var sel = U.el('div', 'btn sel-btn', OS.t('EQ / Envois', 'EQ / Sends'));
            U.tap(sel, function () { openDetail(ch); });
            st.appendChild(sel);
        }
        (isInput ? meterEls.i : meterEls.m)[ch] = fill;
        return st;
    }

    function buildStrips() {
        OS.unwatch('strips');
        U.empty(strips);
        meterEls = { i: [], m: [] };
        meterVals = { i: [], m: [] };
        if (!cfg || !cfg.enabled) { return; }
        var row = U.el('div', 'strip-row');
        var n = view === 'in' ? cfg.channels : cfg.mixes, i;
        for (i = 0; i < n; i++) {
            row.appendChild(strip(view === 'in' ? IN : MIX, i, view === 'in'));
        }
        strips.appendChild(row);
        U.setClass(viewButtons['in'], 'on', view === 'in');
        U.setClass(viewButtons.mix, 'on', view === 'mix');
        drawSof();
    }

    /* ---------- Sends on Fader ---------- */

    function mixName(m) {
        var p = OS.param(MIX + '/Label/Name', m, 0);
        return p && p.v ? p.v : 'MIX ' + (m + 1);
    }

    function drawSof() {
        if (!sofBar) { return; }
        U.setClass(sofBtn, 'lit', sof >= 0);
        sofBar.style.display = sof >= 0 ? 'block' : 'none';
        if (sof >= 0) {
            var n = mixName(sof), d = 'MIX ' + (sof + 1);
            U.text(sofBar.firstChild, 'SENDS ON FADER → ' + (n === d ? d : n + ' (' + d + ')'));
        }
        layoutStrips();
    }

    function setSof(m) {
        sof = m;
        sofPick.style.display = 'none';
        if (m >= 0) {
            view = 'in';
            OS.transport.send({ c: 'loadsends', mix: m });
        }
        buildStrips();
    }

    function openSofPick() {
        if (sof >= 0) { setSof(-1); return; }
        U.empty(sofPick);
        sofPick.appendChild(U.el('div', 'list-title', OS.t('Envois vers quel mix ?', 'Sends to which mix?')));
        var i;
        for (i = 0; i < cfg.mixes; i++) {
            (function (m) {
                var b = U.el('div', 'btn seg', mixName(m));
                U.tap(b, function () { setSof(m); });
                sofPick.appendChild(b);
            }(i));
        }
        var c = U.el('div', 'btn seg', OS.t('Annuler', 'Cancel'));
        U.tap(c, function () { sofPick.style.display = 'none'; layoutStrips(); });
        sofPick.appendChild(c);
        sofPick.style.display = 'block';
        layoutStrips();
    }

    /* Les tranches commencent sous la barre (et le bandeau Sends on Fader s'il est affiché). */
    function layoutStrips() {
        if (!strips || !head) { return; }
        var top = head.offsetHeight + (sofBar.style.display === 'none' ? 0 : sofBar.offsetHeight) +
            (sofPick.style.display === 'none' ? 0 : sofPick.offsetHeight);
        strips.style.top = top + 'px';
    }

    /* ---------- Détail d'une voie ---------- */

    function openDetail(ch) {
        closeDetail();
        OS.transport.send({ c: 'load', ch: ch });
        var g = 'detail';
        var box = U.el('div', 'detail-box');
        var top = U.el('div', 'detail-head');
        var title = U.el('div', 'detail-title', 'CH ' + (ch + 1));
        OS.watch(IN + '/Label/Name', ch, 0, function (v) {
            U.text(title, 'CH ' + (ch + 1) + (v ? ' : ' + v : ''));
        }, g);
        var close = U.el('div', 'btn close-btn', OS.t('Fermer', 'Close'));
        U.tap(close, closeDetail);
        top.appendChild(title);
        top.appendChild(close);
        box.appendChild(top);

        var body = U.el('div', 'detail-body scroll');

        /* EQ façon CL/QL (eq.js) */
        var eq = U.el('div', 'section');
        body.appendChild(eq);
        OS.eq.build(eq, ch, cfg, g);

        /* Envois */
        var sends = U.el('div', 'section');
        var sh = U.el('div', 'section-head');
        sh.appendChild(U.el('div', 'section-title', OS.t('Envois vers les mix', 'Sends to mixes')));
        sends.appendChild(sh);
        var m;
        for (m = 0; m < cfg.mixes; m++) {
            var row = U.el('div', 'param');
            var lab = U.el('div', 'param-label', 'MIX ' + (m + 1));
            (function (lab, m) {
                OS.watch(MIX + '/Label/Name', m, 0, function (v) { U.text(lab, v || ('MIX ' + (m + 1))); }, g);
            }(lab, m));
            row.appendChild(lab);
            var f = fader(IN + '/ToMix/Level', ch, m, false, g);
            row.appendChild(f.slider.el);
            row.appendChild(f.value);
            U.setClass(f.value, 'param-value', true);
            row.appendChild(onButton(IN + '/ToMix/On', ch, m, 'ON', g));
            sends.appendChild(row);
        }
        body.appendChild(sends);
        box.appendChild(body);
        detail.appendChild(box);
        detail.style.display = 'block';
    }

    function closeDetail() {
        OS.unwatch('detail');
        U.empty(detail);
        detail.style.display = 'none';
    }

    /* ---------- Onglet ---------- */

    function viewButton(id, label) {
        var b = U.el('div', 'btn seg', label);
        U.tap(b, function () {
            if (view === id) { return; }
            view = id;
            if (id === 'mix') { sof = -1; }
            buildStrips();
        });
        viewButtons[id] = b;
        return b;
    }

    OS.tabs = OS.tabs || [];
    OS.tabs.push({
        id: 'console',
        label: 'Console',
        visible: function (snap) { return !(snap.yamaha && snap.yamaha.kind === 'none'); },
        init: function (p) {
            pane = p;
            U.setClass(pane, 'console', true);
            head = U.el('div', 'bar');
            info = U.el('div', 'info', '');
            var seg = U.el('div', 'segs');
            seg.appendChild(viewButton('in', OS.t('Entrées', 'Inputs')));
            seg.appendChild(viewButton('mix', 'Mix'));
            sofBtn = U.el('div', 'btn seg sof-btn', 'SENDS ON FADER');
            U.tap(sofBtn, openSofPick);
            seg.appendChild(sofBtn);
            head.appendChild(info);
            head.appendChild(seg);
            pane.appendChild(head);
            sofBar = U.el('div', 'sof-bar');
            sofBar.appendChild(U.el('div', 'sof-text', ''));
            var back = U.el('div', 'btn sof-back', OS.t('◀ Retour aux faders', '◀ Back to faders'));
            U.tap(back, function () { setSof(-1); });
            sofBar.appendChild(back);
            sofBar.style.display = 'none';
            pane.appendChild(sofBar);
            sofPick = U.el('div', 'sof-pick');
            sofPick.style.display = 'none';
            pane.appendChild(sofPick);
            window.addEventListener('resize', layoutStrips, false);
            strips = U.el('div', 'strips scroll');
            pane.appendChild(strips);
            detail = U.el('div', 'detail');
            detail.style.display = 'none';
            pane.appendChild(detail);
        },
        build: function (snap) {
            cfg = snap.yamaha || null;
            sof = -1;
            closeDetail();
            drawStatus();
            buildStrips();
            OS.transport.wantMeters(visible && cfg && cfg.meters);
        },
        show: function (on) {
            visible = on;
            if (on) { window.setTimeout(layoutStrips, 0); }
            OS.transport.wantMeters(on && cfg && cfg.meters);
        },
        ev: function (e) {
            if (e.e === 'ys' && cfg) {
                cfg.connected = e.connected;
                cfg.model = e.model;
                cfg.error = e.error;
                drawStatus();
            }
        },
        meters: function (d) {
            if (!visible) { return; }
            var list = view === 'in' ? d.i : d.m;
            var els = view === 'in' ? meterEls.i : meterEls.m;
            var old = view === 'in' ? meterVals.i : meterVals.m;
            var i, v;
            if (!list) { return; }
            for (i = 0; i < els.length && i < list.length; i++) {
                v = list[i];
                if (old[i] !== v && els[i]) {
                    old[i] = v;
                    els[i].style.height = v + '%';
                    els[i].className = 'meter-fill' + (v >= 90 ? ' hot' : (v >= 70 ? ' mid' : ''));
                }
            }
        }
    });
}());
