// Verifies the real OsClipboardClient reaches the native-dnd plugin, reads its availability once and reports a missing feature as none
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';

const plugin = vi.hoisted(() => ({
	getStatus: vi.fn(),
	hasFeature: vi.fn(),
	setFiles: vi.fn(),
	getFiles: vi.fn(),
	onClipboardChanged: vi.fn(),
}));
vi.mock('@liminal-hq/plugin-native-dnd', () => plugin);

import { createTauriOsClipboardClient } from './tauriOsClipboardClient';

beforeEach(() => {
	for (const fn of Object.values(plugin)) fn.mockReset();
});

describe('createTauriOsClipboardClient', () => {
	it('reports the clipboard feature of the plugin’s status, once', async () => {
		plugin.getStatus.mockResolvedValue({ features: {} });
		plugin.hasFeature.mockReturnValue(true);
		const client = createTauriOsClipboardClient();
		expect(await client.available()).toBe(true);
		expect(await client.available()).toBe(true);
		expect(plugin.getStatus).toHaveBeenCalledTimes(1);
		expect(plugin.hasFeature).toHaveBeenCalledWith({ features: {} }, 'clipboard');
	});

	it('reports a feature the system lacks, and a status that cannot be read, as unavailable', async () => {
		plugin.getStatus.mockResolvedValue({ features: {} });
		plugin.hasFeature.mockReturnValue(false);
		expect(await createTauriOsClipboardClient().available()).toBe(false);
		plugin.getStatus.mockRejectedValue(new Error('no plugin'));
		expect(await createTauriOsClipboardClient().available()).toBe(false);
	});

	it('passes the files through in both directions', async () => {
		const client = createTauriOsClipboardClient();
		const files = { uris: ['file:///a'], cut: true };
		plugin.setFiles.mockResolvedValue(undefined);
		plugin.getFiles.mockResolvedValue(files);
		await client.setFiles(files);
		expect(plugin.setFiles).toHaveBeenCalledWith(files);
		expect(await client.getFiles()).toBe(files);
		plugin.getFiles.mockResolvedValue(null);
		expect(await client.getFiles()).toBeNull();
	});

	it('listens for the clipboard changing and stops, even when stopped before the listener is ready', async () => {
		const unlisten = vi.fn();
		let ready: (stop: () => void) => void = () => {};
		plugin.onClipboardChanged.mockReturnValue(
			new Promise<() => void>((resolve) => {
				ready = resolve;
			}),
		);
		const client = createTauriOsClipboardClient();
		const stop = client.onChange(() => {});
		stop();
		ready(unlisten);
		await Promise.resolve();
		await Promise.resolve();
		expect(unlisten).toHaveBeenCalledTimes(1);
		plugin.onClipboardChanged.mockResolvedValue(unlisten);
		const later = client.onChange(() => {});
		await Promise.resolve();
		later();
		expect(unlisten).toHaveBeenCalledTimes(2);
	});
});
