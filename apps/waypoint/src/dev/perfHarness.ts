// Development-only timing harness: scripted scrolling, jumps, select-all and re-sorts against the real list
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/**
 * Drives the list that is on screen and reports frame times and how long rows take to appear, so the
 * budgets in `docs/architecture/milestone-0-spikes.md` can be re-measured on the real components
 * over a real folder. It is installed as `window.__waypointPerf` in a development build, or in any
 * build made with `VITE_WAYPOINT_PERF=1` (see `MainScreen`). Open a large folder
 * (`scripts/perf-fixture.sh` makes one), then call `await __waypointPerf.runAll()` from the webview
 * console or the Tauri MCP bridge.
 *
 * A release build has no bridge, so it can also run itself: load the window with the URL hash
 * `#perf-auto=/path/to/folder` and it opens that folder in the active tab, runs everything and logs
 * one `PERF_RESULT {json}` line through the log plugin. Add `&max` to maximise the window first and
 * `&grid` to measure the grid view as well (`#perf-auto=/path&max&grid`).
 */

const raf = () => new Promise<number>((resolve) => requestAnimationFrame(resolve));
const round = (value: number) => Math.round(value * 100) / 100;

export interface Stats {
	n: number;
	p50?: number;
	p95?: number;
	p99?: number;
	max?: number;
}

/** Percentiles of a sample, rounded to hundredths. */
export function stats(values: number[]): Stats {
	if (values.length === 0) return { n: 0 };
	const sorted = [...values].sort((a, b) => a - b);
	const at = (p: number) =>
		sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))]!;
	return {
		n: sorted.length,
		p50: round(at(50)),
		p95: round(at(95)),
		p99: round(at(99)),
		max: round(sorted[sorted.length - 1]!),
	};
}

interface ListElements {
	listbox: HTMLElement;
	/** The scrolling element: the listbox's parent. */
	scroller: HTMLElement;
	rowHeight: number;
	rowCount: number;
}

function findList(): ListElements {
	const listbox = document.querySelector<HTMLElement>('[role="listbox"]');
	const scroller = listbox?.parentElement;
	if (!listbox || !scroller) throw new Error('No file list is on screen: open a folder first.');
	const rowHeight =
		parseFloat(getComputedStyle(document.documentElement).getPropertyValue('--wp-row-height')) ||
		28;
	const rowCount = Number(listbox.getAttribute('aria-rowcount')) || 0;
	return { listbox, scroller, rowHeight, rowCount };
}

/** How many rows in view are still placeholders (waiting for their page). */
const placeholders = (list: ListElements) =>
	list.listbox.querySelectorAll('[data-placeholder]').length;

/** Scrolls at a steady rate, recording frame intervals and how often rows were still loading. */
export async function sweep(pxPerFrame: number, maxFrames: number, fromPx = 0) {
	const list = findList();
	const { scroller } = list;
	scroller.scrollTop = fromPx;
	await raf();
	await raf();
	let last = await raf();
	const limit = scroller.scrollHeight - scroller.clientHeight;
	const frames: number[] = [];
	let blankFrames = 0;
	for (let i = 0; i < maxFrames && scroller.scrollTop < limit; i++) {
		scroller.scrollTop = Math.min(limit, scroller.scrollTop + pxPerFrame);
		const now = await raf();
		frames.push(now - last);
		last = now;
		if (placeholders(list) > 0) blankFrames++;
	}
	return {
		pxPerFrame,
		rowsPerFrame: Math.round(pxPerFrame / list.rowHeight),
		frameMs: stats(frames),
		over20ms: frames.filter((f) => f > 20).length,
		over33ms: frames.filter((f) => f > 33).length,
		blankFrames,
		scrolledPx: scroller.scrollTop - fromPx,
		scrollHeight: scroller.scrollHeight,
		// The view replaced its scroller while the sweep ran: the measurement above is then void.
		scrollerReplaced: !document.contains(scroller),
	};
}

/** Whether the row at `target` is rendered, so the virtualiser has caught up with a scroll to it. */
export function targetRendered(listbox: HTMLElement, target: number): boolean {
	return [...listbox.querySelectorAll('[role="option"]')].some(
		(row) => Number(row.getAttribute('aria-posinset')) === target + 1,
	);
}

/** Jumps to random rows and times how long until every row in view has its data. */
export async function jumps(times: number) {
	const list = findList();
	const ms: number[] = [];
	let seed = 12345;
	for (let i = 0; i < times; i++) {
		seed = (seed * 1103515245 + 12345) & 0x7fffffff;
		const target = seed % Math.max(1, list.rowCount - 40);
		await raf();
		const started = performance.now();
		list.scroller.scrollTop = target * list.rowHeight;
		for (let tries = 0; tries < 600; tries++) {
			await raf();
			// Until the virtualiser re-renders, the rows in the DOM are the old ones, with no placeholders.
			if (targetRendered(list.listbox, target) && placeholders(list) === 0) break;
		}
		ms.push(performance.now() - started);
	}
	return { rows: list.rowCount, timeToRowsMs: stats(ms) };
}

/** Times Ctrl+A through to the second painted frame. */
export async function selectAll() {
	const list = findList();
	list.scroller.scrollTop = 0;
	await raf();
	await raf();
	list.listbox.focus();
	const started = performance.now();
	list.listbox.dispatchEvent(
		new KeyboardEvent('keydown', { key: 'a', ctrlKey: true, bubbles: true, cancelable: true }),
	);
	await raf();
	await raf();
	return {
		rows: list.rowCount,
		selectAllMs: round(performance.now() - started),
		selectedInView: list.listbox.querySelectorAll('[aria-selected="true"]').length,
	};
}

