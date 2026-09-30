// THROWAWAY milestone 0 spike: runs every measurement in sequence and logs each result
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import * as bench from './bench';
import * as ipc from './ipc';
import type { FetchMode, ListHandle } from './ListView';

const emit = (name: string, data: unknown) =>
	console.log(`SPIKE_RESULT ${JSON.stringify({ name, data })}`);

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** What the webview will actually allow as a scrollable height, against what the rows need. */
function heightLimits() {
	const out: Record<string, { wanted: number; got: number }> = {};
	for (const rows of [100_000, 500_000, 1_000_000, 2_000_000]) {
		const probe = document.createElement('div');
		const wanted = rows * 28;
		probe.style.cssText = `position:absolute;visibility:hidden;width:1px;height:${wanted}px`;
		document.body.appendChild(probe);
		out[`${rows}`] = { wanted, got: probe.getBoundingClientRect().height };
		probe.remove();
	}
	return out;
}

export interface Host {
	load(count: number, mode: FetchMode): Promise<ipc.Created>;
	list(): ListHandle | null;
	handle(): number;
}

async function listSpike(host: Host, count: number, mode: FetchMode) {
	const created = await host.load(count, mode);
	emit(`create-${count}-${mode}`, created);
	const list = host.list()!;
	emit(`sweep-fast-${count}-${mode}`, await bench.sweep(list, 20000, 2000));
	emit(`sweep-wheel-${count}-${mode}`, await bench.sweep(list, 900, 400));
	emit(`jumps-${count}-${mode}`, await bench.jumps(list, count, 30));
	emit(`selection-${count}-${mode}`, await bench.selection(list, count));
	if (mode === 'json') emit(`sorts-${count}`, await bench.sorts(list, host.handle()));
}

async function scanSpike() {
	const dir = '/tmp/claude-1000/spike-scan';
	const count = 500_000;
	try {
		await ipc.removeDir(dir);
	} catch {
		/* not there yet */
	}
	emit('scan-fixture', { files: count, buildMs: await ipc.makeDir(dir, count) });
	emit('scan-baseline-warm-1', await ipc.scanBaseline(dir));
	emit('scan-baseline-warm-2', await ipc.scanBaseline(dir));
	for (const batch of [500, 5000, 50000]) {
		const started = performance.now();
		let firstRows = 0;
		let received = 0;
		let batches = 0;
		const summary = await ipc.scan(dir, batch, (rows) => {
			if (batches === 0) firstRows = performance.now() - started;
			received += rows.length;
			batches++;
		});
		emit(`scan-channel-${batch}`, {
			rust: summary,
			jsFirstBatchMs: Math.round(firstRows * 100) / 100,
			jsTotalMs: Math.round((performance.now() - started) * 100) / 100,
			jsReceived: received,
		});
	}
	// With a watcher live and files being created and removed underneath the scan.
	await ipc.watch(dir);
	const churning = ipc.churn(dir, 2000);
	const started = performance.now();
	let received = 0;
	const summary = await ipc.scan(dir, 5000, (rows) => (received += rows.length));
	const churnMs = await churning;
	await wait(500);
	emit('scan-with-watcher', {
		rust: summary,
		jsTotalMs: Math.round((performance.now() - started) * 100) / 100,
		jsReceived: received,
		churnMs,
		watcherEvents: await ipc.watchEvents(),
	});
	await ipc.unwatch();
	await ipc.removeDir(dir);
}

async function jumpsAndSorts(host: Host) {
	for (const mode of ['json', 'bin'] as const) {
		const created = await host.load(500_000, mode);
		const list = host.list()!;
		emit(`jumps-${mode}`, await bench.jumps(list, created.count, 30));
		if (mode === 'json') emit('sorts', await bench.sorts(list, host.handle()));
	}
	console.log('SPIKE_DONE');
}

export async function runAll(host: Host) {
	if (location.hash === '#spike-auto-max') {
		const { getCurrentWindow } = await import('@tauri-apps/api/window');
		await getCurrentWindow().maximize();
		await wait(1500);
	}
	if (location.hash === '#spike-auto-jumps') return jumpsAndSorts(host);
	emit('environment', {
		userAgent: navigator.userAgent,
		dpr: window.devicePixelRatio,
		viewport: [window.innerWidth, window.innerHeight],
		heightLimits: heightLimits(),
	});
	await wait(500);
	try {
		emit('ipc-bench', await bench.ipc((await host.load(500_000, 'json')).handle, 500_000));
		await listSpike(host, 500_000, 'json');
		await listSpike(host, 500_000, 'bin');
		await listSpike(host, 100_000, 'bin');
		await scanSpike();
	} catch (error) {
		emit('error', String(error));
	}
	console.log('SPIKE_DONE');
}
