// Runs the module without Companion: the base methods are replaced by recorders, the
// application by a fake OSCQuery server.

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { MediaStarInstance } from '../src/instance.js'
import { flattenTree, formatTime, normalizePrefix, progressBar } from '../src/util.js'
import { cueTree, jingleTree, serve, writablePaths } from './fake-apps.js'

const sleep = (ms) => new Promise((r) => setTimeout(r, ms))

function fakeInstance(config) {
	const inst = Object.create(MediaStarInstance.prototype)
	Object.assign(inst, {
		state: new Map(),
		values: {},
		connected: false,
		timer: null,
		polling: false,
		failures: 0,
		sent: [],
		vars: {},
		status: [],
		checks: 0,
	})
	Object.defineProperty(inst, 'label', { value: 'ms' })
	inst.setActionDefinitions = (a) => (inst.actions = a)
	inst.setFeedbackDefinitions = (f) => (inst.feedbacks = f)
	inst.setVariableDefinitions = (v) => (inst.varDefs = v)
	inst.setPresetDefinitions = (p) => (inst.presets = p)
	inst.setVariableValues = (v) => Object.assign(inst.vars, v)
	inst.checkFeedbacks = () => inst.checks++
	inst.updateStatus = (s, m) => inst.status.push([s, m])
	inst.log = () => {}
	inst.parseVariablesInString = async (s) => s
	inst.oscSend = (host, port, addr, args) => inst.sent.push({ host, port, addr, args })
	inst.config = config
	return inst
}

/** Default option values of an action, or of a feedback. */
function defaults(def) {
	return Object.fromEntries(def.options.filter((o) => 'default' in o).map((o) => [o.id, o.default]))
}

/** Every value combination of the dropdowns (other options at their default). */
function combos(def) {
	let list = [defaults(def)]
	for (const o of def.options.filter((o) => o.type === 'dropdown')) {
		list = list.flatMap((base) => o.choices.map((c) => ({ ...base, [o.id]: c.id })))
	}
	return list
}

const jingleModules = [
	{
		name: 'Jingles',
		mode: 'jingle',
		volume: -3,
		jingles: [{ name: 'Intro', color: '#ff8000', playing: true, remaining: 12.34, volume: -2 }, { name: 'Applause', color: '#2040ff' }],
	},
	{ name: 'Music', mode: 'playlist', playlist: { tracks: 8, current: 3, title: 'track3.mp3', remaining: 125, playing: true, paused: false, shuffle: true, loop: false } },
	{ name: 'Show', mode: 'go', cues: { names: ['Open', 'Scene 2'], standby: 1, playing: 1, playing_name: 'Open', remaining: 4.2, paused: false } },
	{ name: 'Text', mode: 'text' },
]

test('helpers', () => {
	assert.equal(normalizePrefix('jinglestar/'), '/jinglestar')
	assert.equal(normalizePrefix(' / '), '')
	assert.equal(formatTime(-1), '')
	assert.equal(formatTime(12.34), '0:13')
	assert.equal(formatTime(125), '2:05')
	assert.equal(formatTime(3725), '1:02:05')
	const flat = flattenTree(jingleTree('/js', jingleModules), '/js')
	assert.equal(flat.get('/module/1/jingle/1/name'), 'Intro')
	assert.equal(flat.get('/module/2/playlist/shuffle'), true)
	assert.deepEqual(progressBar(-1, 0xffffff, 6), {})
	const bar = progressBar(0.5, 0xff0000, 6, { width: 72, height: 58 })
	assert.deepEqual(bar.imageBufferPosition, { x: 0, y: 52, width: 72, height: 6 })
	const px = Buffer.from(bar.imageBuffer, 'base64')
	assert.equal(px.length, 72 * 6 * 4)
	assert.deepEqual([...px.subarray(0, 4)], [255, 0, 0, 255])
	assert.deepEqual([...px.subarray(71 * 4, 72 * 4)], [0, 0, 0, 160])
})

