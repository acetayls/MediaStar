/* OldStar : onglet Micros. Une carte par voie de micro HF (Sennheiser, Shure) : nom,
 * fréquence, pile, HF des deux antennes, niveau audio, mute et alertes. L'état arrive avec les
 * relevés des vu-mètres (champ « w »), quelques fois par seconde, tant que l'onglet est affiché.
 */
(function () {
    'use strict';

    var U = OS.u;
    var T = OS.t;
    var pane = null;
    var grid = null;
    var empty = null;
    var cards = {};     /* 'rx-ch' → éléments de la carte */
    var keys = '';      /* liste des cartes affichées, pour savoir quand reconstruire */
    var visible = false;
    var count = 0;      /* récepteurs dans les réglages */

    var WARN = {
        low_bat: ['Pile faible', 'Low battery'],
        low_rf: ['HF faible', 'Low RF'],
        rf_mute: ['Pas d’émetteur', 'No transmitter'],
        af_peak: ['Saturation', 'Audio peak'],
        rx_mute: ['Récepteur coupé', 'Receiver muted'],
        tx_mute: ['Émetteur coupé', 'Transmitter muted'],
        interference: ['Interférence', 'Interference'],
        encryption: ['Erreur de chiffrement', 'Encryption error']
    };
    /* Alertes qui passent la carte en rouge. */
    var SERIOUS = { low_bat: 1, low_rf: 1, interference: 1, encryption: 1 };

    function freqText(khz) {
        if (!khz) { return ''; }
        var mhz = Math.floor(khz / 1000), rest = String(khz % 1000);
        while (rest.length < 3) { rest = '0' + rest; }
        return mhz + '.' + rest + ' MHz';
    }

    function batText(m) {
        if (m.mins !== null && m.mins !== undefined) {
            var h = Math.floor(m.mins / 60), mn = m.mins % 60;
            return h + ' h ' + (mn < 10 ? '0' : '') + mn;
        }
        if (m.bat !== null && m.bat !== undefined) { return m.bat + ' %'; }
        return '–';
    }

    function vbar(label) {
        var box = U.el('div', 'mic-rf');
        var track = U.el('div', 'mic-rf-track');
        var fill = U.el('div', 'mic-rf-fill');
        track.appendChild(fill);
        box.appendChild(track);
        var lab = U.el('div', 'mic-rf-label', label);
        box.appendChild(lab);
        return { box: box, fill: fill, label: lab };
    }

    function build(m) {
        var c = U.el('div', 'mic');
        var head = U.el('div', 'mic-head');
        var dot = U.el('div', 'dot');
        var name = U.el('div', 'mic-name', '');
        var freq = U.el('div', 'mic-freq', '');
        head.appendChild(dot);
        head.appendChild(name);
        head.appendChild(freq);
        c.appendChild(head);
        var sub = U.el('div', 'mic-sub', '');
        c.appendChild(sub);

        var body = U.el('div', 'mic-body');
        /* Pile : 5 segments et le temps ou le pourcentage. */
        var bat = U.el('div', 'mic-bat');
        var cells = U.el('div', 'mic-bat-cells');
        var segs = [], i;
        for (i = 0; i < 5; i++) {
            segs.push(U.el('div', 'mic-seg'));
            cells.appendChild(segs[i]);
        }
        bat.appendChild(cells);
        var batTxt = U.el('div', 'mic-bat-text', '–');
        bat.appendChild(batTxt);
        body.appendChild(bat);
        /* HF des antennes A et B. */
        var a = vbar('A'), b = vbar('B');
        body.appendChild(a.box);
        body.appendChild(b.box);
        /* Audio : niveau et crête. */
        var af = U.el('div', 'mic-af');
        var afTrack = U.el('div', 'mic-af-track');
        var afFill = U.el('div', 'mic-af-fill');
        var afPeak = U.el('div', 'mic-af-peak');
        afTrack.appendChild(afFill);
        afTrack.appendChild(afPeak);
        af.appendChild(afTrack);
        af.appendChild(U.el('div', 'mic-af-label', 'AF'));
        body.appendChild(af);
        c.appendChild(body);

        var foot = U.el('div', 'mic-foot');
        var mute = U.el('div', 'mic-mute', 'MUTE');
        var warn = U.el('div', 'mic-warn', '');
        foot.appendChild(mute);
        foot.appendChild(warn);
        c.appendChild(foot);
        return { root: c, dot: dot, name: name, freq: freq, sub: sub, segs: segs, batTxt: batTxt, a: a, b: b, afFill: afFill, afPeak: afPeak, mute: mute, warn: warn, last: '' };
    }

    function update(e, m) {
        /* Rien n'a changé depuis le dernier relevé : on ne touche pas au DOM (vieux iPad). */
        var sig = JSON.stringify(m);
        if (sig === e.last) { return; }
        e.last = sig;
        U.text(e.name, m.name || ('#' + (m.rx + 1)));
        U.text(e.freq, freqText(m.freq));
        U.text(e.sub, m.brand + (m.model ? ' ' + m.model : '') + (m.ch > 0 && m.brand === 'Shure' ? ' · ' + T('voie', 'ch') + ' ' + m.ch : ''));
        var bars = m.bars === null || m.bars === undefined ? -1 : m.bars, i;
        for (i = 0; i < 5; i++) {
            e.segs[i].className = 'mic-seg' + (i < bars ? (bars <= 1 ? ' low' : ' on') : '');
        }
        U.text(e.batTxt, m.tx ? batText(m) : '–');
        e.a.fill.style.height = m.rf_a + '%';
        e.b.fill.style.height = m.rf_b + '%';
        U.setClass(e.a.label, 'act', m.ant === 1);
        U.setClass(e.b.label, 'act', m.ant === 2);
        U.text(e.a.label, 'A' + (m.dbm_a !== null && m.dbm_a !== undefined ? ' ' + m.dbm_a : ''));
        U.text(e.b.label, 'B' + (m.dbm_b !== null && m.dbm_b !== undefined ? ' ' + m.dbm_b : ''));
        e.afFill.style.width = m.af + '%';
        e.afPeak.style.left = Math.min(m.peak, 99) + '%';
        U.setClass(e.afFill, 'hot', m.af >= 90);
        e.mute.style.display = m.mute ? 'block' : 'none';

        var texts = [], serious = false;
        if (!m.online) {
            texts.push(T('Récepteur hors ligne', 'Receiver offline'));
            serious = true;
        } else if (!m.tx) {
            texts.push(T('Pas d’émetteur', 'No transmitter'));
        }
        U.each(m.warn || [], function (w) {
            if ((w === 'rf_mute' || w === 'low_rf') && !m.tx) { return; }
            if (WARN[w]) { texts.push(T(WARN[w][0], WARN[w][1])); }
            if (SERIOUS[w] && m.tx) { serious = true; }
        });
        U.text(e.warn, texts.join(' · '));
        e.dot.className = 'dot' + (m.online && m.tx ? ' ok' : (m.online ? ' idle' : ' bad'));
        e.root.className = 'mic' + (serious ? ' alert' : '') + (m.online && !m.tx ? ' notx' : '');
    }

    function onMics(list) {
        var k = [], i;
        for (i = 0; i < list.length; i++) { k.push(list[i].rx + '-' + list[i].ch); }
        var joined = k.join(',');
        if (joined !== keys) {
            keys = joined;
            cards = {};
            U.empty(grid);
            for (i = 0; i < list.length; i++) {
                cards[k[i]] = build(list[i]);
                grid.appendChild(cards[k[i]].root);
            }
        }
        empty.style.display = list.length ? 'none' : 'block';
        for (i = 0; i < list.length; i++) { update(cards[k[i]], list[i]); }
    }

    OS.tabs = OS.tabs || [];
    OS.tabs.push({
        id: 'micros',
        label: T('Micros', 'Mics'),
        visible: function (snap) { return (snap.wireless || 0) > 0; },
        init: function (p) {
            pane = p;
            U.setClass(pane, 'micros', true);
            grid = U.el('div', 'mic-grid');
            empty = U.el('div', 'empty', T('Recherche des récepteurs…', 'Looking for receivers…'));
            pane.appendChild(empty);
            pane.appendChild(grid);
        },
        build: function (snap) {
            count = snap.wireless || 0;
            U.text(empty, count ? T('Recherche des récepteurs…', 'Looking for receivers…') : T('Aucun récepteur dans les réglages d’OldStar.', 'No receiver in OldStar settings.'));
        },
        show: function (on) {
            visible = on;
            OS.transport.wantMeters(on, 'micros');
        },
        meters: function (d) {
            if (!visible || !d.w) { return; }
            onMics(d.w);
        }
    });
}());
