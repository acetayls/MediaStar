#!/usr/bin/env node
/* Vérifie que la page servie aux tablettes (apps/oldstar/web) reste dans le plancher de
 * compatibilité : iOS 9 / Safari 9 et Android 4.4 / Chrome 30, sans rien qui bloquerait un
 * passage plus tard à iOS 5 / Android 2.3.
 *
 *   node apps/oldstar/tools/verif-page.js [--acorn <chemin du paquet acorn>]
 *
 * JavaScript :
 *  - grammaire ES5 stricte par acorn (ecmaVersion 5) si acorn est trouvé (--acorn, variable
 *    ACORN, ou `require('acorn')`) ; sinon repli sur un balayage des jetons ;
 *  - dans tous les cas, refus des API trop récentes que la grammaire laisse passer (fetch,
 *    Promise, bind, classList…) et exigence de 'use strict'.
 * CSS : refus de var(--…), backdrop-filter, position:sticky, calc(…) ; flex, transform et
 * transition doivent avoir leur écriture -webkit-.
 * Code de sortie 0 si tout va bien, 1 sinon.
 */
'use strict';

var fs = require('fs');
var path = require('path');

var WEB = path.join(__dirname, '..', 'web');

function findAcorn() {
    var i = process.argv.indexOf('--acorn');
    var candidates = [];
    if (i >= 0 && process.argv[i + 1]) { candidates.push(path.resolve(process.argv[i + 1])); }
    if (process.env.ACORN) { candidates.push(path.resolve(process.env.ACORN)); }
    candidates.push('acorn');
    for (var k = 0; k < candidates.length; k++) {
        try { return require(candidates[k]); } catch (e) { /* suivant */ }
    }
    return null;
}

function walk(dir, out) {
    fs.readdirSync(dir).forEach(function (n) {
        var p = path.join(dir, n);
        if (fs.statSync(p).isDirectory()) { walk(p, out); } else { out.push(p); }
    });
    return out;
}

/* Découpe du JS en jetons, sans les commentaires ni le contenu des chaînes et des regex. */
function tokens(src) {
    var out = [], i = 0, n = src.length, last = null;
    function push(type, value, pos) { last = { type: type, value: value, pos: pos }; out.push(last); }
    function regexAllowed() {
        if (!last) { return true; }
        if (last.type === 'punct') { return [')', ']', '}'].indexOf(last.value) < 0; }
        if (last.type === 'name') { return ['return', 'typeof', 'case', 'do', 'else', 'in', 'instanceof', 'new', 'delete', 'void', 'throw'].indexOf(last.value) >= 0; }
        return false;
    }
    while (i < n) {
        var c = src[i];
        if (/\s/.test(c)) { i++; continue; }
        if (c === '/' && src[i + 1] === '/') { while (i < n && src[i] !== '\n') { i++; } continue; }
        if (c === '/' && src[i + 1] === '*') { var e = src.indexOf('*/', i + 2); i = e < 0 ? n : e + 2; continue; }
        if (c === '"' || c === "'") {
            var s = i; i++;
            while (i < n && src[i] !== c) { if (src[i] === '\\') { i++; } i++; }
            i++; push('string', '', s); continue;
        }
        if (c === '`') { push('punct', '`', i); i++; continue; }
        if (c === '/' && regexAllowed()) {
            var r = i, cls = false; i++;
            while (i < n) {
                if (src[i] === '\\') { i += 2; continue; }
                if (src[i] === '[') { cls = true; } else if (src[i] === ']') { cls = false; } else if (src[i] === '/' && !cls) { break; }
                i++;
            }
            i++; while (i < n && /[a-z]/i.test(src[i])) { i++; }
            push('regex', '', r); continue;
        }
        var m = /^[A-Za-z_$][\w$]*/.exec(src.slice(i, i + 64));
        if (m) { push('name', m[0], i); i += m[0].length; continue; }
        m = /^(\d+\.?\d*(e[+-]?\d+)?|0x[0-9a-f]+)/i.exec(src.slice(i, i + 64));
        if (m) { push('num', m[0], i); i += m[0].length; continue; }
        m = /^(=>|\.\.\.|\*\*=?|\?\.|\?\?|===|!==|==|!=|<=|>=|&&|\|\||\+\+|--|[-+*\/%&|^]=|<<=|>>>=|>>=|<<|>>>|>>|.)/.exec(src.slice(i, i + 4));
        push('punct', m[0], i); i += m[0].length;
    }
    return out;
}

