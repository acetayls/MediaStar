// CueStar: OSC addresses (apps/cuestar/src/osc.rs), variables, feedbacks and presets.
// CueStar triggers take no argument; cue and media numbers start at 1.

import { combineRgb } from '@companion-module/base'
import { formatTime, oscBool, truthy } from './util.js'

const WHITE = combineRgb(255, 255, 255)
const BLACK = combineRgb(0, 0, 0)
const RED = combineRgb(200, 0, 0)
const GREEN = combineRgb(0, 150, 0)
const ORANGE = combineRgb(230, 130, 0)
const GREY = combineRgb(50, 50, 50)

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
const TRANSITIONS = [
	{ id: 'cut', label: 'Cut' },
	{ id: 'black', label: 'Through black' },
	{ id: 'fade', label: 'Crossfade' },
]

const i = (value) => [{ type: 'i', value: Math.max(1, Math.round(Number(value))) }]

export function actions(self) {
	const send = (path, args = []) => self.sendOsc(path, args)
	const st = (path) => self.state.get(path)
	const wanted = (state, path) => (state === 'on' ? true : state === 'off' ? false : !truthy(st(path)))
	return {
		black: {
			name: 'Output to black',
			options: [],
			callback: () => send('/noir'),
		},
		drops_reset: {
			name: 'Reset the dropped frames counter',
			options: [],
			callback: () => send('/output/drops/reset'),
		},
		cuelist: {
			name: 'Cue list: transport',
			options: [
				{
					type: 'dropdown',
					id: 'action',
					label: 'Action',
					default: 'go',
					choices: [
						{ id: 'go', label: 'GO (cue in standby, or resume)' },
						{ id: 'play', label: 'Play' },
						{ id: 'pause', label: 'Pause' },
						{ id: 'toggle', label: 'Play / pause' },
						{ id: 'stop', label: 'Stop' },
						{ id: 'next', label: 'Next cue in standby' },
						{ id: 'prev', label: 'Previous cue in standby' },
					],
				},
			],
			callback: ({ options }) => send(`/cuelist/${options.action}`),
		},
		cuelist_standby: {
			name: 'Cue list: put cue number in standby',
			options: [{ type: 'number', id: 'cue', label: 'Cue', default: 1, min: 1, max: 9999 }],
			callback: ({ options }) => send('/cuelist/standby', i(options.cue)),
		},
		cuelist_goto: {
			name: 'Cue list: go to cue number now',
			options: [{ type: 'number', id: 'cue', label: 'Cue', default: 1, min: 1, max: 9999 }],
			callback: ({ options }) => send('/cuelist/goto', i(options.cue)),
		},
		cuelist_loop: {
			name: 'Cue list: loop on / off',
			options: [onOffOpt],
			callback: ({ options }) => send('/cuelist/loop', oscBool(wanted(options.state, '/cuelist/loop'))),
		},
		cuelist_transition: {
			name: 'Cue list: transition of the cue in standby',
			options: [
				{ type: 'dropdown', id: 'kind', label: 'Transition', default: 'fade', choices: TRANSITIONS },
				{ type: 'number', id: 'secs', label: 'Duration (s)', default: 1, min: 0.1, max: 10, step: 0.1 },
			],
			callback: ({ options }) =>
				send('/cuelist/transition', [
					{ type: 's', value: options.kind },
					{ type: 'f', value: Number(options.secs) },
				]),
		},
		slideshow: {
			name: 'Slideshow: transport',
			options: [
				{
					type: 'dropdown',
					id: 'action',
					label: 'Action',
					default: 'toggle',
					choices: [
						{ id: 'play', label: 'Play' },
						{ id: 'pause', label: 'Pause' },
						{ id: 'toggle', label: 'Play / pause' },
						{ id: 'stop', label: 'Stop' },
						{ id: 'next', label: 'Next media' },
						{ id: 'prev', label: 'Previous media' },
					],
				},
			],
			callback: ({ options }) => send(`/diaporama/${options.action}`),
		},
		slideshow_goto: {
			name: 'Slideshow: go to media number',
			options: [{ type: 'number', id: 'n', label: 'Media', default: 1, min: 1, max: 99999 }],
			callback: ({ options }) => send('/diaporama/goto', i(options.n)),
		},
		slideshow_setting: {
			name: 'Slideshow: setting on / off',
			options: [
				{
					type: 'dropdown',
					id: 'setting',
					label: 'Setting',
					default: 'shuffle',
					choices: [
						{ id: 'shuffle', label: 'Shuffle' },
						{ id: 'loop', label: 'Loop' },
						{ id: 'recursive', label: 'Include sub-folders' },
						{ id: 'video_sound', label: 'Video sound' },
					],
				},
				{
					type: 'dropdown',
					id: 'on',
					label: 'State',
					default: 'on',
					choices: [
						{ id: 'on', label: 'On' },
						{ id: 'off', label: 'Off' },
					],
				},
			],
			callback: ({ options }) => send(`/diaporama/settings/${options.setting}`, oscBool(options.on === 'on')),
		},
		slideshow_photo_duration: {
			name: 'Slideshow: photo duration',
			options: [{ type: 'number', id: 'secs', label: 'Duration (s)', default: 5, min: 1, max: 60 }],
			callback: ({ options }) => send('/diaporama/settings/photo_duration', [{ type: 'f', value: Number(options.secs) }]),
		},
		slideshow_fit: {
			name: 'Slideshow: fit',
			options: [
				{
					type: 'dropdown',
					id: 'fit',
					label: 'Fit',
					default: 'contain',
					choices: [
						{ id: 'contain', label: 'Contain' },
						{ id: 'cover', label: 'Cover' },
						{ id: 'auto', label: 'Auto' },
					],
				},
			],
			callback: ({ options }) => send('/diaporama/settings/fit', [{ type: 's', value: options.fit }]),
		},
		slideshow_folder: {
			name: 'Slideshow: folder to play',
			options: [{ type: 'textinput', id: 'folder', label: 'Folder (path on the CueStar computer)', default: '', useVariables: true }],
			callback: async ({ options }) => {
				const folder = await self.parseVariablesInString(String(options.folder ?? ''))
				if (folder.trim()) send('/diaporama/settings/folder', [{ type: 's', value: folder }])
			},
		},
	}
}

