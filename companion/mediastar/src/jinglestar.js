// JingleStar: OSC addresses (apps/jinglestar/src/osc.rs), variables, feedbacks and presets.
// Numbers of modules, jingles, tracks and cues start at 1 in the addresses.

import { combineRgb } from '@companion-module/base'
import { dim, formatTime, numberChoices, oscBool, parseHex, rgb, textOn, truthy } from './util.js'

export const MODULES = 4
export const SLOTS = 12
export const MODULE_DB = [-60, 10]
export const JINGLE_DB = [-40, 10]

const MODES = [
	{ id: 'jingle', label: 'Jingle' },
	{ id: 'playlist', label: 'Playlist' },
	{ id: 'text', label: 'Text' },
	{ id: 'go', label: 'GO (cue list)' },
]

const WHITE = combineRgb(255, 255, 255)
const BLACK = combineRgb(0, 0, 0)
const RED = combineRgb(200, 0, 0)
const GREEN = combineRgb(0, 150, 0)
const ORANGE = combineRgb(230, 130, 0)
const GREY = combineRgb(50, 50, 50)

const moduleOpt = {
	type: 'dropdown',
	id: 'module',
	label: 'Module',
	default: 1,
	choices: numberChoices(MODULES, 'Module'),
}
const slotOpt = {
	type: 'dropdown',
	id: 'slot',
	label: 'Jingle',
	default: 1,
	choices: numberChoices(SLOTS, 'Jingle'),
}
const onOffOpt = {
	type: 'dropdown',
	id: 'state',
	label: 'State',
	default: 'toggle',
	choices: [
		{ id: 'on', label: 'On' },
		{ id: 'off', label: 'Off' },
		{ id: 'toggle', label: 'Toggle' },
	],
}

const clamp = (v, [lo, hi]) => Math.min(hi, Math.max(lo, v))
const f = (value) => [{ type: 'f', value }]
const i = (value) => [{ type: 'i', value: Math.round(value) }]

/** On/Off/Toggle → boolean, from the current state. */
function wanted(state, current) {
	return state === 'on' ? true : state === 'off' ? false : !truthy(current)
}

