// Verifies the real NativeDndClient reaches the native-dnd plugin, reads its status once, and stops listening even when stopped early
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';

const plugin = vi.hoisted(() => ({
	getStatus: vi.fn(),
	startDrag: vi.fn(),
	onEnter: vi.fn(),
	onOver: vi.fn(),
	onDrop: vi.fn(),
	onLeave: vi.fn(),
	onDragEnded: vi.fn(),
}));
vi.mock('@liminal-hq/plugin-native-dnd', () => plugin);

import { createTauriNativeDndClient } from './tauriNativeDndClient';

beforeEach(() => {
	for (const fn of Object.values(plugin)) fn.mockReset();
});

const on = { available: true, reason: null };

describe('createTauriNativeDndClient', () => {
	it('reads the status once and reports it as availability', async () => {
		plugin.getStatus.mockResolvedValue({
			available: true,
			reason: null,
			displayServer: 'x11',
			features: {
				inbound: on,
				outbound: on,
				positions: on,
				modifiers: on,
				clipboard: { available: false, reason: 'no clipboard' },
				'self-drop-filter': on,
			},
		});
		const client = createTauriNativeDndClient();
		expect(await client.status()).toMatchObject({
			inbound: true,
			outbound: true,
			modifiers: true,
			clipboard: false,
			reasons: { clipboard: 'no clipboard' },
		});
		await client.status();
		expect(plugin.getStatus).toHaveBeenCalledTimes(1);
	});

	it('reports a plugin that cannot answer as offering nothing', async () => {
		plugin.getStatus.mockRejectedValue(new Error('no plugin'));
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		expect(await createTauriNativeDndClient().status()).toMatchObject({
			inbound: false,
			outbound: false,
		});
		warn.mockRestore();
	});

	it('starts a drag with no icon, and passes the plugin\u2019s answer and refusals through', async () => {
		const client = createTauriNativeDndClient();
		plugin.startDrag.mockResolvedValue({ id: 3, ended: null });
		await expect(
			client.startDrag({ uris: ['file:///a'], actions: ['copy', 'move'] }),
		).resolves.toEqual({
			id: 3,
			ended: null,
		});
		expect(plugin.startDrag).toHaveBeenCalledWith({
			uris: ['file:///a'],
			actions: ['copy', 'move'],
			icon: null,
		});
		plugin.startDrag.mockRejectedValue({ kind: 'buttonNotPressed', message: 'up' });
		await expect(
			client.startDrag({ uris: ['file:///a'], actions: ['copy'] }),
		).rejects.toMatchObject({
			kind: 'buttonNotPressed',
		});
	});

	it('listens for each event and stops, even when stopped before the listener is ready', async () => {
		const client = createTauriNativeDndClient();
		for (const [method, listen] of [
			['onEnter', 'onEnter'],
			['onOver', 'onOver'],
			['onDrop', 'onDrop'],
			['onLeave', 'onLeave'],
			['onDragEnded', 'onDragEnded'],
		] as const) {
			const unlisten = vi.fn();
			let ready: (stop: () => void) => void = () => {};
			plugin[listen].mockReturnValue(
				new Promise<() => void>((resolve) => {
					ready = resolve;
				}),
			);
			const handler = vi.fn();
			const stop = client[method](handler);
			expect(plugin[listen]).toHaveBeenCalledWith(handler);
			stop();
			ready(unlisten);
			await Promise.resolve();
			await Promise.resolve();
			expect(unlisten, method).toHaveBeenCalledTimes(1);
		}
	});
});
