// MediaStar module for Bitfocus Companion: one connection drives one JingleStar or CueStar.
// Commands go out as OSC (UDP); state comes back by reading the OSCQuery tree over HTTP.

import { InstanceBase, InstanceStatus, Regex, combineRgb } from '@companion-module/base'
import * as jinglestar from './jinglestar.js'
import * as cuestar from './cuestar.js'
import { flattenTree, normalizePrefix } from './util.js'

export const APPS = {
	jinglestar: { name: 'JingleStar', impl: jinglestar, port: 8000, oscquery: 8001, prefix: '/jinglestar' },
	cuestar: { name: 'CueStar', impl: cuestar, port: 9000, oscquery: 9001, prefix: '/cuestar' },
}

export class MediaStarInstance extends InstanceBase {
	constructor(internal) {
		super(internal)
		this.state = new Map()
		this.values = {}
		this.connected = false
		this.timer = null
		this.polling = false
		this.failures = 0
	}

	get app() {
		return APPS[this.config?.app] ?? APPS.jinglestar
	}

	get prefix() {
		const p = this.config?.prefix
		return normalizePrefix(p === undefined || String(p).trim() === '' ? this.app.prefix : p)
	}

	async init(config) {
		this.config = config
		this.setup()
	}

	async configUpdated(config) {
		this.config = config
		this.setup()
	}

	async destroy() {
		this.stopPolling()
	}

	getConfigFields() {
		return [
			{
				type: 'static-text',
				id: 'info',
				width: 12,
				label: 'Setup',
				value:
					'In the application, open the remote controls (Preferences) and enable OSC. Enable OSCQuery too: ' +
					'the buttons read the state from it (names, colours, time left). For CueStar on another computer, ' +
					'also allow network access. Leave the prefix empty to use the default one.',
			},
			{
				type: 'dropdown',
				id: 'app',
				label: 'Application',
				width: 6,
				default: 'jinglestar',
				choices: Object.entries(APPS).map(([id, a]) => ({ id, label: a.name })),
			},
			{ type: 'textinput', id: 'host', label: 'Computer (IP address or name)', width: 6, default: '127.0.0.1', regex: Regex.HOSTNAME },
			{
				type: 'number',
				id: 'port',
				label: 'OSC port (0 = default: JingleStar 8000, CueStar 9000)',
				width: 6,
				default: 0,
				min: 0,
				max: 65535,
			},
			{
				type: 'number',
				id: 'oscquery',
				label: 'OSCQuery port (0 = default: JingleStar 8001, CueStar 9001)',
				width: 6,
				default: 0,
				min: 0,
				max: 65535,
			},
			{ type: 'textinput', id: 'prefix', label: 'OSC prefix (empty = /jinglestar or /cuestar)', width: 6, default: '' },
			{ type: 'number', id: 'poll', label: 'State refresh (ms)', width: 6, default: 250, min: 100, max: 5000 },
		]
	}

	oscPort() {
		return Number(this.config?.port) || this.app.port
	}

	oscQueryPort() {
		return Number(this.config?.oscquery) || this.app.oscquery
	}

	host() {
		return String(this.config?.host ?? '').trim() || '127.0.0.1'
	}

	/** Sends one OSC message; `path` is relative to the prefix. */
	sendOsc(path, args = []) {
		const addr = `${this.prefix}${path}`
		this.log('debug', `OSC ${addr} ${JSON.stringify(args.map((a) => a.value))}`)
		this.oscSend(this.host(), this.oscPort(), addr, args)
	}

	setup() {
		this.stopPolling()
		this.state = new Map()
		this.values = {}
		this.connected = false
		this.failures = 0
		const impl = this.app.impl
		this.setActionDefinitions(impl.actions(this))
		this.setFeedbackDefinitions({ ...impl.feedbacks(this), ...this.commonFeedbacks() })
		this.setVariableDefinitions([{ variableId: 'connected', name: 'Connected (state readable)' }, ...impl.variableDefinitions()])
		this.setPresetDefinitions(impl.presets(this))
		this.applyState(new Map())
		this.updateStatus(InstanceStatus.Connecting)
		this.poll()
		this.timer = setInterval(() => this.poll(), Math.max(100, Number(this.config?.poll) || 250))
	}

	stopPolling() {
		if (this.timer) clearInterval(this.timer)
		this.timer = null
	}

	commonFeedbacks() {
		return {
			connected: {
				type: 'boolean',
				name: 'Connection: state readable',
				description: 'True while the OSCQuery state of the application can be read.',
				defaultStyle: { bgcolor: combineRgb(0, 120, 0), color: combineRgb(255, 255, 255) },
				options: [],
				callback: () => this.connected,
			},
		}
	}

	async poll() {
		if (this.polling) return
		this.polling = true
		const url = `http://${this.host()}:${this.oscQueryPort()}/`
		try {
			const res = await fetch(url, { signal: AbortSignal.timeout(1500) })
			if (!res.ok) throw new Error(`HTTP ${res.status}`)
			const tree = await res.json()
			this.failures = 0
			if (!this.connected) {
				this.connected = true
				this.updateStatus(InstanceStatus.Ok)
				this.log('info', `${this.app.name} state read from ${url}`)
			}
			this.applyState(flattenTree(tree, this.prefix))
		} catch (e) {
			this.failures++
			// One missed answer is tolerated; after that the state is cleared.
			if (this.connected && this.failures < 3) return
			if (this.connected || this.failures === 3) {
				this.connected = false
				this.applyState(new Map())
				this.updateStatus(
					InstanceStatus.UnknownWarning,
					`No state from OSCQuery (${url}): commands are still sent. Is OSCQuery enabled in ${this.app.name}?`,
				)
				this.log('warn', `OSCQuery unreachable at ${url}: ${e?.message ?? e}`)
			}
		} finally {
			this.polling = false
		}
	}

	/** Stores the new state, pushes the variables that changed and rechecks the feedbacks. */
	applyState(state) {
		const changed = !sameMap(this.state, state)
		this.state = state
		const next = { connected: this.connected ? '1' : '0', ...this.app.impl.variableValues(state) }
		const diff = {}
		for (const [k, v] of Object.entries(next)) {
			if (this.values[k] !== v) diff[k] = v
		}
		this.values = next
		if (Object.keys(diff).length) this.setVariableValues(diff)
		if (changed || 'connected' in diff) this.checkFeedbacks()
	}
}

function sameMap(a, b) {
	if (a.size !== b.size) return false
	for (const [k, v] of a) {
		if (b.get(k) !== v) return false
	}
	return true
}
