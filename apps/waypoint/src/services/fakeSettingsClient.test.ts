// Verifies the fake settings client keeps Rust's contract, so tests that use it prove something
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createFakeSettingsClient } from './fakeSettingsClient';
import { DEFAULT_SETTINGS, type Settings } from './settingsClient';

const grid: Settings = {
	...DEFAULT_SETTINGS,
	general: { ...DEFAULT_SETTINGS.general, defaultView: 'grid' },
};

describe('createFakeSettingsClient', () => {
	it('starts at revision 0 and raises it by one per accepted change, announcing each', async () => {
		const fake = createFakeSettingsClient();
		const heard = vi.fn();
		fake.onChanged(heard);
		expect((await fake.snapshot()).revision).toBe(0);
		expect((await fake.set(grid)).revision).toBe(1);
		expect(heard).toHaveBeenCalledWith({ revision: 1, settings: grid });
		// What is already in force is quiet.
		expect((await fake.set(grid)).revision).toBe(1);
		expect(heard).toHaveBeenCalledTimes(1);
		expect(fake.calls).toHaveLength(2);
	});

	it('refuses a delay out of range as Rust does, naming the field and range', async () => {
		const fake = createFakeSettingsClient();
		const bad = { ...DEFAULT_SETTINGS, dnd: { ...DEFAULT_SETTINGS.dnd, springLoadMs: 5 } };
		await expect(fake.set(bad)).rejects.toMatchObject({
			kind: 'invalid',
			field: 'dnd.springLoadMs',
			min: 200,
			max: 2000,
		});
		expect(fake.current().revision).toBe(0);
	});

	it('fails the next save on request and then recovers', async () => {
		const fake = createFakeSettingsClient();
		fake.failNext({ kind: 'storage', message: 'the disk is full' });
		await expect(fake.set(grid)).rejects.toMatchObject({ kind: 'storage' });
		expect((await fake.set(grid)).revision).toBe(1);
	});

	it('stops announcing to a listener that unsubscribed', () => {
		const fake = createFakeSettingsClient();
		const heard = vi.fn();
		const stop = fake.onChanged(heard);
		stop();
		fake.change(grid);
		expect(heard).not.toHaveBeenCalled();
	});
});
