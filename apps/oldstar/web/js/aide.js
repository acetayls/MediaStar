/* OldStar : onglet Aide. Le manuel d'OldStar (le même que la fenêtre d'aide du PC), demandé au
 * serveur à la première ouverture de l'onglet, avec la version d'OldStar en tête.
 */
(function () {
    'use strict';

    var U = OS.u;
    var pane = null;
    var loaded = false;

    function load() {
        U.empty(pane);
        pane.appendChild(U.el('div', 'empty', OS.t('Chargement de l’aide…', 'Loading help…')));
        OS.transport.get(OS.lang === 'en' ? '/aide?en=1' : '/aide', function (r) {
            if (!r || typeof r.html !== 'string') {
                U.empty(pane);
                var box = U.el('div', 'empty', OS.t('Aide indisponible : le PC ne répond pas.', 'Help unavailable: the PC does not answer.'));
                var again = U.el('div', 'btn help-retry', OS.t('Réessayer', 'Retry'));
                U.tap(again, load);
                pane.appendChild(box);
                pane.appendChild(again);
                return;
            }
            loaded = true;
            var doc = U.el('div', 'help');
            doc.innerHTML = r.html;
            U.empty(pane);
            pane.appendChild(doc);
        });
    }

    OS.tabs = OS.tabs || [];
    OS.tabs.push({
        id: 'aide',
        label: OS.t('Aide', 'Help'),
        init: function (p) { pane = p; },
        show: function (on) {
            if (on && !loaded) { load(); }
        }
    });
}());