test('JingleStar: state, variables, feedbacks, actions, presets', async () => {
	let tree = jingleTree('/jinglestar', jingleModules)
	const server = await serve(() => tree)
	const port = server.address().port
	const inst = fakeInstance({ app: 'jinglestar', host: '127.0.0.1', port: 0, oscquery: port, prefix: '', poll: 100 })
	try {
		inst.setup()
		await sleep(300)
		assert.equal(inst.connected, true)
		assert.equal(inst.vars.connected, '1')
		assert.equal(inst.vars.m1_name, 'Jingles')
		assert.equal(inst.vars.m1_j1_name, 'Intro')
		assert.equal(inst.vars.m1_j1_remaining, '0:13')
		assert.equal(inst.vars.m1_j2_remaining, '')
		assert.equal(inst.vars.m1_volume, '-3')
		assert.equal(inst.vars.m2_pl_title, 'track3.mp3')
		assert.equal(inst.vars.m2_pl_track, '3')
		assert.equal(inst.vars.m2_pl_remaining, '2:05')
		assert.equal(inst.vars.m3_cue_name, 'Scene 2')
		assert.equal(inst.vars.m3_cue_standby, '2')
		assert.equal(inst.vars.m3_cue_remaining, '0:05')

		const fb = (id, options) => inst.feedbacks[id].callback({ options: { ...defaults(inst.feedbacks[id]), ...options } })
		assert.equal(fb('jingle_playing', { module: 1, slot: 1 }), true)
		assert.equal(fb('jingle_playing', { module: 1, slot: 2 }), false)
		assert.deepEqual(fb('jingle_colour', { module: 1, slot: 1 }), { bgcolor: 0xff8000, color: 0x000000 })
		assert.equal(fb('jingle_colour', { module: 1, slot: 2 }).bgcolor, (11 << 16) | (22 << 8) | 89)
		assert.deepEqual(fb('jingle_colour', { module: 1, slot: 3 }), {})
		assert.equal(fb('playlist_playing', { module: 2 }), true)
		assert.equal(fb('playlist_shuffle', { module: 2 }), true)
		assert.equal(fb('playlist_loop', { module: 2 }), false)
		assert.equal(fb('cue_playing', { module: 3 }), true)
		assert.equal(fb('module_mode', { module: 4, mode: 'text' }), true)
		assert.equal(fb('connected', {}), true)

		// Every address sent must exist, writable, in a JingleStar tree (module numbers aside).
		const all = new Set()
		for (const mode of ['jingle', 'playlist', 'go']) {
			const mods = [1, 2, 3, 4].map(() => ({ ...jingleModules.find((m) => m.mode === mode) }))
			for (const p of writablePaths(jingleTree('/jinglestar', mods), '/jinglestar')) all.add(p)
		}
		for (const [id, def] of Object.entries(inst.actions)) {
			for (const options of combos(def)) {
				inst.sent = []
				await def.callback({ options })
				assert.equal(inst.sent.length, 1, id)
				const { addr, port: p, host, args } = inst.sent[0]
				assert.equal(p, 8000)
				assert.equal(host, '127.0.0.1')
				assert.ok(addr.startsWith('/jinglestar/'), addr)
				assert.ok(all.has(addr.slice('/jinglestar'.length)), `${id}: ${addr} unknown`)
				for (const a of args) assert.ok(['i', 'f', 's'].includes(a.type) && a.value !== undefined && !Number.isNaN(a.value), id)
			}
		}
		// Toggles read the current state; relative volume starts from it.
		inst.sent = []
		await inst.actions.playlist_shuffle.callback({ options: { module: 2, state: 'toggle' } })
		assert.deepEqual(inst.sent[0].args, [{ type: 'i', value: 0 }])
		inst.sent = []
		await inst.actions.module_volume_adjust.callback({ options: { module: 1, step: -2 } })
		assert.deepEqual(inst.sent[0].args, [{ type: 'f', value: -5 }])

		checkPresets(inst)

		// The state follows the application.
		tree = jingleTree('/jinglestar', [{ ...jingleModules[0], jingles: [{ name: 'Intro', color: '#ff8000', playing: false }] }])
		const before = inst.checks
		await sleep(300)
		assert.equal(fb('jingle_playing', { module: 1, slot: 1 }), false)
		assert.equal(inst.vars.m2_pl_title, '')
		assert.ok(inst.checks > before)
	} finally {
		server.close()
	}
	// Application gone: warning, state cleared, commands still sent.
	await sleep(4000)
	inst.stopPolling()
	assert.equal(inst.connected, false)
	assert.equal(inst.vars.connected, '0')
	assert.equal(inst.vars.m1_j1_name, '')
	assert.match(inst.status.at(-1)[1], /OSCQuery/)
	inst.sent = []
	inst.actions.stop_all.callback({ options: {} })
	assert.equal(inst.sent[0].addr, '/jinglestar/stop_all')
})

