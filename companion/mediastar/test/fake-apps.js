// OSCQuery trees shaped like those of the apps (apps/jinglestar/src/osc.rs `tree`,
// apps/cuestar/src/osc.rs `tree`), and a small HTTP server that serves them.

import http from 'node:http'

function insert(root, path, node) {
	let cur = root
	let full = ''
	const segs = path.split('/').filter(Boolean)
	segs.forEach((seg, i) => {
		full += `/${seg}`
		cur.CONTENTS ??= {}
		if (i === segs.length - 1) {
			cur.CONTENTS[seg] = { ...node, FULL_PATH: full }
		} else {
			cur.CONTENTS[seg] ??= { FULL_PATH: full, ACCESS: 0 }
			cur = cur.CONTENTS[seg]
		}
	})
}
const leaf = (TYPE, ACCESS, value) => (value === undefined ? { TYPE, ACCESS } : { TYPE, ACCESS, VALUE: [value] })
const trig = () => leaf('N', 2)
const b = (v) => (v ? 'T' : 'F')

/** modules: [{ name, mode, volume, jingles: [{name, color, playing, remaining, volume}|null], playlist, cues }] */
export function jingleTree(prefix, modules) {
	const root = { FULL_PATH: '/', ACCESS: 0, CONTENTS: {} }
	const add = (p, n) => insert(root, `${prefix}${p}`, n)
	add('/stop_all', trig())
	add('/go', trig())
	add('/subscribe', leaf('i', 2))
	modules.forEach((m, mi) => {
		const base = `/module/${mi + 1}`
		add(`${base}/name`, leaf('s', 1, m.name))
		add(`${base}/mode`, leaf('s', 3, m.mode))
		add(`${base}/volume`, leaf('f', 3, m.volume ?? 0))
		add(`${base}/stop`, trig())
		if (m.mode === 'go') {
			const c = m.cues
			add(`${base}/go`, trig())
			add(`${base}/cue/next`, trig())
			add(`${base}/cue/previous`, trig())
			add(`${base}/cue/standby`, leaf('i', 3, c.standby < c.names.length ? c.standby + 1 : 0))
			add(`${base}/cue/name`, leaf('s', 1, c.names[c.standby] ?? ''))
			add(`${base}/cue/count`, leaf('i', 1, c.names.length))
			add(`${base}/cue/playing`, leaf('i', 1, c.playing))
			add(`${base}/cue/playing_name`, leaf('s', 1, c.playing_name))
			add(`${base}/cue/remaining`, leaf('f', 1, c.remaining ?? -1))
			add(`${base}/cue/progress`, leaf('f', 1, -1))
			add(`${base}/cue/pause`, trig())
			add(`${base}/cue/paused`, leaf(b(c.paused), 3, c.paused))
		} else if (m.mode === 'jingle') {
			for (let s = 0; s < 12; s++) {
				const j = m.jingles[s] ?? null
				const jb = `${base}/jingle/${s + 1}`
				add(`${jb}/name`, leaf('s', 1, j?.name ?? ''))
				add(`${jb}/play`, trig())
				add(`${jb}/stop`, trig())
				add(`${jb}/toggle`, trig())
				add(`${jb}/volume`, leaf('f', 3, j?.volume ?? 0))
				add(`${jb}/playing`, leaf(b(j?.playing), 1, !!j?.playing))
				add(`${jb}/color`, leaf('s', 1, j?.color ?? ''))
				add(`${jb}/remaining`, leaf('f', 1, j?.remaining ?? -1))
				add(`${jb}/progress`, leaf('f', 1, -1))
			}
		} else if (m.mode === 'playlist') {
			const pl = m.playlist
			const pb = `${base}/playlist`
			for (const t of ['play', 'stop', 'pause', 'next', 'previous']) add(`${pb}/${t}`, trig())
			add(`${pb}/paused`, leaf(b(pl.paused), 3, pl.paused))
			add(`${pb}/track`, leaf('i', 3, pl.current ?? 0))
			add(`${pb}/tracks`, leaf('i', 1, pl.tracks))
			add(`${pb}/title`, leaf('s', 1, pl.title))
			add(`${pb}/remaining`, leaf('f', 1, pl.remaining ?? -1))
			add(`${pb}/progress`, leaf('f', 1, -1))
			add(`${pb}/playing`, leaf(b(pl.playing), 1, pl.playing))
			add(`${pb}/shuffle`, leaf(b(pl.shuffle), 3, pl.shuffle))
			add(`${pb}/loop`, leaf(b(pl.loop), 3, pl.loop))
		}
	})
	return root
}

export function cueTree(prefix, s) {
	const root = { FULL_PATH: '/', ACCESS: 0, CONTENTS: {} }
	const add = (p, n) => insert(root, `${prefix}/${p}`, n)
	add('noir', trig())
	add('output/drops/reset', trig())
	add('subscribe', leaf('i', 2))
	add('media', leaf('s', 1, s.media))
	add('paused', leaf(b(s.paused), 1, s.paused))
	add('time', leaf('f', 1, s.time))
	add('remaining', leaf('f', 1, s.remaining))
	for (const t of ['go', 'pause', 'play', 'toggle', 'stop', 'next', 'prev']) {
		add(`lecteur/${t}`, trig())
		add(`cuelist/${t}`, trig())
	}
	add('lecteur/goto', leaf('i', 2))
	add('lecteur/shuffle', leaf(b(false), 3, false))
	add('lecteur/loop', leaf(b(s.loop), 3, s.loop))
	add('lecteur/transition', leaf('sf', 2))
	add('cuelist/standby', leaf('i', 3, s.standby))
	add('cuelist/standby_name', leaf('s', 1, s.standby_name))
	add('cuelist/running_name', leaf('s', 1, s.running_name))
	add('cuelist/cues', leaf('i', 1, s.cues))
	add('cuelist/goto', leaf('i', 2))
	add('cuelist/loop', leaf(b(s.loop), 3, s.loop))
	add('cuelist/transition', leaf('sf', 2))
	for (const t of ['play', 'pause', 'toggle', 'stop', 'next', 'prev']) add(`diaporama/${t}`, trig())
	add('diaporama/goto', leaf('i', 2))
	add('diaporama/index', leaf('i', 1, s.slide[0]))
	add('diaporama/count', leaf('i', 1, s.slide[1]))
	for (const k of ['shuffle', 'loop', 'recursive', 'video_sound']) add(`diaporama/settings/${k}`, leaf('T', 2))
	add('diaporama/settings/photo_duration', leaf('f', 2))
	add('diaporama/settings/fit', leaf('s', 2))
	add('diaporama/settings/folder', leaf('s', 2))
	return root
}

/** Serves `getTree()` at "/" on a free port. */
export function serve(getTree) {
	return new Promise((resolve) => {
		const server = http.createServer((req, res) => {
			res.setHeader('Content-Type', 'application/json')
			res.end(JSON.stringify(getTree()))
		})
		server.listen(0, '127.0.0.1', () => resolve(server))
	})
}

/** Every writable path of a tree (without prefix), with module numbers replaced by "N". */
export function writablePaths(tree, prefix) {
	const out = new Set()
	const walk = (n) => {
		if (n.TYPE && n.ACCESS & 2) out.add(n.FULL_PATH.slice(prefix.length))
		for (const c of Object.values(n.CONTENTS ?? {})) walk(c)
	}
	walk(tree)
	return out
}