function lineOf(src, pos) { return src.slice(0, pos).split('\n').length; }

/* Mots et API refusés partout (la grammaire ES5 d'acorn ne voit pas les API). */
var BAD_NAMES = {
    'let': 'let', 'const': 'const', 'class': 'class', 'import': 'module', 'export': 'module',
    'async': 'async', 'await': 'await', 'yield': 'yield',
    'fetch': 'fetch (XHR seulement)', 'Promise': 'Promise', 'Symbol': 'Symbol', 'Map': 'Map', 'Set': 'Set',
    'WeakMap': 'WeakMap', 'Proxy': 'Proxy', 'Reflect': 'Reflect'
};
var BAD_PROPS = {
    'bind': 'Function.prototype.bind (absent d’iOS 5)', 'classList': 'classList (absent d’Android 2.3)',
    'includes': 'includes', 'startsWith': 'startsWith', 'endsWith': 'endsWith', 'repeat': 'repeat',
    'padStart': 'padStart', 'padEnd': 'padEnd', 'assign': 'Object.assign', 'entries': 'entries',
    'findIndex': 'findIndex', 'fill': 'Array.prototype.fill', 'closest': 'Element.closest',
    'dataset': 'dataset', 'addEventListenerOnce': '', 'append': 'append (appendChild)', 'remove': 'remove()',
    'forEach': 'forEach sur une NodeList (U.each)'
};
var BAD_PUNCT = { '=>': 'fonction fléchée', '`': 'chaîne `…`', '...': 'décomposition ...', '**': '**', '**=': '**=', '?.': '?.', '??': '??' };

function checkJs(file, src, acorn, problems) {
    function bad(pos, what) { problems.push(file + ':' + lineOf(src, pos) + ' : ' + what); }
    if (acorn) {
        try {
            acorn.parse(src, { ecmaVersion: 5, sourceType: 'script', allowReserved: false });
        } catch (e) {
            problems.push(file + ':' + (e.loc ? e.loc.line : '?') + ' : pas de l’ES5 (' + e.message + ')');
        }
    }
    var t = tokens(src), k;
    for (k = 0; k < t.length; k++) {
        var tok = t[k], prev = t[k - 1];
        if (tok.type === 'punct' && BAD_PUNCT[tok.value]) { bad(tok.pos, BAD_PUNCT[tok.value]); }
        if (tok.type === 'name') {
            var isProp = prev && prev.type === 'punct' && prev.value === '.';
            if (isProp && BAD_PROPS.hasOwnProperty(tok.value) && BAD_PROPS[tok.value]) {
                /* forEach est permis sur les tableaux : on ne le refuse que sur querySelectorAll / childNodes. */
                /* fill() sans argument : celui du canvas 2D, permis (Array.prototype.fill prend une valeur). */
                if (tok.value === 'fill' && t[k + 1] && t[k + 1].value === '(' && t[k + 2] && t[k + 2].value === ')') { continue; }
                if (tok.value === 'forEach') {
                    var obj = t[k - 2];
                    if (!obj || ['childNodes', 'children', 'querySelectorAll'].indexOf(obj.value) < 0) { continue; }
                }
                bad(tok.pos, BAD_PROPS[tok.value]);
            }
            if (!isProp && BAD_NAMES.hasOwnProperty(tok.value)) {
                var next = t[k + 1];
                /* « clé: » d'un objet littéral : permis. */
                if (next && next.type === 'punct' && next.value === ':' && prev && (prev.value === '{' || prev.value === ',')) { continue; }
                bad(tok.pos, BAD_NAMES[tok.value]);
            }
        }
        /* Virgule finale dans un appel ou une liste de paramètres (ES2017). */
        if (tok.type === 'punct' && tok.value === ')' && prev && prev.value === ',') { bad(tok.pos, 'virgule finale avant « ) »'); }
    }
    if (src.indexOf("'use strict'") < 0) { problems.push(file + ' : pas de \'use strict\''); }
}

