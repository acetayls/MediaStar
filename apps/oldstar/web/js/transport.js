/* OldStar : transport entre la page et le serveur. Seul fichier qui parle au réseau en temps
 * réel ; le reste de la page ne connaît que cette interface :
 *
 *   OS.transport.connect({ open: fn(mode), message: fn(obj), close: fn(raison) })
 *   OS.transport.send(obj)            commande JSON vers le serveur
 *   OS.transport.mode()               'ws', 'poll' ou ''
 *   OS.transport.wantMeters(oui)      reçoit ou non les relevés des vu-mètres
 *
 * Messages reçus (voir docs/protocole.md) : { t: 'snap' | 'ev' | 'mtr', ... }. Le transport
 * retient seulement les numéros (seq des événements, ms des vu-mètres) pour reprendre là où il
 * en était après une coupure.
 *
 * Deux façons de parler au serveur :
 *  - WebSocket (RFC 6455) sur /ws : la voie normale (iOS 6+, Android 4.4+) ;
 *  - long-polling XHR (GET /poll, POST /cmd) : repli quand WebSocket manque ou échoue
 *    plusieurs fois de suite (vieux navigateurs, proxy qui coupe l'Upgrade).
 */
(function () {
    'use strict';

    var U = OS.u;
    var T = {};
    OS.transport = T;

    var handlers = null;
    var current = '';      /* 'ws', 'poll' ou '' */
    var ws = null;
    var seq = 0;           /* dernier événement reçu */
    var mseq = 0;          /* dernier relevé des vu-mètres reçu */
    var meters = false;
    var wsFailures = 0;
    var wsOpened = false;  /* WebSocket a déjà marché une fois : on n'abandonne pas pour si peu */
    var retryTimer = null;
    var pollXhr = null;
    var pollGen = 0;
    var clientId = 'c' + U.now().toString(36) + Math.floor(Math.random() * 1e6).toString(36);

    /* Après 3 échecs de WebSocket sans aucune réussite, on passe au long-polling. */
    var WS_MAX_FAILURES = 3;

    function note(obj) {
        if (obj && typeof obj.seq === 'number') { seq = obj.seq; }
        if (obj && typeof obj.ms === 'number') { mseq = obj.ms; }
    }

    function deliver(text) {
        var obj;
        try { obj = JSON.parse(text); } catch (e) { return; }
        note(obj);
        if (handlers && handlers.message) { handlers.message(obj); }
    }

    function setMode(m, reason) {
        var was = current;
        current = m;
        if (m && handlers && handlers.open) { handlers.open(m); }
        if (!m && was && handlers && handlers.close) { handlers.close(reason || ''); }
    }

    function later(fn, ms) {
        if (retryTimer !== null) { window.clearTimeout(retryTimer); }
        retryTimer = window.setTimeout(function () { retryTimer = null; fn(); }, ms);
    }

    /* ---------- WebSocket ---------- */

    function wsUrl() {
        var proto = window.location.protocol === 'https:' ? 'wss://' : 'ws://';
        return proto + window.location.host + '/ws';
    }

    function startWs() {
        var sock;
        try {
            sock = new window.WebSocket(wsUrl());
        } catch (e) {
            wsFailed('WebSocket refusé');
            return;
        }
        ws = sock;
        var opened = false;
        sock.onopen = function () {
            if (ws !== sock) { return; }
            opened = true;
            wsOpened = true;
            wsFailures = 0;
            sock.send(JSON.stringify({ c: 'hello', seq: seq, meters: meters, id: clientId }));
            setMode('ws');
        };
        sock.onmessage = function (ev) {
            if (ws !== sock) { return; }
            if (typeof ev.data === 'string') { deliver(ev.data); }
        };
        sock.onclose = function () {
            if (ws !== sock) { return; }
            ws = null;
            if (opened) {
                setMode('', 'connexion coupée');
                later(startWs, 1000);
            } else {
                wsFailed('connexion impossible');
            }
        };
        sock.onerror = function () { /* onclose suit toujours */ };
    }

    function wsFailed(reason) {
        wsFailures++;
        setMode('', reason);
        if (!wsOpened && wsFailures >= WS_MAX_FAILURES) {
            startPoll();
        } else {
            later(startWs, Math.min(1000 * wsFailures, 5000));
        }
    }

    /* ---------- Long-polling XHR ---------- */

    function xhr(method, url, body, done) {
        var x = new XMLHttpRequest();
        x.open(method, url, true);
        if (body !== null) { x.setRequestHeader('Content-Type', 'application/json'); }
        x.onreadystatechange = function () {
            if (x.readyState !== 4) { return; }
            done(x.status, x.responseText);
        };
        x.send(body);
        return x;
    }

    function startPoll() {
        pollGen++;
        var gen = pollGen;
        function loop() {
            if (gen !== pollGen) { return; }
            var url = '/poll?since=' + seq + '&ms=' + mseq + '&meters=' + (meters ? 1 : 0) +
                '&id=' + clientId + '&_=' + U.now();
            pollXhr = xhr('GET', url, null, function (status, text) {
                pollXhr = null;
                if (gen !== pollGen) { return; }
                if (status !== 200) {
                    if (current) { setMode('', 'serveur injoignable'); }
                    later(loop, 2000);
                    return;
                }
                if (!current) { setMode('poll'); }
                var obj;
                try { obj = JSON.parse(text); } catch (e) { obj = null; }
                if (obj && obj.msgs) {
                    U.each(obj.msgs, function (m) {
                        note(m);
                        if (handlers && handlers.message) { handlers.message(m); }
                    });
                }
                /* Le serveur garde la requête jusqu'à 20 s ; on repart tout de suite. */
                later(loop, meters ? 30 : 0);
            });
        }
        loop();
    }

    /* ---------- Interface ---------- */

    T.connect = function (h) {
        handlers = h;
        if (window.WebSocket) {
            startWs();
        } else {
            startPoll();
        }
    };

    T.send = function (obj) {
        var text = JSON.stringify(obj);
        if (current === 'ws' && ws && ws.readyState === 1) {
            ws.send(text);
            return true;
        }
        if (current === 'poll') {
            xhr('POST', '/cmd?id=' + clientId, text, function () {});
            return true;
        }
        return false;
    };

    T.mode = function () { return current; };

    T.wantMeters = function (on) {
        on = !!on;
        if (on === meters) { return; }
        meters = on;
        if (current === 'ws') {
            T.send({ c: 'meters', on: meters });
        } else if (current === 'poll' && pollXhr) {
            /* La requête en cours a été faite avec l'ancien réglage : on la relance. */
            pollXhr.onreadystatechange = null;
            try { pollXhr.abort(); } catch (e) { /* rien */ }
            pollXhr = null;
            startPoll();
        }
    };

    /* Requête ponctuelle (liste des fenêtres…) : done(objet JSON ou null). */
    T.get = function (url, done) {
        xhr('GET', url + (url.indexOf('?') < 0 ? '?' : '&') + '_=' + U.now(), null, function (status, text) {
            var obj = null;
            if (status === 200) {
                try { obj = JSON.parse(text); } catch (e) { obj = null; }
            }
            done(obj);
        });
    };

    /* Après un événement « cfg », ou si la page veut tout relire. */
    T.resync = function () {
        T.send({ c: 'snap' });
    };
}());
