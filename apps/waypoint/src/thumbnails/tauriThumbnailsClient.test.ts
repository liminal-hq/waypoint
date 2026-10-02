// Tests for the real ThumbnailsClient with Tauri's invoke and Channel mocked: only ids, locations and keys go to Rust
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createTauriThumbnailsClient } from './tauriThumbnailsClient';

const tauri = vi.hoisted(() => {
	class FakeChannel {
		onmessage: (message: unknown) => void = () => {};
	}
	return { invoke: vi.fn(), Channel: FakeChannel };
});
vi.mock('@tauri-apps/api/core', () => tauri);
vi.mock('@liminal-hq/plugin-thumbnails', () => ({ getStatus: vi.fn() }));

beforeEach(() => {
	tauri.invoke.mockReset();
	tauri.invoke.mockResolvedValue(7);
});

describe('createTauriThumbnailsClient', () => {
	it('requests entries by listing handle and id, and delivers the channel’s events', async () => {
		const received: unknown[] = [];
		const ticket = await createTauriThumbnailsClient().requestEntries(
			3,
			[{ key: '5:0', id: 5 }],
			'large',
			(event) => received.push(event),
		);
		expect(ticket).toBe(7);
		const [command, args] = tauri.invoke.mock.calls[0]!;
		expect(command).toBe('thumbnails_request_entries');
		expect(args).toMatchObject({ handle: 3, items: [{ key: '5:0', id: 5 }], size: 'large' });
		expect(JSON.stringify(args.items)).not.toMatch(/path/);
		args.onEvent.onmessage({ kind: 'ready', key: '5:0', url: 'thumb://localhost/large/a.png' });
		expect(received).toEqual([{ kind: 'ready', key: '5:0', url: 'thumb://localhost/large/a.png' }]);
	});

	it('requests locations, prioritises and cancels by ticket', async () => {
		const client = createTauriThumbnailsClient();
		const location = { display: '/a.png', uri: 'file:///a.png' };
		await client.requestLocations([{ key: 'file:///a.png', location }], 'normal', () => {});
		expect(tauri.invoke.mock.calls[0]![0]).toBe('thumbnails_request_locations');
		expect(tauri.invoke.mock.calls[0]![1]).toMatchObject({
			items: [{ key: 'file:///a.png', location }],
			size: 'normal',
		});
		await client.prioritise(7, ['a']);
		expect(tauri.invoke).toHaveBeenCalledWith('thumbnails_prioritise', { ticket: 7, keys: ['a'] });
		await client.cancel(7);
		expect(tauri.invoke).toHaveBeenCalledWith('thumbnails_cancel', { ticket: 7 });
	});
});
