// Verifies starting an application and the words for a failure
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createFakeOpenWithClient, fakeApp } from './fakeOpenWithClient';
import { openWithFailureText, startOpenWith } from './startOpenWith';

const uris = ['file:///a.png'];

describe('startOpenWith', () => {
	it('opens in the default application, a chosen one, or the system chooser', async () => {
		const client = createFakeOpenWithClient();
		const say = vi.fn();
		expect(await startOpenWith(client, uris, { kind: 'default' }, say)).toBe(true);
		expect(await startOpenWith(client, uris, { kind: 'app', app: fakeApp('x.desktop') }, say)).toBe(
			true,
		);
		expect(await startOpenWith(client, uris, { kind: 'chooser' }, say)).toBe(true);
		expect(client.calls).toEqual([
			['openDefault', uris],
			['openWith', uris, 'x.desktop'],
			['choose', uris],
		]);
		expect(say).not.toHaveBeenCalled();
	});

	it('says which application failed, and that a type has no handler', async () => {
		const say = vi.fn();
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		const failing = createFakeOpenWithClient({ openError: { kind: 'failed', message: 'boom' } });
		expect(
			await startOpenWith(failing, uris, { kind: 'app', app: fakeApp('x', 'Viewer') }, say),
		).toBe(false);
		expect(say).toHaveBeenLastCalledWith('Could not open the item with Viewer.');
		const none = createFakeOpenWithClient({ openError: { kind: 'noHandler', mime: 'x/y' } });
		await startOpenWith(none, uris, { kind: 'default' }, say);
		expect(say).toHaveBeenLastCalledWith('No application is set to open this type of file.');
		warn.mockRestore();
	});

	it('is silent when the system chooser is dismissed', async () => {
		const say = vi.fn();
		const client = createFakeOpenWithClient({ chooseError: { kind: 'cancelled' } });
		expect(await startOpenWith(client, uris, { kind: 'chooser' }, say)).toBe(false);
		expect(say).not.toHaveBeenCalled();
		expect(openWithFailureText({ kind: 'cancelled' }, null)).toBeNull();
	});
});