export function actions(self) {
	const send = (path, args = []) => self.sendOsc(path, args)
	const st = (path) => self.state.get(path)
	const m = (o) => `/module/${o.module}`
	return {
		stop_all: {
			name: 'Stop all sounds',
			options: [],
			callback: () => send('/stop_all'),
		},
		go_first: {
			name: 'GO (first GO module)',
			options: [],
			callback: () => send('/go'),
		},
		module_stop: {
			name: 'Module: stop',
			options: [moduleOpt],
			callback: ({ options }) => send(`${m(options)}/stop`),
		},
		module_volume: {
			name: 'Module: set volume',
			options: [moduleOpt, { type: 'number', id: 'db', label: 'Volume (dB)', default: 0, min: MODULE_DB[0], max: MODULE_DB[1], step: 0.5 }],
			callback: ({ options }) => send(`${m(options)}/volume`, f(clamp(Number(options.db), MODULE_DB))),
		},
		module_volume_adjust: {
			name: 'Module: adjust volume',
			options: [moduleOpt, { type: 'number', id: 'step', label: 'Step (dB, negative lowers)', default: 1, min: -20, max: 20, step: 0.5 }],
			callback: ({ options }) => {
				const cur = Number(st(`${m(options)}/volume`) ?? 0)
				send(`${m(options)}/volume`, f(clamp(cur + Number(options.step), MODULE_DB)))
			},
		},
		module_mode: {
			name: 'Module: set mode',
			options: [moduleOpt, { type: 'dropdown', id: 'mode', label: 'Mode', default: 'jingle', choices: MODES }],
			callback: ({ options }) => send(`${m(options)}/mode`, [{ type: 's', value: options.mode }]),
		},
		jingle: {
			name: 'Jingle: play / stop / toggle',
			options: [
				moduleOpt,
				slotOpt,
				{
					type: 'dropdown',
					id: 'action',
					label: 'Action',
					default: 'toggle',
					choices: [
						{ id: 'toggle', label: 'Toggle (play or stop)' },
						{ id: 'play', label: 'Play from the start' },
						{ id: 'stop', label: 'Stop' },
					],
				},
			],
			callback: ({ options }) => send(`${m(options)}/jingle/${options.slot}/${options.action}`),
		},
		jingle_volume: {
			name: 'Jingle: set volume',
			options: [moduleOpt, slotOpt, { type: 'number', id: 'db', label: 'Volume (dB)', default: 0, min: JINGLE_DB[0], max: JINGLE_DB[1], step: 0.5 }],
			callback: ({ options }) => send(`${m(options)}/jingle/${options.slot}/volume`, f(clamp(Number(options.db), JINGLE_DB))),
		},
		jingle_volume_adjust: {
			name: 'Jingle: adjust volume',
			options: [moduleOpt, slotOpt, { type: 'number', id: 'step', label: 'Step (dB, negative lowers)', default: 1, min: -20, max: 20, step: 0.5 }],
			callback: ({ options }) => {
				const path = `${m(options)}/jingle/${options.slot}/volume`
				send(path, f(clamp(Number(st(path) ?? 0) + Number(options.step), JINGLE_DB)))
			},
		},
		playlist: {
			name: 'Playlist: transport',
			options: [
				moduleOpt,
				{
					type: 'dropdown',
					id: 'action',
					label: 'Action',
					default: 'play',
					choices: [
						{ id: 'play', label: 'Play (selected track)' },
						{ id: 'stop', label: 'Stop' },
						{ id: 'pause', label: 'Pause / resume' },
						{ id: 'next', label: 'Next track' },
						{ id: 'previous', label: 'Previous track' },
					],
				},
			],
			callback: ({ options }) => send(`${m(options)}/playlist/${options.action}`),
		},
		playlist_track: {
			name: 'Playlist: play track number',
			options: [moduleOpt, { type: 'number', id: 'track', label: 'Track', default: 1, min: 1, max: 9999 }],
			callback: ({ options }) => send(`${m(options)}/playlist/track`, i(Math.max(1, Number(options.track)))),
		},
		playlist_paused: {
			name: 'Playlist: pause on / off',
			options: [moduleOpt, onOffOpt],
			callback: ({ options }) => {
				const path = `${m(options)}/playlist/paused`
				send(path, oscBool(wanted(options.state, st(path))))
			},
		},
		playlist_shuffle: {
			name: 'Playlist: shuffle on / off',
			options: [moduleOpt, onOffOpt],
			callback: ({ options }) => {
				const path = `${m(options)}/playlist/shuffle`
				send(path, oscBool(wanted(options.state, st(path))))
			},
		},
		playlist_loop: {
			name: 'Playlist: loop the list on / off',
			options: [moduleOpt, onOffOpt],
			callback: ({ options }) => {
				const path = `${m(options)}/playlist/loop`
				send(path, oscBool(wanted(options.state, st(path))))
			},
		},
		cue_go: {
			name: 'Cue list: GO',
			options: [moduleOpt],
			callback: ({ options }) => send(`${m(options)}/go`),
		},
		cue: {
			name: 'Cue list: standby / pause',
			options: [
				moduleOpt,
				{
					type: 'dropdown',
					id: 'action',
					label: 'Action',
					default: 'next',
					choices: [
						{ id: 'next', label: 'Next cue in standby' },
						{ id: 'previous', label: 'Previous cue in standby' },
						{ id: 'pause', label: 'Pause / resume the cues playing' },
					],
				},
			],
			callback: ({ options }) => send(`${m(options)}/cue/${options.action}`),
		},
		cue_standby: {
			name: 'Cue list: put cue number in standby',
			options: [moduleOpt, { type: 'number', id: 'cue', label: 'Cue', default: 1, min: 1, max: 9999 }],
			callback: ({ options }) => send(`${m(options)}/cue/standby`, i(Math.max(1, Number(options.cue)))),
		},
		cue_paused: {
			name: 'Cue list: pause on / off',
			options: [moduleOpt, onOffOpt],
			callback: ({ options }) => {
				const path = `${m(options)}/cue/paused`
				send(path, oscBool(wanted(options.state, st(path))))
			},
		},
	}
}

// ------------------------------------------------------------------ variables

export function variableDefinitions() {
	const defs = []
	for (let mi = 1; mi <= MODULES; mi++) {
		const v = (id, name) => defs.push({ variableId: `m${mi}_${id}`, name: `Module ${mi}: ${name}` })
		v('name', 'name')
		v('mode', 'mode')
		v('volume', 'volume (dB)')
		for (let s = 1; s <= SLOTS; s++) {
			v(`j${s}_name`, `jingle ${s} name`)
			v(`j${s}_remaining`, `jingle ${s} time left`)
			v(`j${s}_volume`, `jingle ${s} volume (dB)`)
		}
		v('pl_title', 'playlist track title')
		v('pl_track', 'playlist track number')
		v('pl_tracks', 'playlist track count')
		v('pl_remaining', 'playlist time left')
		v('cue_name', 'cue in standby')
		v('cue_standby', 'number of the cue in standby')
		v('cue_count', 'cue count')
		v('cue_playing_name', 'cue playing')
		v('cue_remaining', 'time left of the cue playing')
	}
	return defs
}