// ------------------------------------------------------------------ variables

const VARS = [
	['media', 'Media on screen'],
	['time', 'Media position'],
	['remaining', 'Media time left'],
	['cue_standby', 'Number of the cue in standby'],
	['cue_name', 'Cue in standby'],
	['cue_running', 'Last cue started'],
	['cue_count', 'Cue count'],
	['slide_index', 'Slideshow: media number'],
	['slide_count', 'Slideshow: media count'],
]

export function variableDefinitions() {
	return VARS.map(([variableId, name]) => ({ variableId, name }))
}

export function variableValues(state) {
	const g = (p) => state.get(p)
	const str = (v) => (v === undefined || v === null ? '' : String(v))
	const pos = (v) => (typeof v === 'number' && v > 0 ? String(v) : '')
	return {
		media: str(g('/media')),
		time: formatTime(g('/time')),
		remaining: formatTime(g('/remaining')),
		cue_standby: pos(g('/cuelist/standby')),
		cue_name: str(g('/cuelist/standby_name')),
		cue_running: str(g('/cuelist/running_name')),
		cue_count: str(g('/cuelist/cues')),
		slide_index: pos(g('/diaporama/index')),
		slide_count: str(g('/diaporama/count')),
	}
}

// ------------------------------------------------------------------ feedbacks

export function feedbacks(self) {
	const g = (p) => self.state.get(p)
	return {
		paused: {
			type: 'boolean',
			name: 'Media is paused',
			defaultStyle: { bgcolor: ORANGE, color: BLACK },
			options: [],
			callback: () => truthy(g('/paused')) && String(g('/media') ?? '') !== '',
		},
		on_screen: {
			type: 'boolean',
			name: 'A media is on screen (not black)',
			defaultStyle: { bgcolor: GREEN, color: WHITE },
			options: [],
			callback: () => String(g('/media') ?? '') !== '',
		},
		black: {
			type: 'boolean',
			name: 'Output is black',
			defaultStyle: { bgcolor: RED, color: WHITE },
			options: [],
			callback: () => self.connected && String(g('/media') ?? '') === '',
		},
		cuelist_loop: {
			type: 'boolean',
			name: 'Cue list loop is on',
			defaultStyle: { bgcolor: ORANGE, color: BLACK },
			options: [],
			callback: () => truthy(g('/cuelist/loop')),
		},
		cue_standby_is: {
			type: 'boolean',
			name: 'Cue in standby is number',
			defaultStyle: { bgcolor: combineRgb(0, 80, 160), color: WHITE },
			options: [{ type: 'number', id: 'cue', label: 'Cue', default: 1, min: 1, max: 9999 }],
			callback: ({ options }) => Number(g('/cuelist/standby')) === Number(options.cue),
		},
	}
}

// ------------------------------------------------------------------ presets

export function presets(self) {
	const L = self.label
	const v = (id) => `$(${L}:${id})`
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

	const c = 'CueStar: cue list'
	button('go', c, 'GO (cue in standby)', `GO\\n${v('cue_standby')} ${v('cue_name')}`, press('cuelist', { action: 'go' }), [], {
		bgcolor: combineRgb(0, 90, 0),
	})
	button('prev', c, 'Previous cue', '▲ CUE', press('cuelist', { action: 'prev' }))
	button('next', c, 'Next cue', '▼ CUE', press('cuelist', { action: 'next' }))
	button(
		'toggle',
		c,
		'Play / pause',
		`⏯\\n${v('remaining')}`,
		press('cuelist', { action: 'toggle' }),
		[{ feedbackId: 'paused', options: {} }],
	)
	button('stop', c, 'Stop', '⏹', press('cuelist', { action: 'stop' }))
	button('black', c, 'Black', 'BLACK', press('black'), [{ feedbackId: 'black', options: {} }])
	button(
		'loop',
		c,
		'Loop',
		'LOOP',
		press('cuelist_loop', { state: 'toggle' }),
		[{ feedbackId: 'cuelist_loop', options: {} }],
	)
	button('now', c, 'Now playing', `${v('cue_running')}\\n${v('time')} / -${v('remaining')}`, [{ down: [], up: [] }], [
		{ feedbackId: 'on_screen', options: {} },
	])

	const d = 'CueStar: slideshow'
	button('sl_toggle', d, 'Play / pause', `⏯\\n${v('slide_index')}/${v('slide_count')}`, press('slideshow', { action: 'toggle' }))
	button('sl_prev', d, 'Previous media', '⏮', press('slideshow', { action: 'prev' }))
	button('sl_next', d, 'Next media', '⏭', press('slideshow', { action: 'next' }))
	button('sl_stop', d, 'Stop', '⏹', press('slideshow', { action: 'stop' }))
	return out
}
