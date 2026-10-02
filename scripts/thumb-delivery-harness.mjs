// Development-only measurements of thumbnail delivery through the `thumb://` scheme in the real grid
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

// Not shipped and not imported by the app: a development build's webview loads it through Vite's
// file server, from the console or the Tauri MCP bridge, after a folder of images is open in the grid
// (`scripts/thumb-fixture.sh` makes one; `docs/architecture/milestone-5-spikes.md` says how to run it):
//
//   const h = await import('/@fs/<repo>/scripts/thumb-delivery-harness.mjs');
//   const pm = await import('/src/dev/perfHarness.ts');
//   await pm.openFolder('/tmp/waypoint-thumbs'); await pm.showGrid();
//   await h.fillCache(); await h.timeToPaint(40); await h.serveTimes(); ...
//
// Every function returns a JSON-serialisable summary. It reads the page only (the listbox, its
// images, `requestAnimationFrame`, `fetch`). `invoke` is frozen in the webview, so the thumbnail
// commands are counted from outside (the MCP bridge's `ipc_monitor`).

const raf = () => new Promise((resolve) => requestAnimationFrame(resolve));
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const round = (value) => Math.round(value * 100) / 100;

/** Percentiles of a sample, rounded to hundredths. */
export function stats(values) {
	if (values.length === 0) return { n: 0 };
	const sorted = [...values].sort((a, b) => a - b);
	const at = (p) => sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))];
	return {
		n: sorted.length,
		p50: round(at(50)),
		p95: round(at(95)),
		max: round(sorted[sorted.length - 1]),
	};
}

function find() {
	const listbox = document.querySelector('[role="listbox"]');
	const scroller = listbox?.parentElement;
	if (!listbox || !scroller) throw new Error('Open a folder in the grid first.');
	return { listbox, scroller };
}

/** The tiles drawn now (each has its icon, and a picture over it once the thumbnail is ready), and which are inside the scroller's box. */
function tiles() {
	const { listbox, scroller } = find();
	const box = scroller.getBoundingClientRect();
	const all = [...listbox.querySelectorAll('[role="option"]')];
	const visible = all.filter((tile) => {
		const r = tile.getBoundingClientRect();
		return r.bottom > box.top && r.top < box.bottom;
	});
	return { all, visible };
}

/** A tile is painted when its picture has loaded; a tile still showing its icon is not. */
const painted = (tile) => {
	const img = tile.querySelector('img');
	return img !== null && img.complete && img.naturalWidth > 0;
};

/** Every address seen on a picture while the harness watches, so later passes can ask for the same ones. */
const seen = new Set();
let observer = null;
function watch() {
	if (observer) return;
	const note = (node) => {
		if (node instanceof HTMLImageElement && node.src.startsWith('thumb:')) seen.add(node.src);
		else if (node instanceof Element)
			node
				.querySelectorAll('img')
				.forEach((img) => img.src.startsWith('thumb:') && seen.add(img.src));
	};
	observer = new MutationObserver((records) => {
		for (const record of records) {
			record.addedNodes.forEach(note);
			if (record.type === 'attributes') note(record.target);
		}
	});
	observer.observe(document.body, {
		childList: true,
		subtree: true,
		attributes: true,
		attributeFilter: ['src'],
	});
}

/**
 * Scrolls down a screen at a time, waiting for each screen's pictures, so the cache fills (cold
 * generation through the bridge). Returns how long each screen took to be fully painted.
 */
export async function fillCache({ step = 0.9, timeoutMs = 20000, maxScreens = 400 } = {}) {
	watch();
	const { scroller } = find();
	scroller.scrollTop = 0;
	await sleep(500);
	const perScreen = [];
	const t0 = performance.now();
	let timeouts = 0;
	for (let i = 0; i < maxScreens; i++) {
		const start = performance.now();
		for (;;) {
			const { visible } = tiles();
			if (visible.length > 0 && visible.every(painted)) break;
			if (performance.now() - start > timeoutMs) {
				timeouts++;
				break;
			}
			await raf();
		}
		perScreen.push(performance.now() - start);
		globalThis.__fill = { screen: i, lastMs: round(perScreen[perScreen.length - 1]), timeouts };
		const limit = scroller.scrollHeight - scroller.clientHeight;
		if (scroller.scrollTop >= limit - 1) break;
		scroller.scrollTop = Math.min(limit, scroller.scrollTop + scroller.clientHeight * step);
		await raf();
	}
	return {
		screens: perScreen.length,
		totalSeconds: round((performance.now() - t0) / 1000),
		screenMs: stats(perScreen),
		timeouts,
		addressesSeen: seen.size,
	};
}

