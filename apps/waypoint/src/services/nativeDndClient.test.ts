// Verifies the native drag and drop status becomes feature availability with reasons, and the fake client scripts the plugin's events and answers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PluginStatus } from '@liminal-hq/plugin-native-dnd';
import { describe, expect, it, vi } from 'vitest';
import { FakeNativeDndClient } from './fakeNativeDndClient';
import { availabilityOf, nativeDndErrorKind } from './nativeDndClient';

const on = { available: true, reason: null };
const off = (reason: string) => ({ available: false, reason });

const status = (features: Partial<PluginStatus['features']>): PluginStatus => ({
	available: true,
	reason: null,
	displayServer: 'wayland',
	features: {
		inbound: on,
		outbound: on,
		positions: on,
		modifiers: off('the compositor keeps the keyboard during a drag'),
		clipboard: on,
		'self-drop-filter': on,
		...features,
	},
});

describe('availabilityOf', () => {
	it('maps the features the file drag decides by, with the reasons for the ones that do not work', () => {
		expect(availabilityOf(status({}))).toEqual({
			inbound: true,
			outbound: true,
			modifiers: false,
			clipboard: true,
			reasons: { modifiers: 'the compositor keeps the keyboard during a drag' },
			displayServer: 'wayland',
		});
	});

	it('reports everything off, with the reason, where the plugin has no display', () => {
		const none = off('no display server');
		expect(
			availabilityOf({
				...status({ inbound: none, outbound: none, modifiers: none, clipboard: none }),
				available: false,
				displayServer: 'none',
			}),
		).toMatchObject({
			inbound: false,
			outbound: false,
			modifiers: false,
			clipboard: false,
			reasons: {
				inbound: 'no display server',
				outbound: 'no display server',
				modifiers: 'no display server',
				clipboard: 'no display server',
			},
			displayServer: 'none',
		});
	});
});

describe('nativeDndErrorKind', () => {
	it('reads the plugin\u2019s error kind, and nothing from other failures', () => {
		expect(nativeDndErrorKind({ kind: 'buttonNotPressed', message: 'x' })).toBe('buttonNotPressed');
		expect(nativeDndErrorKind(new Error('x'))).toBeNull();
		expect(nativeDndErrorKind(null)).toBeNull();
	});
});

describe('FakeNativeDndClient', () => {
	it('says what is available, with the keys unavailable as on Wayland', async () => {
		const fake = new FakeNativeDndClient({ availability: { modifiers: false } });
		expect(await fake.status()).toMatchObject({ inbound: true, outbound: true, modifiers: false });
	});

	it('delivers scripted events in the plugin\u2019s shapes, and the keys as released where they are unavailable', () => {
		const fake = new FakeNativeDndClient({ availability: { modifiers: false } });
		const seen: unknown[] = [];
		fake.onEnter((event) => seen.push(event));
		fake.onOver((event) => seen.push(event));
		fake.onDrop((event) => seen.push(event));
		fake.onLeave((event) => seen.push(event));
		fake.enter(['file:///a/b%20c'], { x: 1, y: 2 }, { modifiers: { ctrl: true } });
		fake.over({ x: 3, y: 4 });
		fake.drop(['file:///a/b%20c'], { x: 3, y: 4 }, { selfDrop: true });
		fake.leave();
		expect(seen).toEqual([
			{
				window: 'main-1',
				uris: ['file:///a/b%20c'],
				paths: ['/a/b c'],
				position: { x: 1, y: 2 },
				modifiers: { ctrl: false, shift: false, alt: false },
				action: null,
			},
			{
				window: 'main-1',
				position: { x: 3, y: 4 },
				modifiers: { ctrl: false, shift: false, alt: false },
				action: null,
			},
			{
				window: 'main-1',
				uris: ['file:///a/b%20c'],
				paths: ['/a/b c'],
				position: { x: 3, y: 4 },
				modifiers: { ctrl: false, shift: false, alt: false },
				action: null,
				selfDrop: true,
			},
			{ window: 'main-1' },
		]);
	});

	it('records an outbound drag, ends it, and refuses the way the plugin does', async () => {
		const fake = new FakeNativeDndClient();
		const ended = vi.fn();
		fake.onDragEnded(ended);
		const started = await fake.startDrag({ uris: ['file:///a'], actions: ['copy'] });
		expect(started).toEqual({ id: 1, ended: null });
		fake.endDrag('dropped-copy');
		expect(ended).toHaveBeenCalledWith({
			id: 1,
			outcome: 'dropped-copy',
			uris: ['file:///a'],
			reason: null,
		});
		fake.refuseStart = 'buttonNotPressed';
		await expect(fake.startDrag({ uris: ['file:///a'], actions: ['copy'] })).rejects.toMatchObject({
			kind: 'buttonNotPressed',
		});
		const none = new FakeNativeDndClient({ availability: { outbound: false } });
		await expect(none.startDrag({ uris: ['file:///a'], actions: ['copy'] })).rejects.toMatchObject({
			kind: 'unsupported',
		});
	});

	it('answers a modal drag only when it ends (Windows)', async () => {
		const fake = new FakeNativeDndClient({ modal: true });
		let answer: unknown = null;
		void fake
			.startDrag({ uris: ['file:///a'], actions: ['copy'] })
			.then((result) => (answer = result));
		await Promise.resolve();
		expect(answer).toBeNull();
		fake.endDrag('cancelled');
		await Promise.resolve();
		expect(answer).toMatchObject({ id: 1, ended: { outcome: 'cancelled' } });
	});
});
