// THROWAWAY milestone 0 spike: scripted measurements, driven from outside through window.__spike
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ROW_HEIGHT, type ListHandle } from './ListView';
import { rangeBinary, rangeJson, sortListing } from './ipc';

const raf = () => new Promise<number>((resolve) => requestAnimationFrame(resolve));
const round = (n: number) => Math.round(n * 100) / 100;

export function stats(values: number[]) {
	if (values.length === 0) return { n: 0 };
	const sorted = [...values].sort((a, b) => a - b);
	const at = (p: number) => sorted[Math.min(sorted.length - 1, Math.floor((p / 100) * sorted.length))]!;
	return {
		n: values.length,
		p50: round(at(50)),
		p95: round(at(95)),
		p99: round(at(99)),
		max: round(sorted[sorted.length - 1]!),
	};
}

/** Scrolls at a steady rate and records frame time and how often visible rows had no data yet. */
export async function sweep(list: ListHandle, pxPerFrame: number, maxFrames: number, fromPx = 0) {
	const el = list.container!;
	el.scrollTop = fromPx;
	await raf();
	await raf();
	let last = await raf();
	const limit = el.scrollHeight - el.clientHeight;
	const frames: number[] = [];
	let blankFrames = 0;
	let blankRows = 0;
	for (let i = 0; i < maxFrames && el.scrollTop < limit; i++) {
		el.scrollTop = Math.min(limit, el.scrollTop + pxPerFrame);
		const now = await raf();
		frames.push(now - last);
		last = now;
		const blank = list.blankVisible();
		if (blank > 0) {
			blankFrames++;
			blankRows += blank;
		}
	}
	return {
		pxPerFrame,
		rowsPerFrame: Math.round(pxPerFrame / ROW_HEIGHT),
		frameMs: stats(frames),
		over20ms: frames.filter((f) => f > 20).length,
		over33ms: frames.filter((f) => f > 33).length,
		blankFrames,
		blankRows,
		scrolledPx: el.scrollTop - fromPx,
		scrollHeight: el.scrollHeight,
		fetchMs: stats(list.fetches),
	};
}

/** Jumps to random rows and times how long until every visible row has data. */
export async function jumps(list: ListHandle, count: number, times: number) {
	const el = list.container!;
	const ms: number[] = [];
	let seed = 12345;
	for (let i = 0; i < times; i++) {
		seed = (seed * 1103515245 + 12345) & 0x7fffffff;
		const target = seed % Math.max(1, count - 40);
		await raf();
		const started = performance.now();
		el.scrollTop = target * ROW_HEIGHT;
		for (let tries = 0; tries < 600; tries++) {
			await raf();
			const [first, last] = list.visibleRange();
			if (first <= target + 5 && last >= target && list.blankVisible() === 0) break;
		}
		ms.push(performance.now() - started);
	}
	return { timeToRowsMs: stats(ms) };
}

/** Times select-all (a keyboard shortcut in the real app) through to the next painted frame. */
export async function selection(list: ListHandle, count: number) {
	const el = list.container!;
	el.scrollTop = 0;
	await raf();
	await raf();
	const started = performance.now();
	list.selectAll();
	await raf();
	await raf();
	const selectAllMs = performance.now() - started;
	const row = el.querySelector<HTMLElement>('[data-row="5"]');
	const clickStarted = performance.now();
	row?.dispatchEvent(new MouseEvent('click', { bubbles: true }));
	await raf();
	await raf();
	const clickMs = performance.now() - clickStarted;
	const far = el.querySelector<HTMLElement>('[data-row="15"]');
	const shiftStarted = performance.now();
	far?.dispatchEvent(new MouseEvent('click', { bubbles: true, shiftKey: true }));
	await raf();
	await raf();
	return {
		rows: count,
		selectAllMs: round(selectAllMs),
		clickMs: round(clickMs),
		shiftClickMs: round(performance.now() - shiftStarted),
	};
}

/** Sorts in Rust, then times how long until the visible rows show the new order. */
export async function sorts(list: ListHandle, handle: number) {
	const out: Record<string, { rustMs: number; visibleMs: number }> = {};
	for (const by of ['name', 'size', 'modified']) {
		list.container!.scrollTop = 0;
		await raf();
		const started = performance.now();
		const rustMs = await sortListing(handle, by);
		list.invalidate();
		for (let tries = 0; tries < 600; tries++) {
			await raf();
			if (list.blankVisible() === 0) break;
		}
		out[by] = { rustMs: round(rustMs), visibleMs: round(performance.now() - started) };
	}
	return out;
}

/** IPC round-trip time by page size, JSON against raw bytes. */
export async function ipc(handle: number, count: number) {
	const out: Record<string, unknown> = {};
	for (const rows of [50, 256, 1000, 5000]) {
		for (const [name, fetch] of [
			['json', rangeJson],
			['bin', rangeBinary],
		] as const) {
			const times: number[] = [];
			for (let i = 0; i < 40; i++) {
				const start = (i * 7919 * rows) % Math.max(1, count - rows);
				const started = performance.now();
				await fetch(handle, start, rows);
				times.push(performance.now() - started);
			}
			out[`${name}-${rows}`] = stats(times);
		}
	}
	return out;
}
