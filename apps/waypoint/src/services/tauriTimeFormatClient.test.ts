// Tests for the real TimeFormatClient with the plugin's guest-js module mocked
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createTauriTimeFormatClient } from './tauriTimeFormatClient';

const plugin = vi.hoisted(() => ({
	getTimeFormat: vi.fn(),
	onTimeFormatChanged: vi.fn(),
	hourCycleOf: (format: { is24Hour: boolean; source: string }) =>
		format.source === 'default' ? undefined : format.is24Hour ? 'h23' : 'h12',
}));
vi.mock('@liminal-hq/plugin-os-prefs', () => plugin);

beforeEach(() => {
	plugin.getTimeFormat.mockReset();
	plugin.onTimeFormatChanged.mockReset();
	vi.spyOn(console, 'warn').mockImplementation(() => {});
});

describe('createTauriTimeFormatClient', () => {
	it('maps the plugin reading to an hour cycle', async () => {
		const client = createTauriTimeFormatClient();
		plugin.getTimeFormat.mockResolvedValue({ is24Hour: true, source: 'gnome-portal' });
		expect(await client.get()).toBe('h23');
		plugin.getTimeFormat.mockResolvedValue({ is24Hour: false, source: 'gnome-portal' });
		expect(await client.get()).toBe('h12');
	});

	it('leaves a guessed reading to Intl', async () => {
		plugin.getTimeFormat.mockResolvedValue({ is24Hour: false, source: 'default' });
		expect(await createTauriTimeFormatClient().get()).toBeUndefined();
	});

	it('resolves undefined and logs once when the plugin errors', async () => {
		const client = createTauriTimeFormatClient();
		plugin.getTimeFormat.mockRejectedValue(new Error('plugin missing'));
		expect(await client.get()).toBeUndefined();
		expect(await client.get()).toBeUndefined();
		expect(console.warn).toHaveBeenCalledTimes(1);
	});

	it('passes change events through and unsubscribes', async () => {
		let deliver: (format: { is24Hour: boolean; source: string }) => void = () => {};
		const stop = vi.fn();
		plugin.onTimeFormatChanged.mockImplementation(async (callback) => {
			deliver = callback;
			return stop;
		});
		const listener = vi.fn();
		const unsubscribe = createTauriTimeFormatClient().onChange(listener);
		await Promise.resolve();
		deliver({ is24Hour: false, source: 'gnome-portal' });
		expect(listener).toHaveBeenCalledWith('h12');
		unsubscribe();
		expect(stop).toHaveBeenCalled();
	});

	it('stops a listener that finishes registering after it was cancelled', async () => {
		const stop = vi.fn();
		plugin.onTimeFormatChanged.mockResolvedValue(stop);
		createTauriTimeFormatClient().onChange(() => {})();
		await Promise.resolve();
		await Promise.resolve();
		expect(stop).toHaveBeenCalled();
	});
});