function checkCss(file, src, problems) {
    var clean = src.replace(/\/\*[\s\S]*?\*\//g, function (c) { return c.replace(/[^\n]/g, ' '); });
    function bad(re, what) {
        var m, g = new RegExp(re.source, 'g' + (re.ignoreCase ? 'i' : ''));
        while ((m = g.exec(clean))) { problems.push(file + ':' + lineOf(clean, m.index) + ' : ' + what); }
    }
    bad(/var\(\s*--/, 'variable CSS');
    bad(/--[\w-]+\s*:/, 'variable CSS');
    bad(/backdrop-filter/, 'backdrop-filter');
    bad(/position\s*:\s*sticky/, 'position: sticky');
    bad(/calc\(/, 'calc()');
    bad(/display\s*:\s*grid/, 'grille CSS');
    /* Chaque déclaration sans préfixe doit avoir sa jumelle -webkit- juste avant, dans le même bloc. */
    var blocks = clean.split('}');
    var offset = 0;
    blocks.forEach(function (b) {
        var pairs = [
            [/(^|[;{\s])display\s*:\s*flex\b/, /display\s*:\s*-webkit-flex\b/, 'display: flex sans display: -webkit-flex'],
            [/(^|[;{\s])flex\s*:/, /-webkit-flex\s*:/, 'flex sans -webkit-flex'],
            [/(^|[;{\s])flex-wrap\s*:/, /-webkit-flex-wrap\s*:/, 'flex-wrap sans -webkit-flex-wrap'],
            [/(^|[;{\s])flex-direction\s*:/, /-webkit-flex-direction\s*:/, 'flex-direction sans -webkit-'],
            [/(^|[;{\s])align-items\s*:/, /-webkit-align-items\s*:/, 'align-items sans -webkit-align-items'],
            [/(^|[;{\s])justify-content\s*:/, /-webkit-justify-content\s*:/, 'justify-content sans -webkit-'],
            [/(^|[;{\s])transform\s*:/, /-webkit-transform\s*:/, 'transform sans -webkit-transform'],
            [/(^|[;{\s])transition\s*:/, /-webkit-transition\s*:/, 'transition sans -webkit-transition'],
            [/(^|[;{\s])box-shadow\s*:/, /-webkit-box-shadow\s*:/, 'box-shadow sans -webkit-box-shadow']
        ];
        pairs.forEach(function (p) {
            var m = p[0].exec(b);
            if (m && !p[1].test(b)) { problems.push(file + ':' + lineOf(clean, offset + m.index + 1) + ' : ' + p[2]); }
        });
        offset += b.length + 1;
    });
}

function checkHtml(file, src, problems) {
    if (!/name="viewport"/.test(src)) { problems.push(file + ' : meta viewport manquante'); }
    if (!/apple-mobile-web-app-capable/.test(src)) { problems.push(file + ' : meta apple-mobile-web-app-capable manquante'); }
    if (/<script[^>]*type="module"/.test(src)) { problems.push(file + ' : script de type module'); }
    var inline = src.replace(/<script[^>]*src=[^>]*><\/script>/g, '');
    if (/<script/.test(inline)) { problems.push(file + ' : script en ligne (tout le JS doit être dans web/js, vérifié ici)'); }
}

var acorn = findAcorn();
var problems = [];
var files = walk(WEB, []);
var counts = { js: 0, css: 0, html: 0 };
files.forEach(function (f) {
    var src = fs.readFileSync(f, 'utf8');
    var rel = path.relative(path.join(__dirname, '..'), f).replace(/\\/g, '/');
    if (/\.js$/.test(f)) { counts.js++; checkJs(rel, src, acorn, problems); }
    else if (/\.css$/.test(f)) { counts.css++; checkCss(rel, src, problems); }
    else if (/\.html?$/.test(f)) { counts.html++; checkHtml(rel, src, problems); }
});

console.log('Page OldStar : ' + counts.js + ' JS, ' + counts.css + ' CSS, ' + counts.html + ' HTML ; ' +
    (acorn ? 'grammaire ES5 vérifiée par acorn ' + (acorn.version || '') : 'acorn introuvable : balayage des jetons seulement'));
if (problems.length) {
    problems.forEach(function (p) { console.log('  ✗ ' + p); });
    console.log(problems.length + ' problème(s).');
    process.exit(1);
}
console.log('  ✓ conforme au plancher (iOS 9 / Android 4.4, ES5 strict).');
