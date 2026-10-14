// Verifies when a folder nothing watches is read again: after a job, on focus and on Ctrl+R
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { createOpsStore } from '../ops/opsStore';
import { request } from '../test/opsHarness';
import { startRefreshAfterJobs, startRefreshOnFocus, startRefreshShortcut } from './refreshRules';

describe('after a job', () => {
	async function setup() {
		const fake = createFakeOpsClient();
		const handle = createOpsStore(fake);
		await handle.ready;
		const wrote = vi.fn();
		const stop = startRefreshAfterJobs(handle, wrote);
		return { fake, wrote, stop };
	}

	it('is called once for each job that finishes, whichever way it ends', async () => {
		const { fake, wrote } = await setup();
		const done = await fake.submit(request(['a']));
		fake.start(done);
		expect(wrote).not.toHaveBeenCalled();
		fake.done(done, 'Copy');
		expect(wrote).toHaveBeenCalledTimes(1);
		const failed = await fake.submit(request(['b']));
		fake.start(failed);
		fake.fail(failed, { kind: 'cancelled' });
		expect(wrote).toHaveBeenCalledTimes(2);
	});

	it('does not count the jobs that had finished before it started, and stops when told', async () => {
		const fake = createFakeOpsClient();
		const early = await fake.submit(request(['a']));
		fake.start(early);
		fake.done(early, 'Copy');
		const handle = createOpsStore(fake);
		await handle.ready;
		const wrote = vi.fn();
		const stop = startRefreshAfterJobs(handle, wrote);
		expect(wrote).not.toHaveBeenCalled();
		stop();
		const later = await fake.submit(request(['b']));
		fake.start(later);
		fake.done(later, 'Copy');
		expect(wrote).not.toHaveBeenCalled();
	});
});

describe('on focus, F5 and Ctrl+R', () => {
	it('calls back when the window is focused or becomes visible, and not after it is stopped', () => {
		const onFocus = vi.fn();
		const stop = startRefreshOnFocus(window, onFocus);
		window.dispatchEvent(new Event('focus'));
		expect(onFocus).toHaveBeenCalledTimes(1);
		document.dispatchEvent(new Event('visibilitychange'));
		expect(onFocus).toHaveBeenCalledTimes(2);
		stop();
		window.dispatchEvent(new Event('focus'));
		expect(onFocus).toHaveBeenCalledTimes(2);
	});

	it('F5 and Ctrl+R refresh and keep the webview from reloading; other keys, Shift+F5 and Ctrl+F5 do not', () => {
		const onRefresh = vi.fn();
		const stop = startRefreshShortcut(window, onRefresh);
		const press = (init: KeyboardEventInit) => {
			const event = new KeyboardEvent('keydown', { cancelable: true, ...init });
			window.dispatchEvent(event);
			return event;
		};
		expect(press({ key: 'r', ctrlKey: true }).defaultPrevented).toBe(true);
		expect(onRefresh).toHaveBeenCalledTimes(1);
		expect(press({ key: 'F5' }).defaultPrevented).toBe(true);
		expect(onRefresh).toHaveBeenCalledTimes(2);
		press({ key: 'r' });
		press({ key: 'r', ctrlKey: true, shiftKey: true });
		press({ key: 'x', ctrlKey: true });
		press({ key: 'F5', shiftKey: true });
		press({ key: 'F5', ctrlKey: true });
		press({ key: 'F5', ctrlKey: true, shiftKey: true });
		expect(onRefresh).toHaveBeenCalledTimes(2);
		stop();
	});

	it('F5 in a text field keeps the webview from reloading but leaves the field alone', () => {
		const onRefresh = vi.fn();
		const stop = startRefreshShortcut(window, onRefresh);
		const field = document.createElement('input');
		document.body.append(field);
		const event = new KeyboardEvent('keydown', { key: 'F5', cancelable: true, bubbles: true });
		field.dispatchEvent(event);
		expect(event.defaultPrevented).toBe(true);
		expect(onRefresh).not.toHaveBeenCalled();
		field.remove();
		stop();
	});
});