const str = (v) => (v === undefined || v === null ? '' : String(v))
const num = (v, digits = 1) => (typeof v === 'number' ? String(Number(v.toFixed(digits))) : '')

export function variableValues(state) {
	const out = {}
	const g = (p) => state.get(p)
	for (let mi = 1; mi <= MODULES; mi++) {
		const b = `/module/${mi}`
		const set = (id, v) => (out[`m${mi}_${id}`] = v)
		set('name', str(g(`${b}/name`)))
		set('mode', str(g(`${b}/mode`)))
		set('volume', num(g(`${b}/volume`)))
		for (let s = 1; s <= SLOTS; s++) {
			const j = `${b}/jingle/${s}`
			set(`j${s}_name`, str(g(`${j}/name`)))
			set(`j${s}_remaining`, formatTime(g(`${j}/remaining`)))
			set(`j${s}_volume`, num(g(`${j}/volume`)))
		}
		const pl = `${b}/playlist`
		set('pl_title', str(g(`${pl}/title`)))
		const track = g(`${pl}/track`)
		set('pl_track', typeof track === 'number' && track > 0 ? String(track) : '')
		set('pl_tracks', str(g(`${pl}/tracks`)))
		set('pl_remaining', formatTime(g(`${pl}/remaining`)))
		set('cue_name', str(g(`${b}/cue/name`)))
		const sb = g(`${b}/cue/standby`)
		set('cue_standby', typeof sb === 'number' && sb > 0 ? String(sb) : '')
		set('cue_count', str(g(`${b}/cue/count`)))
		set('cue_playing_name', str(g(`${b}/cue/playing_name`)))
		set('cue_remaining', formatTime(g(`${b}/cue/remaining`)))
	}
	return out
}

// ------------------------------------------------------------------ feedbacks

export function feedbacks(self) {
	const g = (p) => self.state.get(p)
	const bool = (name, options, path, style, description) => ({
		type: 'boolean',
		name,
		description,
		defaultStyle: style,
		options,
		callback: ({ options: o }) => truthy(g(path(o))),
	})
	return {
		jingle_playing: bool('Jingle is playing', [moduleOpt, slotOpt], (o) => `/module/${o.module}/jingle/${o.slot}/playing`, {
			bgcolor: RED,
			color: WHITE,
		}),
		jingle_colour: {
			type: 'advanced',
			name: 'Jingle colour (bright when playing)',
			description: 'Button background takes the colour of the jingle in JingleStar, dimmed when stopped.',
			options: [
				moduleOpt,
				slotOpt,
				{ type: 'number', id: 'dim', label: 'Brightness when stopped (%)', default: 35, min: 0, max: 100 },
			],
			callback: ({ options: o }) => {
				const j = `/module/${o.module}/jingle/${o.slot}`
				const name = g(`${j}/name`)
				const c = parseHex(g(`${j}/color`))
				if (!c || !name) return {}
				const shown = truthy(g(`${j}/playing`)) ? c : dim(c, Number(o.dim) / 100)
				return { bgcolor: rgb(...shown), color: textOn(shown) }
			},
		},
		playlist_playing: bool('Playlist is playing', [moduleOpt], (o) => `/module/${o.module}/playlist/playing`, { bgcolor: GREEN, color: WHITE }),
		playlist_paused: bool('Playlist is paused', [moduleOpt], (o) => `/module/${o.module}/playlist/paused`, { bgcolor: ORANGE, color: BLACK }),
		playlist_shuffle: bool('Playlist shuffle is on', [moduleOpt], (o) => `/module/${o.module}/playlist/shuffle`, { bgcolor: ORANGE, color: BLACK }),
		playlist_loop: bool('Playlist loop is on', [moduleOpt], (o) => `/module/${o.module}/playlist/loop`, { bgcolor: ORANGE, color: BLACK }),
		cue_playing: {
			type: 'boolean',
			name: 'Cue list: a cue is playing',
			defaultStyle: { bgcolor: GREEN, color: WHITE },
			options: [moduleOpt],
			callback: ({ options: o }) => Number(g(`/module/${o.module}/cue/playing`) ?? 0) > 0,
		},
		cue_paused: bool('Cue list is paused', [moduleOpt], (o) => `/module/${o.module}/cue/paused`, { bgcolor: ORANGE, color: BLACK }),
		module_mode: {
			type: 'boolean',
			name: 'Module is in mode',
			defaultStyle: { bgcolor: combineRgb(0, 80, 160), color: WHITE },
			options: [moduleOpt, { type: 'dropdown', id: 'mode', label: 'Mode', default: 'jingle', choices: MODES }],
			callback: ({ options: o }) => g(`/module/${o.module}/mode`) === o.mode,
		},
	}
}

