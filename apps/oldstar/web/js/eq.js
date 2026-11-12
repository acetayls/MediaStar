/* OldStar : égaliseur d'une voie, à la manière de l'écran d'une CL/QL. Courbe de réponse
 * dessinée (canvas 2D), points des bandes déplaçables (X = fréquence, Y = gain), et pour
 * chaque bande LOW / LOW-MID / HIGH-MID / HIGH des encodeurs GAIN, FREQ et Q (glisser
 * verticalement). HPF séparé (ON + fréquence), EQ ON et FLAT.
 *
 * Paramètres : MIXER:Current/InCh/PEQ/On, PEQ/Band/Gain|Freq|Q (x = voie, y = bande).
 * Supposés, à vérifier sur une vraie console : PEQ/Band/Type (0 cloche, 1 plateau, 2 coupe-haut
 * pour HIGH), HPF/On et HPF/Freq (Hz × freq_scale comme les bandes).
 */
(function () {
    'use strict';

    var U = OS.u;
    var IN = 'MIXER:Current/InCh';
    var FS = 48000;
    var F_MIN = 20, F_MAX = 20000, G_MAX = 18, Q_MIN = 0.1, Q_MAX = 16, HPF_MIN = 20, HPF_MAX = 600;
    var NAMES = ['LOW', 'LOW-MID', 'HIGH-MID', 'HIGH'];
    var COLORS = ['#4a90e2', '#46c855', '#f0b030', '#d94848'];
    var HPF_COLOR = '#b07ae0';

    /* ---------- Réponse des filtres (biquads RBJ) ---------- */

    function coeffs(type, f0, gdb, q) {
        f0 = U.clamp(f0, 10, FS * 0.45);
        var A = Math.pow(10, gdb / 40), w0 = 2 * Math.PI * f0 / FS, cs = Math.cos(w0), sn = Math.sin(w0);
        var alpha, sq;
        if (type === 'peak') {
            alpha = sn / (2 * q);
            return [1 + alpha * A, -2 * cs, 1 - alpha * A, 1 + alpha / A, -2 * cs, 1 - alpha / A];
        }
        if (type === 'lowshelf' || type === 'highshelf') {
            alpha = sn / 2 * Math.SQRT2;
            sq = 2 * Math.sqrt(A) * alpha;
            if (type === 'lowshelf') {
                return [A * ((A + 1) - (A - 1) * cs + sq), 2 * A * ((A - 1) - (A + 1) * cs), A * ((A + 1) - (A - 1) * cs - sq),
                    (A + 1) + (A - 1) * cs + sq, -2 * ((A - 1) + (A + 1) * cs), (A + 1) + (A - 1) * cs - sq];
            }
            return [A * ((A + 1) + (A - 1) * cs + sq), -2 * A * ((A - 1) + (A + 1) * cs), A * ((A + 1) + (A - 1) * cs - sq),
                (A + 1) - (A - 1) * cs + sq, 2 * ((A - 1) - (A + 1) * cs), (A + 1) - (A - 1) * cs - sq];
        }
        alpha = sn / (2 * 0.7071);
        if (type === 'lpf') {
            return [(1 - cs) / 2, 1 - cs, (1 - cs) / 2, 1 + alpha, -2 * cs, 1 - alpha];
        }
        return [(1 + cs) / 2, -(1 + cs), (1 + cs) / 2, 1 + alpha, -2 * cs, 1 - alpha];
    }

    function magDb(c, f) {
        var w = 2 * Math.PI * f / FS, c1 = Math.cos(w), s1 = Math.sin(w), c2 = Math.cos(2 * w), s2 = Math.sin(2 * w);
        var nr = c[0] + c[1] * c1 + c[2] * c2, ni = -(c[1] * s1 + c[2] * s2);
        var dr = c[3] + c[4] * c1 + c[5] * c2, di = -(c[4] * s1 + c[5] * s2);
        var r = (nr * nr + ni * ni) / (dr * dr + di * di);
        return r > 0 ? 10 * Math.log(r) / Math.LN10 : -60;
    }

    function bandType(b, t) {
        if (b === 0 && t === 1) { return 'lowshelf'; }
        if (b === 3 && t === 1) { return 'highshelf'; }
        if (b === 3 && t === 2) { return 'lpf'; }
        return 'peak';
    }

    /* ---------- Encodeur rotatif (glisser verticalement) ---------- */

    function encoder(label, color, onChange, fmt) {
        var root = U.el('div', 'enc');
        var cv = document.createElement('canvas');
        var size = 54, ratio = window.devicePixelRatio || 1;
        cv.width = size * ratio;
        cv.height = size * ratio;
        cv.style.width = size + 'px';
        cv.style.height = size + 'px';
        cv.className = 'enc-knob';
        var val = U.el('div', 'enc-value', '–');
        root.appendChild(U.el('div', 'enc-label', label));
        root.appendChild(cv);
        root.appendChild(val);
        var norm = 0, raw = null, dragging = false, startY = 0, startN = 0;

        function draw() {
            var g = cv.getContext('2d');
            g.setTransform(ratio, 0, 0, ratio, 0, 0);
            g.clearRect(0, 0, size, size);
            var a0 = Math.PI * 0.75, a1 = Math.PI * 2.25, r = size / 2 - 5;
            g.lineWidth = 5;
            g.strokeStyle = '#3a3a43';
            g.beginPath();
            g.arc(size / 2, size / 2, r, a0, a1, false);
            g.stroke();
            g.strokeStyle = color;
            g.beginPath();
            g.arc(size / 2, size / 2, r, a0, a0 + (a1 - a0) * norm, false);
            g.stroke();
            g.fillStyle = dragging ? '#f0b030' : '#2e2e35';
            g.beginPath();
            g.arc(size / 2, size / 2, r - 7, 0, Math.PI * 2, false);
            g.fill();
            var a = a0 + (a1 - a0) * norm;
            g.strokeStyle = '#dddde2';
            g.lineWidth = 2;
            g.beginPath();
            g.moveTo(size / 2, size / 2);
            g.lineTo(size / 2 + Math.cos(a) * (r - 8), size / 2 + Math.sin(a) * (r - 8));
            g.stroke();
        }

        U.drag(root, {
            start: function (p) { dragging = true; startY = p.y; startN = norm; draw(); },
            move: function (p) {
                var n = U.clamp(startN + (startY - p.y) / 160, 0, 1);
                if (n !== norm) {
                    norm = n;
                    draw();
                    U.text(val, fmt(onChange(norm)));
                }
            },
            end: function () { dragging = false; draw(); }
        });
        draw();
        return {
            el: root,
            set: function (n, text) {
                if (dragging) { return; }
                norm = U.clamp(n, 0, 1);
                U.text(val, text);
                draw();
            }
        };
    }

    /* ---------- Égaliseur d'une voie ---------- */

    OS.eq = {};

    /* Construit l'égaliseur de la voie `ch` dans `box`. `cfg` : snap.yamaha (échelles). */
    OS.eq.build = function (box, ch, cfg, group) {
        var fs = cfg.freq_scale || 10, qs = cfg.q_scale || 1000;
        var st = { on: 1, hpfOn: 0, hpfF: 80, bands: [] };
        var b;
        for (b = 0; b < 4; b++) { st.bands.push({ g: 0, f: [125, 1000, 4000, 10000][b], q: 0.7, t: 0 }); }

        var send = {};
        function setter(addr, y) {
            var k = addr + '|' + y;
            if (!send[k]) { send[k] = U.throttle(function (v) { OS.setParam(addr, ch, y, v); }, 60); }
            return send[k];
        }

        /* En-tête : EQ ON, FLAT */
        var head = U.el('div', 'section-head');
        head.appendChild(U.el('div', 'section-title', OS.t('Égaliseur', 'Equalizer')));
        var flat = U.el('div', 'btn', 'FLAT');
        U.tap(flat, function () {
            var i;
            for (i = 0; i < 4; i++) { OS.setParam(IN + '/PEQ/Band/Gain', ch, i, 0); }
        });
        var onBtn = U.el('div', 'btn on-btn eq-on', 'EQ ON');
        U.tap(onBtn, function () { OS.setParam(IN + '/PEQ/On', ch, 0, st.on ? 0 : 1); });
        head.appendChild(flat);
        head.appendChild(onBtn);
        box.appendChild(head);

        /* Courbe */
        var cv = document.createElement('canvas');
        cv.className = 'eq-graph';
        box.appendChild(cv);
        var W = 0, H = 0, ratio = window.devicePixelRatio || 1, pending = false;

        function xOf(f) { return Math.log(f / F_MIN) / Math.log(F_MAX / F_MIN) * W; }
        function fOf(x) { return F_MIN * Math.pow(F_MAX / F_MIN, U.clamp(x / W, 0, 1)); }
        function yOf(db) { return H / 2 - db / (G_MAX + 2) * (H / 2); }
        function gOf(y) { return U.clamp((H / 2 - y) / (H / 2) * (G_MAX + 2), -G_MAX, G_MAX); }

        function filters() {
            var list = [], i, bd;
            for (i = 0; i < 4; i++) {
                bd = st.bands[i];
                list.push(coeffs(bandType(i, bd.t), bd.f, bd.g, bd.q));
            }
            if (st.hpfOn) { list.push(coeffs('hpf', st.hpfF, 0, 0.7071)); }
            return list;
        }

        function drawGraph() {
            pending = false;
            W = cv.clientWidth;
            H = cv.clientHeight;
            if (W < 10 || H < 10) { return; }
            if (cv.width !== Math.round(W * ratio)) { cv.width = Math.round(W * ratio); cv.height = Math.round(H * ratio); }
            var g = cv.getContext('2d');
            g.setTransform(ratio, 0, 0, ratio, 0, 0);
            g.fillStyle = '#141416';
            g.fillRect(0, 0, W, H);
            g.lineWidth = 1;
            g.font = '10px sans-serif';
            g.fillStyle = '#858590';
            var d, f, i;
            for (d = -12; d <= 12; d += 6) {
                g.strokeStyle = d === 0 ? '#45454f' : '#2a2a30';
                g.beginPath();
                g.moveTo(0, Math.round(yOf(d)) + 0.5);
                g.lineTo(W, Math.round(yOf(d)) + 0.5);
                g.stroke();
                if (d !== 0) { g.fillText((d > 0 ? '+' : '') + d, 3, yOf(d) - 2); }
            }
            var marks = [50, 100, 200, 500, 1000, 2000, 5000, 10000];
            for (i = 0; i < marks.length; i++) {
                f = marks[i];
                g.strokeStyle = '#2a2a30';
                g.beginPath();
                g.moveTo(Math.round(xOf(f)) + 0.5, 0);
                g.lineTo(Math.round(xOf(f)) + 0.5, H);
                g.stroke();
                if (f === 100 || f === 1000 || f === 10000) { g.fillText(f >= 1000 ? (f / 1000) + 'k' : String(f), xOf(f) + 3, H - 3); }
            }
            /* Courbe totale */
            var fl = filters(), x, sum, j;
            g.beginPath();
            for (x = 0; x <= W; x += 2) {
                f = fOf(x);
                sum = 0;
                for (j = 0; j < fl.length; j++) { sum += magDb(fl[j], f); }
                if (!st.on) { sum = st.hpfOn ? magDb(fl[fl.length - 1], f) : 0; }
                if (x === 0) { g.moveTo(x, yOf(sum)); } else { g.lineTo(x, yOf(sum)); }
            }
            g.strokeStyle = st.on ? '#f0b030' : '#858590';
            g.lineWidth = 2;
            g.stroke();
            /* Points des bandes */
            for (i = 0; i < 4; i++) {
                var bd = st.bands[i], py = yOf(bandType(i, bd.t) === 'lpf' ? 0 : bd.g);
                g.fillStyle = COLORS[i];
                g.globalAlpha = st.on ? 1 : 0.4;
                g.beginPath();
                g.arc(xOf(bd.f), py, 9, 0, Math.PI * 2, false);
                g.fill();
                g.globalAlpha = 1;
                g.fillStyle = '#101010';
                g.font = 'bold 11px sans-serif';
                g.fillText(String(i + 1), xOf(bd.f) - 3, py + 4);
            }
            if (st.hpfOn) {
                g.fillStyle = HPF_COLOR;
                g.beginPath();
                g.arc(xOf(st.hpfF), yOf(0), 7, 0, Math.PI * 2, false);
                g.fill();
            }
        }

        function redraw() {
            if (pending) { return; }
            pending = true;
            U.frame(drawGraph);
        }

        /* Glisser un point sur la courbe */
        var active = -1;
        U.drag(cv, {
            start: function (p) {
                var r = cv.getBoundingClientRect(), x = p.x - r.left, y = p.y - r.top, best = 40, i, dx, dy, dd;
                active = -1;
                for (i = 0; i < 4; i++) {
                    dx = xOf(st.bands[i].f) - x;
                    dy = yOf(st.bands[i].g) - y;
                    dd = Math.sqrt(dx * dx + dy * dy);
                    if (dd < best) { best = dd; active = i; }
                }
                if (st.hpfOn) {
                    dx = xOf(st.hpfF) - x;
                    dy = yOf(0) - y;
                    if (Math.sqrt(dx * dx + dy * dy) < best) { active = 4; }
                }
            },
            move: function (p) {
                if (active < 0) { return; }
                var r = cv.getBoundingClientRect(), x = p.x - r.left, y = p.y - r.top;
                if (active === 4) {
                    st.hpfF = U.clamp(fOf(x), HPF_MIN, HPF_MAX);
                    setter(IN + '/HPF/Freq', 0)(Math.round(st.hpfF * fs));
                    hpfEnc.set(U.logToNorm(st.hpfF, HPF_MIN, HPF_MAX), fmtHz(st.hpfF));
                } else {
                    var bd = st.bands[active];
                    bd.f = fOf(x);
                    setter(IN + '/PEQ/Band/Freq', active)(Math.round(bd.f * fs));
                    if (bandType(active, bd.t) !== 'lpf') {
                        bd.g = Math.round(gOf(y) * 10) / 10;
                        setter(IN + '/PEQ/Band/Gain', active)(Math.round(bd.g * 100));
                    }
                    cols[active].sync();
                }
                redraw();
            },
            end: function () { active = -1; }
        });

        function fmtHz(hz) { return hz >= 1000 ? (hz / 1000).toFixed(hz >= 10000 ? 1 : 2) + 'k' : String(Math.round(hz)); }
        function fmtDb(db) { return (db > 0 ? '+' : '') + db.toFixed(1); }

        /* Colonnes : HPF puis les 4 bandes */
        var row = U.el('div', 'eq-cols');
        box.appendChild(row);

        var hpfCol = U.el('div', 'eq-col');
        hpfCol.appendChild(U.el('div', 'eq-col-title', 'HPF'));
        var hpfBtn = U.el('div', 'btn on-btn', 'ON');
        U.tap(hpfBtn, function () { OS.setParam(IN + '/HPF/On', ch, 0, st.hpfOn ? 0 : 1); });
        hpfCol.appendChild(hpfBtn);
        var hpfEnc = encoder('FREQ', HPF_COLOR, function (n) {
            st.hpfF = U.normToLog(n, HPF_MIN, HPF_MAX);
            setter(IN + '/HPF/Freq', 0)(Math.round(st.hpfF * fs));
            redraw();
            return st.hpfF;
        }, fmtHz);
        hpfCol.appendChild(hpfEnc.el);
        row.appendChild(hpfCol);

        var cols = [];
        function bandCol(i) {
            var col = U.el('div', 'eq-col');
            var title = U.el('div', 'eq-col-title', NAMES[i]);
            title.style.color = COLORS[i];
            col.appendChild(title);
            var types = i === 0 ? [['PEAK', 0], ['SHELF', 1]] : (i === 3 ? [['PEAK', 0], ['SHELF', 1], ['LPF', 2]] : []);
            var typeBtns = [];
            if (types.length) {
                var seg = U.el('div', 'eq-types');
                U.each(types, function (t) {
                    var tb = U.el('div', 'btn eq-type', t[0]);
                    U.tap(tb, function () { OS.setParam(IN + '/PEQ/Band/Type', ch, i, t[1]); });
                    seg.appendChild(tb);
                    typeBtns.push({ el: tb, v: t[1] });
                });
                col.appendChild(seg);
            }
            var bd = st.bands[i];
            var gEnc = encoder('GAIN', COLORS[i], function (n) {
                bd.g = Math.round((n * 2 * G_MAX - G_MAX) * 10) / 10;
                setter(IN + '/PEQ/Band/Gain', i)(Math.round(bd.g * 100));
                redraw();
                return bd.g;
            }, fmtDb);
            var fEnc = encoder('FREQ', COLORS[i], function (n) {
                bd.f = U.normToLog(n, F_MIN, F_MAX);
                setter(IN + '/PEQ/Band/Freq', i)(Math.round(bd.f * fs));
                redraw();
                return bd.f;
            }, fmtHz);
            var qEnc = encoder('Q', COLORS[i], function (n) {
                bd.q = U.normToLog(n, Q_MIN, Q_MAX);
                setter(IN + '/PEQ/Band/Q', i)(Math.round(bd.q * qs));
                redraw();
                return bd.q;
            }, function (q) { return q.toFixed(2); });
            col.appendChild(gEnc.el);
            col.appendChild(fEnc.el);
            col.appendChild(qEnc.el);
            row.appendChild(col);
            return {
                sync: function () {
                    gEnc.set((bd.g + G_MAX) / (2 * G_MAX), fmtDb(bd.g));
                    fEnc.set(U.logToNorm(bd.f, F_MIN, F_MAX), fmtHz(bd.f));
                    qEnc.set(U.logToNorm(bd.q, Q_MIN, Q_MAX), bd.q.toFixed(2));
                    U.each(typeBtns, function (t) { U.setClass(t.el, 'lit', t.v === bd.t); });
                    var cut = bandType(i, bd.t) === 'lpf';
                    U.setClass(gEnc.el, 'off', cut);
                    U.setClass(qEnc.el, 'off', bandType(i, bd.t) !== 'peak');
                }
            };
        }
        for (b = 0; b < 4; b++) { cols.push(bandCol(b)); }

        /* Valeurs de la console */
        OS.watch(IN + '/PEQ/On', ch, 0, function (v) {
            st.on = v ? 1 : 0;
            U.setClass(onBtn, 'lit', !!st.on);
            redraw();
        }, group);
        OS.watch(IN + '/HPF/On', ch, 0, function (v) {
            st.hpfOn = v ? 1 : 0;
            U.setClass(hpfBtn, 'lit', !!st.hpfOn);
            redraw();
        }, group);
        OS.watch(IN + '/HPF/Freq', ch, 0, function (v) {
            st.hpfF = U.clamp(v / fs, HPF_MIN, HPF_MAX);
            hpfEnc.set(U.logToNorm(st.hpfF, HPF_MIN, HPF_MAX), fmtHz(st.hpfF));
            redraw();
        }, group);
        U.each([0, 1, 2, 3], function (i) {
            var bd = st.bands[i];
            OS.watch(IN + '/PEQ/Band/Gain', ch, i, function (v) { bd.g = U.clamp(v / 100, -G_MAX, G_MAX); cols[i].sync(); redraw(); }, group);
            OS.watch(IN + '/PEQ/Band/Freq', ch, i, function (v) { bd.f = U.clamp(v / fs, F_MIN, F_MAX); cols[i].sync(); redraw(); }, group);
            OS.watch(IN + '/PEQ/Band/Q', ch, i, function (v) { bd.q = U.clamp(v / qs, Q_MIN, Q_MAX); cols[i].sync(); redraw(); }, group);
            OS.watch(IN + '/PEQ/Band/Type', ch, i, function (v) { bd.t = v | 0; cols[i].sync(); redraw(); }, group);
            cols[i].sync();
        });
        hpfEnc.set(U.logToNorm(st.hpfF, HPF_MIN, HPF_MAX), fmtHz(st.hpfF));
        redraw();
        return { redraw: redraw };
    };
}());