/** Clicks a column header and times how long until the rows in view are filled in again. */
export async function sortBy(column: string) {
	const list = findList();
	const button = [...document.querySelectorAll<HTMLButtonElement>('button')].find((b) =>
		(b.getAttribute('aria-label') ?? b.textContent ?? '').trim().startsWith(column),
	);
	if (!button) throw new Error(`No "${column}" column header`);
	list.scroller.scrollTop = 0;
	await raf();
	const started = performance.now();
	button.click();
	let slow = true;
	for (let tries = 0; tries < 900; tries++) {
		await raf();
		if (list.listbox.querySelectorAll('[role="option"]').length > 0 && placeholders(list) === 0) {
			slow = false;
			break;
		}
	}
	return { column, visibleMs: round(performance.now() - started), timedOut: slow };
}

/** Everything in sequence, for a folder that is already open. */
export async function runAll() {
	const out: Record<string, unknown> = {
		environment: {
			userAgent: navigator.userAgent,
			dpr: window.devicePixelRatio,
			viewport: [window.innerWidth, window.innerHeight],
		},
	};
	out.sweepFast = await sweep(20000, 2000);
	out.sweepWheel = await sweep(900, 400);
	out.jumps = await jumps(30);
	out.selectAll = await selectAll();
	for (const column of ['Size', 'Modified', 'Name']) out[`sort${column}`] = await sortBy(column);
	return out;
}

/** Switches the open folder to the grid view, through the status bar's view switcher. */
export async function showGrid() {
	const button = [...document.querySelectorAll<HTMLButtonElement>('button')].find(
		(b) => (b.getAttribute('aria-label') ?? '').trim() === 'Grid',
	);
	if (!button) throw new Error('No Grid view button');
	button.click();
	await new Promise((resolve) => setTimeout(resolve, 1500));
}

/** The grid's share of the budgets: scrolling at wheel and fling speed, then select-all. */
export async function runGrid() {
	await showGrid();
	const out: Record<string, unknown> = {};
	out.gridSweepFast = await sweep(20000, 2000);
	out.gridSweepWheel = await sweep(900, 400);
	out.gridSelectAll = await selectAll();
	return out;
}

/** Opens `path` in the active tab and waits for its listing to fill in. */
export async function openFolder(path: string): Promise<void> {
	const { tabsApi } = await import('../services/tabsApi');
	// The window opens its first tab a moment after start-up, so wait for it.
	let snapshot = await tabsApi.getSnapshot();
	for (let tries = 0; snapshot.active === null && tries < 100; tries++) {
		await new Promise((resolve) => setTimeout(resolve, 100));
		snapshot = await tabsApi.getSnapshot();
	}
	if (snapshot.active === null) throw new Error('The window has no active tab.');
	const current = snapshot.tabs.find((tab) => tab.id === snapshot.active)?.location;
	// Rust parses the path, so `#`, `?` and Windows paths become the URI the provider expects.
	const { createTauriVfsClient } = await import('../services/tauriVfsClient');
	const target = await createTauriVfsClient().parseLocation(
		path,
		current ?? { display: '/', uri: 'file:///' },
	);
	const before = document.querySelector('[role="listbox"]');
	await tabsApi.navigate(snapshot.active, target);
	for (let tries = 0; tries < 600; tries++) {
		await new Promise((resolve) => setTimeout(resolve, 100));
		const listbox = document.querySelector('[role="listbox"]');
		const count = Number(listbox?.getAttribute('aria-rowcount') ?? 0);
		// A new folder gets a new list element; staying put needs no wait for one.
		const arrived = current?.uri === target.uri || listbox !== before;
		if (arrived && count > 1000) return;
	}
	throw new Error(`${path} did not open as a large listing.`);
}

/** Extra passes `autoRun` can add: maximise the window first, and measure the grid view as well. */
export interface AutoOptions {
	maximise?: boolean;
	grid?: boolean;
}

/** Opens a folder, runs everything and logs the result as one line. */
export async function autoRun(path: string, options: AutoOptions = {}): Promise<void> {
	try {
		if (options.maximise) {
			const { getCurrentWindow } = await import('@tauri-apps/api/window');
			await getCurrentWindow().maximize();
			await new Promise((resolve) => setTimeout(resolve, 1000));
		}
		await openFolder(path);
		await new Promise((resolve) => setTimeout(resolve, 1500));
		const result = await runAll();
		if (options.grid) Object.assign(result, await runGrid());
		console.log(`PERF_RESULT ${JSON.stringify(result)}`);
	} catch (error) {
		console.log(`PERF_RESULT ${JSON.stringify({ error: String(error) })}`);
	}
}

declare global {
	interface Window {
		__waypointPerf?: unknown;
	}
}

/** Makes the harness callable from the webview console and the Tauri MCP bridge. */
export function installPerfHarness(): void {
	window.__waypointPerf = { sweep, jumps, selectAll, sortBy, runAll, runGrid, stats, autoRun };
	// `#perf-auto=/path`, optionally followed by `&max` (maximise first) and `&grid` (measure the grid too).
	const requested = /^#perf-auto=([^&]+)((?:&[a-z]+)*)$/.exec(window.location.hash);
	if (!requested) return;
	try {
		const flags = requested[2]!.split('&');
		void autoRun(decodeURIComponent(requested[1]!), {
			maximise: flags.includes('max'),
			grid: flags.includes('grid'),
		});
	} catch (error) {
		console.warn('the perf-auto path in the URL hash is not valid', error);
	}
}
