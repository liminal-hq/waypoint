// Verifies the plugin's events reach the file drag: this window's only, files only, the keys where they are known, and the hand-off to the end of an outbound drag
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it } from 'vitest';
import { FakeNativeDndClient, unavailableNativeDnd } from '../services/fakeNativeDndClient';
import type { NativeDndAvailability } from '../services/nativeDndClient';
import {
	disposeHarnesses,
	mark,
	MUSIC,
	nativeHarness,
	settle,
	type NativeHarness,
} from '../test/nativeDragHarness';
import { connectNativeDnd } from './nativeDndHost';

afterEach(() => {
	disposeHarnesses();
	document.body.innerHTML = '';
	document.documentElement.removeAttribute('style');
});

const FILES = ['file:///srv/share/with%20space.txt', 'file:///srv/share/bad%FF%FE.txt'];

async function connect(
	client: FakeNativeDndClient,
	h: NativeHarness,
	label: string | null = 'main-1',
) {
	const seen: NativeDndAvailability[] = [];
	const stop = connectNativeDnd({
		client,
		drag: h.drag,
		windowLabel: () => label,
		onAvailability: (found) => seen.push(found),
	});
	await settle();
	return { stop, seen };
}

describe('files dragged in', () => {
	it('runs enter, over and drop through the file drag, with the keys the events carry', async () => {
		const h = await nativeHarness();
		const client = new FakeNativeDndClient();
		await connect(client, h);
		client.enter(
			FILES,
			{ x: 100, y: 100 },
			{ paths: ['/srv/share/with space.txt', '/srv/share/bad�.txt'] },
		);
		expect(h.phase()).toBe('dragging');
		expect(h.pill()).toBe('Dragging 2 items');
		h.over(mark('place', MUSIC.uri, 'Music'));
		client.over({ x: 140, y: 100 }, { modifiers: { ctrl: true } });
		expect(h.pill()).toBe('Copy 2 items to Music');
		client.drop(FILES, { x: 140, y: 100 }, { modifiers: { ctrl: true } });
		await settle();
		expect(h.transfers).toEqual([
			{
				kind: 'copy',
				items: [
					{ display: '/srv/share/with space.txt', uri: FILES[0] },
					{ display: '/srv/share/bad�.txt', uri: FILES[1] },
				],
				destination: expect.objectContaining({ uri: MUSIC.uri }),
			},
		]);
	});

	it('clears the highlight when the files leave', async () => {
		const h = await nativeHarness();
		const client = new FakeNativeDndClient();
		await connect(client, h);
		const row = mark('place', MUSIC.uri, 'Music');
		h.over(row);
		client.enter(FILES, { x: 100, y: 100 });
		client.over({ x: 140, y: 100 });
		await settle();
		expect(row.getAttribute('data-drop-over')).toBe('ok');
		client.leave();
		expect(row.hasAttribute('data-drop-over')).toBe(false);
		expect(h.phase()).not.toBe('dragging');
	});

	it('begins the drag at a drop that was not entered first', async () => {
		const h = await nativeHarness();
		const client = new FakeNativeDndClient();
		await connect(client, h);
		h.over(mark('place', MUSIC.uri, 'Music'));
		client.drop(FILES, { x: 140, y: 100 }, { modifiers: { ctrl: true } });
		await settle();
		expect(h.transfers).toHaveLength(1);
	});

	it('ignores events for another window', async () => {
		const h = await nativeHarness();
		const client = new FakeNativeDndClient();
		await connect(client, h);
		client.enter(FILES, { x: 1, y: 1 }, { window: 'main-2' });
		expect(h.phase()).toBe('idle');
		client.enter(FILES, { x: 1, y: 1 });
		client.leave({ window: 'main-2' });
		expect(h.phase()).toBe('dragging');
	});

	it('ignores drags that are not of files: a link or text dragged from a browser', async () => {
		const h = await nativeHarness();
		const client = new FakeNativeDndClient();
		await connect(client, h);
		client.enter(['https://example.com/page'], { x: 1, y: 1 });
		expect(h.phase()).toBe('idle');
		client.drop(['https://example.com/page'], { x: 1, y: 1 });
		await settle();
		expect(h.transfers).toEqual([]);
		// A mixed drag acts on the files in it.
		client.enter(['https://example.com/page', FILES[0]!], { x: 1, y: 1 });
		expect(h.phase()).toBe('dragging');
		expect(h.pill()).toBe('Dragging with space.txt');
	});

	it('reads the keys as released where the platform does not report them (Wayland)', async () => {
		const h = await nativeHarness();
		h.state.sameVolume = false;
		const client = new FakeNativeDndClient({
			availability: { modifiers: false, displayServer: 'wayland' },
		});
		await connect(client, h);
		h.over(mark('place', MUSIC.uri, 'Music'));
		client.enter(FILES, { x: 100, y: 100 });
		// Even if an event carried Shift, it is not believed.
		client.over({ x: 140, y: 100 }, { modifiers: { shift: true } });
		await settle();
		h.clock.advance(150);
		await settle();
		expect(h.pill()).toBe('Copy 2 items to Music');
		client.drop(FILES, { x: 140, y: 100 }, { modifiers: { shift: true } });
		await settle();
		expect(h.transfers[0]?.kind).toBe('copy');
	});

	it('listens to nothing inbound where the plugin says it does not work, and says why', async () => {
		const h = await nativeHarness();
		const client = unavailableNativeDnd('there is no display');
		const { seen } = await connect(client, h);
		expect(client.listenerCount()).toBe(0);
		expect(seen[0]).toMatchObject({
			inbound: false,
			outbound: false,
			reasons: { inbound: 'there is no display' },
		});
	});

	it('stops listening, and ends a drag in progress, when it is disconnected', async () => {
		const h = await nativeHarness();
		const client = new FakeNativeDndClient();
		const { stop } = await connect(client, h);
		expect(client.listenerCount()).toBe(5);
		client.enter(FILES, { x: 1, y: 1 });
		stop();
		expect(client.listenerCount()).toBe(0);
		expect(h.phase()).not.toBe('dragging');
	});

	it('does not listen at all if it is disconnected before the plugin has answered', async () => {
		const h = await nativeHarness();
		const client = new FakeNativeDndClient();
		const stop = connectNativeDnd({ client, drag: h.drag, windowLabel: () => 'main-1' });
		stop();
		await settle();
		expect(client.listenerCount()).toBe(0);
	});
});

describe('the end of an outbound drag', () => {
	it('reaches the drag, which announces it', async () => {
		const h = await nativeHarness();
		const client = new FakeNativeDndClient();
		h.state.start = (request) => client.startDrag(request);
		await connect(client, h);
		h.startDrag();
		h.move(-5);
		await settle();
		expect(client.started[0]?.uris).toEqual(['file:///home/test/notes.txt']);
		client.endDrag('dropped-move');
		expect(h.announced.at(-1)).toBe('Moved notes.txt to another application');
	});

	it('is not listened to where outbound drags do not work', async () => {
		const h = await nativeHarness();
		const client = new FakeNativeDndClient({ availability: { outbound: false } });
		await connect(client, h);
		expect(client.listenerCount()).toBe(4);
	});
});
