// Shared helpers: OSC prefixes, OSCQuery trees, time and colour formatting.

/** Same rule as the apps (star_remote::osc::normalize_prefix): "/x", no trailing slash, "" for none. */
export function normalizePrefix(prefix) {
	const p = String(prefix ?? '')
		.trim()
		.replace(/\/+$/, '')
	if (p === '') return ''
	return p.startsWith('/') ? p : `/${p}`
}

/**
 * OSCQuery tree → Map(path without prefix → value). Only leaves that carry a VALUE are kept.
 * A path outside the prefix is kept as is.
 */
export function flattenTree(tree, prefix) {
	const out = new Map()
	const p = normalizePrefix(prefix)
	const walk = (node) => {
		if (!node || typeof node !== 'object') return
		if (Array.isArray(node.VALUE) && typeof node.FULL_PATH === 'string') {
			let path = node.FULL_PATH
			if (p && (path === p || path.startsWith(`${p}/`))) path = path.slice(p.length)
			out.set(path, node.VALUE[0])
		}
		if (node.CONTENTS && typeof node.CONTENTS === 'object') {
			for (const child of Object.values(node.CONTENTS)) walk(child)
		}
	}
	walk(tree)
	return out
}

/** Seconds → "m:ss" (or "h:mm:ss"); "" when there is nothing (negative, missing). */
export function formatTime(secs) {
	if (typeof secs !== 'number' || !Number.isFinite(secs) || secs < 0) return ''
	const t = Math.ceil(secs - 0.05)
	const h = Math.floor(t / 3600)
	const m = Math.floor((t % 3600) / 60)
	const s = t % 60
	const ss = String(s).padStart(2, '0')
	return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`
}

/** "#rrggbb" → [r, g, b], or null. */
export function parseHex(hex) {
	const m = /^#?([0-9a-f]{6})$/i.exec(String(hex ?? '').trim())
	if (!m) return null
	const n = parseInt(m[1], 16)
	return [(n >> 16) & 255, (n >> 8) & 255, n & 255]
}

/** Text colour readable on a background. */
export function textOn([r, g, b]) {
	return 0.299 * r + 0.587 * g + 0.114 * b > 150 ? 0x000000 : 0xffffff
}

export function rgb(r, g, b) {
	return ((r & 255) << 16) | ((g & 255) << 8) | (b & 255)
}

export function dim([r, g, b], f) {
	return [Math.round(r * f), Math.round(g * f), Math.round(b * f)]
}

/** OSC booleans: the apps accept 0/1 for every flag. */
export function oscBool(on) {
	return [{ type: 'i', value: on ? 1 : 0 }]
}

/** Number of choices 1..n for a dropdown. */
export function numberChoices(n, label) {
	return Array.from({ length: n }, (_, i) => ({ id: i + 1, label: `${label} ${i + 1}` }))
}

/** Value of a state entry as a boolean (OSCQuery booleans are JSON booleans). */
export function truthy(v) {
	return v === true || v === 1 || v === 'true'
}

/**
 * Progress bar drawn at the bottom of the button (advanced feedback result), or {} when there is
 * no progress (fraction negative or missing). `image` is the size Companion gives the feedback.
 */
export function progressBar(fraction, colour, barHeight, image) {
	if (typeof fraction !== 'number' || !Number.isFinite(fraction) || fraction < 0) return {}
	const width = Math.max(1, Math.round(image?.width ?? 72))
	const fullHeight = Math.max(1, Math.round(image?.height ?? 72))
	const height = Math.min(fullHeight, Math.max(1, Math.round(barHeight || 6)))
	const filled = Math.round(Math.min(1, fraction) * width)
	const c = Number(colour) >>> 0
	const fg = [(c >> 16) & 255, (c >> 8) & 255, c & 255, 255]
	const bg = [0, 0, 0, 160]
	const buf = Buffer.alloc(width * height * 4)
	for (let y = 0; y < height; y++) {
		for (let x = 0; x < width; x++) {
			buf.set(x < filled ? fg : bg, (y * width + x) * 4)
		}
	}
	return {
		imageBuffer: buf.toString('base64'),
		imageBufferEncoding: { pixelFormat: 'RGBA' },
		imageBufferPosition: { x: 0, y: fullHeight - height, width, height },
	}
}

/** Options shared by the progress bar feedbacks. */
export const barOptions = [
	{ type: 'colorpicker', id: 'colour', label: 'Bar colour', default: 0xffffff },
	{ type: 'number', id: 'height', label: 'Bar height (px)', default: 6, min: 1, max: 36 },
]
