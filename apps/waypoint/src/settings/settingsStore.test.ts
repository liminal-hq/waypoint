// Verifies the settings store: the first snapshot, revision gating, own saves, refusals and disposal
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS, type Settings, type SettingsClient } from '../services/settingsClient';
import { createSettingsStore } from './settingsStore';

const withView = (defaultView: 'list' | 'grid'): Settings => ({
	...DEFAULT_SETTINGS,
	general: { ...DEFAULT_SETTINGS.general, defaultView },
});

describe('createSettingsStore', () => {
	it('starts on the defaults and takes Rust’s answer, even at revision 0', async () => {
		const fake = createFakeSettingsClient(withView('grid'));
		const handle = createSettingsStore(fake);
		expect(handle.store.getState()).toMatchObject({ settings: DEFAULT_SETTINGS, ready: false });
		await handle.ready;
		expect(handle.store.getState()).toEqual({
			settings: withView('grid'),
			revision: 0,
			ready: true,
		});
	});

	it('keeps the defaults when the first read fails', async () => {
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		const fake = createFakeSettingsClient();
		fake.snapshot = () => Promise.reject(new Error('no plugin'));
		const handle = createSettingsStore(fake);
		await handle.ready;
		expect(handle.store.getState().ready).toBe(false);
		expect(handle.store.getState().settings).toEqual(DEFAULT_SETTINGS);
		warn.mockRestore();
	});

	it('applies a change another window made, and only a newer one', async () => {
		const fake = createFakeSettingsClient();
		const handle = createSettingsStore(fake);
		await handle.ready;
		fake.change(withView('grid'));
		expect(handle.store.getState()).toMatchObject({ revision: 1, settings: withView('grid') });
		// A late event for an older revision, and a repeat of the current one, change nothing.
		fake.emit({ revision: 0, settings: DEFAULT_SETTINGS });
		fake.emit({ revision: 1, settings: DEFAULT_SETTINGS });
		expect(handle.store.getState().settings).toEqual(withView('grid'));
		fake.emit({ revision: 5, settings: DEFAULT_SETTINGS });
		expect(handle.store.getState()).toMatchObject({ revision: 5, settings: DEFAULT_SETTINGS });
	});

	it('uses an event that arrives before the snapshot, and ignores the older snapshot after it', async () => {
		const fake = createFakeSettingsClient();
		const gate = fake.holdSnapshot();
		const handle = createSettingsStore(fake);
		fake.emit({ revision: 2, settings: withView('grid') });
		expect(handle.store.getState()).toMatchObject({ revision: 2, ready: true });
		gate.release();
		await handle.ready;
		expect(handle.store.getState()).toMatchObject({ revision: 2, settings: withView('grid') });
	});

	it('shows what Rust answers to its own save, once, and not before', async () => {
		const fake = createFakeSettingsClient();
		const handle = createSettingsStore(fake);
		await handle.ready;
		const seen: number[] = [];
		handle.store.subscribe((state) => seen.push(state.revision));
		const answer = await handle.save(withView('grid'));
		expect(answer.revision).toBe(1);
		// The event and the answer are the same revision: one change reaches subscribers.
		expect(seen).toEqual([1]);
	});

	it('leaves the store as it was when Rust refuses a save', async () => {
		const fake = createFakeSettingsClient();
		const handle = createSettingsStore(fake);
		await handle.ready;
		fake.failNext({ kind: 'storage', message: 'the disk is full' });
		await expect(handle.save(withView('grid'))).rejects.toMatchObject({ kind: 'storage' });
		expect(handle.store.getState()).toMatchObject({ revision: 0, settings: DEFAULT_SETTINGS });
	});

	it('stops following once disposed', async () => {
		const fake = createFakeSettingsClient();
		const handle = createSettingsStore(fake);
		await handle.ready;
		handle.dispose();
		fake.change(withView('grid'));
		expect(handle.store.getState().revision).toBe(0);
	});

	it('subscribes before it reads, so no change falls between the two', async () => {
		const order: string[] = [];
		const client: SettingsClient = {
			snapshot: async () => {
				order.push('snapshot');
				return { revision: 0, settings: DEFAULT_SETTINGS };
			},
			set: async () => ({ revision: 0, settings: DEFAULT_SETTINGS }),
			setUi: async () => ({ revision: 0, settings: DEFAULT_SETTINGS }),
			onChanged: () => {
				order.push('subscribe');
				return () => {};
			},
		};
		await createSettingsStore(client).ready;
		expect(order).toEqual(['subscribe', 'snapshot']);
	});
});