// ------------------------------------------------------------------ presets

export function presets(self) {
	const L = self.label
	const out = {}
	const button = (id, category, name, text, steps, fb = [], style = {}) => {
		out[id] = {
			type: 'button',
			category,
			name,
			style: { text, size: 'auto', color: WHITE, bgcolor: GREY, ...style },
			steps,
			feedbacks: fb,
		}
	}
	const press = (actionId, options = {}) => [{ down: [{ actionId, options }], up: [] }]

	button('stop_all', 'JingleStar', 'Stop all', 'STOP\\nALL', press('stop_all'), [], { bgcolor: RED })
	button('go_first', 'JingleStar', 'GO', 'GO', press('go_first'), [], { bgcolor: GREEN })

	for (let mi = 1; mi <= MODULES; mi++) {
		const v = (id) => `$(${L}:m${mi}_${id})`
		const cat = `Module ${mi}: jingles`
		for (let s = 1; s <= SLOTS; s++) {
			button(
				`m${mi}_j${s}`,
				cat,
				`Jingle ${s}`,
				`${v(`j${s}_name`)}\\n${v(`j${s}_remaining`)}`,
				press('jingle', { module: mi, slot: s, action: 'toggle' }),
				[{ feedbackId: 'jingle_colour', options: { module: mi, slot: s, dim: 35 } }],
			)
		}
		button(`m${mi}_stop`, cat, 'Stop module', `STOP\\n${v('name')}`, press('module_stop', { module: mi }), [], { bgcolor: RED })

		const pcat = `Module ${mi}: playlist`
		button(
			`m${mi}_pl_play`,
			pcat,
			'Play',
			`▶\\n${v('pl_title')}\\n${v('pl_remaining')}`,
			press('playlist', { module: mi, action: 'play' }),
			[{ feedbackId: 'playlist_playing', options: { module: mi } }],
		)
		button(
			`m${mi}_pl_pause`,
			pcat,
			'Pause',
			'⏸',
			press('playlist', { module: mi, action: 'pause' }),
			[{ feedbackId: 'playlist_paused', options: { module: mi } }],
		)
		button(`m${mi}_pl_stop`, pcat, 'Stop', '⏹', press('playlist', { module: mi, action: 'stop' }))
		button(`m${mi}_pl_prev`, pcat, 'Previous', '⏮', press('playlist', { module: mi, action: 'previous' }))
		button(`m${mi}_pl_next`, pcat, 'Next', '⏭', press('playlist', { module: mi, action: 'next' }))
		button(
			`m${mi}_pl_shuffle`,
			pcat,
			'Shuffle',
			'SHUFFLE',
			press('playlist_shuffle', { module: mi, state: 'toggle' }),
			[{ feedbackId: 'playlist_shuffle', options: { module: mi } }],
		)
		button(
			`m${mi}_pl_loop`,
			pcat,
			'Loop',
			'LOOP',
			press('playlist_loop', { module: mi, state: 'toggle' }),
			[{ feedbackId: 'playlist_loop', options: { module: mi } }],
		)

		const ccat = `Module ${mi}: cue list`
		button(
			`m${mi}_go`,
			ccat,
			'GO (cue in standby)',
			`GO\\n${v('cue_standby')} ${v('cue_name')}`,
			press('cue_go', { module: mi }),
			[{ feedbackId: 'cue_playing', options: { module: mi } }],
			{ bgcolor: combineRgb(0, 90, 0) },
		)
		button(`m${mi}_cue_prev`, ccat, 'Previous cue', '▲ CUE', press('cue', { module: mi, action: 'previous' }))
		button(`m${mi}_cue_next`, ccat, 'Next cue', '▼ CUE', press('cue', { module: mi, action: 'next' }))
		button(
			`m${mi}_cue_pause`,
			ccat,
			'Pause cues',
			`PAUSE\\n${v('cue_playing_name')}\\n${v('cue_remaining')}`,
			press('cue', { module: mi, action: 'pause' }),
			[{ feedbackId: 'cue_paused', options: { module: mi } }],
		)
		button(`m${mi}_cue_stop`, ccat, 'Stop module', 'STOP', press('module_stop', { module: mi }), [], { bgcolor: RED })
	}
	return out
}