test('CueStar: state, variables, feedbacks, actions, presets', async () => {
	const snap = {
		media: 'clip.mp4',
		paused: true,
		time: 61.2,
		remaining: 30,
		loop: true,
		standby: 3,
		standby_name: 'Finale',
		running_name: 'Clip',
		cues: 5,
		slide: [2, 10],
	}
	const server = await serve(() => cueTree('/show', snap))
	const inst = fakeInstance({ app: 'cuestar', host: 'localhost', port: 9500, oscquery: server.address().port, prefix: 'show', poll: 100 })
	try {
		inst.setup()
		await sleep(300)
		assert.equal(inst.connected, true)
		assert.deepEqual(
			{ ...inst.vars },
			{
				connected: '1',
				media: 'clip.mp4',
				time: '1:02',
				remaining: '0:30',
				cue_standby: '3',
				cue_name: 'Finale',
				cue_running: 'Clip',
				cue_count: '5',
				slide_index: '2',
				slide_count: '10',
			},
		)
		const fb = (id, options = {}) => inst.feedbacks[id].callback({ options: { ...defaults(inst.feedbacks[id]), ...options } })
		assert.equal(fb('paused'), true)
		assert.equal(fb('on_screen'), true)
		assert.equal(fb('black'), false)
		assert.equal(fb('cuelist_loop'), true)
		assert.equal(fb('cue_standby_is', { cue: 3 }), true)
		const bar = inst.feedbacks.progress.callback({ options: { colour: 0xffffff, height: 6 }, image: { width: 72, height: 72 } })
		const px = Buffer.from(bar.imageBuffer, 'base64')
		const filled = [...Array(72).keys()].filter((x) => px[x * 4 + 3] === 255).length
		assert.equal(filled, Math.round((61.2 / 91.2) * 72))

		const all = writablePaths(cueTree('/show', snap), '/show')
		for (const [id, def] of Object.entries(inst.actions)) {
			for (const options of combos(def)) {
				if (id === 'slideshow_folder') options.folder = 'D:\\Media'
				inst.sent = []
				await def.callback({ options })
				assert.equal(inst.sent.length, 1, id)
				const { addr, port, args } = inst.sent[0]
				assert.equal(port, 9500)
				assert.ok(all.has(addr.slice('/show'.length)), `${id}: ${addr} unknown`)
				for (const a of args) assert.ok(['i', 'f', 's'].includes(a.type) && !Number.isNaN(a.value), id)
			}
		}
		inst.sent = []
		await inst.actions.cuelist_transition.callback({ options: { kind: 'black', secs: 2 } })
		assert.deepEqual(inst.sent[0].args, [
			{ type: 's', value: 'black' },
			{ type: 'f', value: 2 },
		])
		checkPresets(inst)
	} finally {
		inst.stopPolling()
		server.close()
	}
})

/** Presets only use known actions, feedbacks and variables, with valid options. */
function checkPresets(inst) {
	const varIds = new Set(inst.varDefs.map((v) => v.variableId))
	assert.ok(Object.keys(inst.presets).length > 0)
	for (const [id, p] of Object.entries(inst.presets)) {
		for (const m of p.style.text.matchAll(/\$\(([^:]+):([^)]+)\)/g)) {
			assert.equal(m[1], 'ms', id)
			assert.ok(varIds.has(m[2]), `${id}: variable ${m[2]}`)
		}
		for (const step of p.steps) {
			for (const a of [...step.down, ...step.up]) {
				const def = inst.actions[a.actionId]
				assert.ok(def, `${id}: action ${a.actionId}`)
				for (const k of Object.keys(a.options)) assert.ok(def.options.some((o) => o.id === k), `${id}: option ${k}`)
			}
		}
		for (const f of p.feedbacks) {
			const def = inst.feedbacks[f.feedbackId]
			assert.ok(def, `${id}: feedback ${f.feedbackId}`)
			if (def.type === 'boolean') assert.ok(f.style && Object.keys(f.style).length > 0, `${id}: style of ${f.feedbackId}`)
			if (def.type === 'boolean') assert.ok(!('text' in f.style), `${id}: style of ${f.feedbackId} must keep the text`)
			for (const k of Object.keys(f.options)) assert.ok(def.options.some((o) => o.id === k), `${id}: option ${k}`)
		}
	}
}