/**
 * Jumps to `count` random positions (what a scrollbar drag or Page Down does) and times, from the
 * jump, the first picture appearing and every picture in view being painted. The pictures are in
 * the disk cache by now but mostly not in the loader's memory, so this is the cache-hit path:
 * `invoke` to the bridge, the plugin finding the file, `Channel` back, then `thumb://` and decoding.
 */
export async function timeToPaint(count = 40) {
	watch();
	const { scroller } = find();
	const limit = scroller.scrollHeight - scroller.clientHeight;
	const first = [];
	const all = [];
	const tileCount = [];
	let unpainted = 0;
	let seed = 12345;
	const rand = () => (seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648;
	for (let i = 0; i < count; i++) {
		scroller.scrollTop = 0;
		await sleep(400);
		const target = Math.floor(rand() * limit);
		const start = performance.now();
		scroller.scrollTop = target;
		let firstAt = null;
		for (;;) {
			const { visible } = tiles();
			const done = visible.filter(painted).length;
			if (firstAt === null && done > 0) firstAt = performance.now() - start;
			if (visible.length > 0 && done === visible.length) {
				all.push(performance.now() - start);
				tileCount.push(visible.length);
				break;
			}
			if (performance.now() - start > 10000) {
				unpainted++;
				break;
			}
			await raf();
		}
		if (firstAt !== null) first.push(firstAt);
		await sleep(150);
	}
	return {
		jumps: count,
		tilesInView: stats(tileCount),
		firstPictureMs: stats(first),
		allInViewMs: stats(all),
		allInViewEachMs: all.map(Math.round),
		over150ms: all.filter((v) => v > 150).length,
		unpainted,
	};
}

/** Time to fetch one thumbnail the way an `<img>` does: through the scheme handler, from the plugin's cache. */
export async function serveTimes({ sequential = 1000, parallel = 60, rounds = 10 } = {}) {
	const urls = [...seen];
	if (urls.length < sequential)
		throw new Error(`Only ${urls.length} addresses seen: run fillCache first.`);
	const bust = (url, n) => `${url}&r=${n}-${Math.random().toString(36).slice(2)}`;
	const one = async (url) => {
		const start = performance.now();
		const response = await fetch(url, { cache: 'no-store' });
		const bytes = (await response.arrayBuffer()).byteLength;
		return { ms: performance.now() - start, ok: response.ok, bytes };
	};
	const seq = [];
	let failed = 0;
	let bytes = 0;
	for (let i = 0; i < sequential; i++) {
		const r = await one(bust(urls[(i * 7) % urls.length], i));
		seq.push(r.ms);
		bytes += r.bytes;
		if (!r.ok) failed++;
	}
	const batchMs = [];
	const perInBatch = [];
	for (let round = 0; round < rounds; round++) {
		const start = performance.now();
		const results = await Promise.all(
			Array.from({ length: parallel }, (_, i) =>
				one(bust(urls[(round * parallel + i * 13) % urls.length], i)),
			),
		);
		batchMs.push(performance.now() - start);
		perInBatch.push(...results.map((r) => r.ms));
	}
	return {
		sequential: stats(seq),
		meanBytes: Math.round(bytes / sequential),
		failed,
		batchOf: parallel,
		batchWallMs: stats(batchMs),
		perRequestInBatchMs: stats(perInBatch),
	};
}

/** Scrolls at a steady rate and reports frame intervals (rAF deltas) and how long the pictures took to settle after it. */
export async function sweep(pxPerFrame, maxFrames, { fromPx = 0 } = {}) {
	const { scroller } = find();
	scroller.scrollTop = fromPx;
	await raf();
	await raf();
	let last = await raf();
	const limit = scroller.scrollHeight - scroller.clientHeight;
	const frames = [];
	let blank = 0;
	let maxImgs = 0;
	for (let i = 0; i < maxFrames && scroller.scrollTop < limit; i++) {
		scroller.scrollTop = Math.min(limit, scroller.scrollTop + pxPerFrame);
		const now = await raf();
		frames.push(now - last);
		last = now;
		const { all, visible } = tiles();
		maxImgs = Math.max(maxImgs, all.length);
		if (visible.some((img) => !painted(img))) blank++;
	}
	const settleStart = performance.now();
	for (;;) {
		const { visible } = tiles();
		if (visible.length > 0 && visible.every(painted)) break;
		if (performance.now() - settleStart > 10000) break;
		await raf();
	}
	return {
		pxPerFrame,
		frameMs: stats(frames),
		// Frame times are whole milliseconds here, so 17 is a normal 60 Hz frame: over 20 is a missed one.
		over16_7ms: frames.filter((f) => f > 16.7).length,
		over20ms: frames.filter((f) => f > 20).length,
		over33ms: frames.filter((f) => f > 33.4).length,
		over50ms: frames.filter((f) => f > 50).length,
		framesWithAnUnpaintedTile: blank,
		mostTilesInDom: maxImgs,
		msFromLastScrollToAllPainted: round(performance.now() - settleStart),
	};
}

/**
 * Sixty tiles drawn three ways from `thumb://`: `<img decoding="sync">`, `<img decoding="async">`
 * and `fetch` then `createImageBitmap` onto a canvas. Reports the time until every tile has painted
 * and the worst frame interval while it did. Each round uses different pictures and a cache-busting
 * query so the webview's own cache does not answer.
 */
export async function decodeCompare({ rounds = 20, tilesPerRound = 60 } = {}) {
	const urls = [...seen];
	if (urls.length < rounds * tilesPerRound) throw new Error('Run fillCache first.');
	const host = document.createElement('div');
	host.style.cssText =
		'position:fixed;inset:0;z-index:99999;background:#222;display:grid;grid-template-columns:repeat(12,64px);gap:4px;padding:8px;align-content:start';
	document.body.append(host);
	const out = {};
	const variants = {
		imgSync: (url) => {
			const img = new Image();
			img.decoding = 'sync';
			img.width = img.height = 64;
			img.style.objectFit = 'cover';
			img.src = url;
			host.append(img);
			return new Promise((resolve) => (img.onload = img.onerror = resolve));
		},
		imgAsync: (url) => {
			const img = new Image();
			img.decoding = 'async';
			img.width = img.height = 64;
			img.style.objectFit = 'cover';
			img.src = url;
			host.append(img);
			return new Promise((resolve) => (img.onload = img.onerror = resolve));
		},
		imgDecodeCall: async (url) => {
			const img = new Image();
			img.width = img.height = 64;
			img.style.objectFit = 'cover';
			img.src = url;
			await img.decode().catch(() => undefined);
			host.append(img);
		},
		bitmapCanvas: async (url) => {
			const blob = await (await fetch(url, { cache: 'no-store' })).blob();
			const bitmap = await createImageBitmap(blob, { resizeWidth: 64, resizeHeight: 64 });
			const canvas = document.createElement('canvas');
			canvas.width = canvas.height = 64;
			canvas.getContext('2d').drawImage(bitmap, 0, 0);
			bitmap.close();
			host.append(canvas);
		},
	};
	let offset = 0;
	for (const [name, make] of Object.entries(variants)) {
		const wall = [];
		const worstFrame = [];
		for (let round = 0; round < rounds; round++) {
			host.replaceChildren();
			await sleep(100);
			let last = await raf();
			let worst = 0;
			let running = true;
			const watcher = (async () => {
				while (running) {
					const now = await raf();
					worst = Math.max(worst, now - last);
					last = now;
				}
			})();
			const start = performance.now();
			await Promise.all(
				Array.from({ length: tilesPerRound }, (_, i) =>
					make(`${urls[(offset + i) % urls.length]}&d=${name}${round}`),
				),
			);
			await raf();
			await raf();
			wall.push(performance.now() - start);
			running = false;
			await watcher;
			worstFrame.push(worst);
			offset += tilesPerRound;
		}
		out[name] = { allPaintedMs: stats(wall), worstFrameMs: stats(worstFrame) };
	}
	host.remove();
	return out;
}
