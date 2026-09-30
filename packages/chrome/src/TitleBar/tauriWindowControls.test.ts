// Verifies the Tauri adapter signals when its listeners are live and lets only the latest maximised read report
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';

const win = vi.hoisted(() => ({
	onResized: vi.fn(),
	onFocusChanged: vi.fn(),
	isMaximized: vi.fn(),
}));

vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => win }));

import { createTauriWindowControls } from './tauriWindowControls';

describe('createTauriWindowControls subscriptions', () => {
	it('reports ready only once the asynchronous registration has finished', async () => {
		let finishRegistering: (off: () => void) => void = () => {};
		win.onFocusChanged.mockImplementation(
			() => new Promise<() => void>((resolve) => (finishRegistering = resolve)),
		);
		const controls = createTauriWindowControls();
		const unsubscribe = controls.onFocusChange!(() => {});

		let ready = false;
		void unsubscribe.ready!.then(() => (ready = true));
		await Promise.resolve();
		expect(ready).toBe(false);

		finishRegistering(() => {});
		await unsubscribe.ready;
		expect(ready).toBe(true);
	});

	it('removes a listener that finishes registering after it was cancelled', async () => {
		const off = vi.fn();
		let finishRegistering: (off: () => void) => void = () => {};
		win.onResized.mockImplementation(
			() => new Promise<() => void>((resolve) => (finishRegistering = resolve)),
		);
		const unsubscribe = createTauriWindowControls().onMaximizedChange(() => {});
		unsubscribe();
		finishRegistering(off);
		await unsubscribe.ready;
		expect(off).toHaveBeenCalledTimes(1);
	});

	it('lets only the latest resize read report, so an older read cannot overwrite it', async () => {
		let onResize: () => Promise<void> = async () => {};
		win.onResized.mockImplementation(async (handler: () => Promise<void>) => {
			onResize = handler;
			return () => {};
		});
		const finish: Array<(value: boolean) => void> = [];
		win.isMaximized.mockImplementation(
			() => new Promise<boolean>((resolve) => finish.push(resolve)),
		);
		const seen: boolean[] = [];
		const unsubscribe = createTauriWindowControls().onMaximizedChange((value) => seen.push(value));
		await unsubscribe.ready;

		const first = onResize();
		const second = onResize();
		finish[1]!(true); // the newer read resolves first
		finish[0]!(false); // the older one resolves late
		await Promise.all([first, second]);
		expect(seen).toEqual([true]);
	});
});
