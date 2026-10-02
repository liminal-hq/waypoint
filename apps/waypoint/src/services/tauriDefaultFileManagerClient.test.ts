// Verifies the real DefaultFileManagerClient over a mocked mime-apps plugin: the action each system allows, who is default, and making Waypoint so
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DIRECTORY_TYPE, WAYPOINT_DESKTOP_ID } from './defaultFileManagerClient';
import { createTauriDefaultFileManagerClient } from './tauriDefaultFileManagerClient';

const plugin = vi.hoisted(() => ({
	getStatus: vi.fn(),
	handlers: vi.fn(),
	setDefault: vi.fn(),
	openDefaultAppsSettings: vi.fn(),
}));
vi.mock('@liminal-hq/plugin-mime-apps', () => ({
	...plugin,
	hasFeature: (status: { features: Array<{ name: string; available: boolean }> }, name: string) =>
		status.features.some((f) => f.name === name && f.available),
	featureReason: (
		status: { features: Array<{ name: string; reason: string | null }> },
		name: string,
	) => status.features.find((f) => f.name === name)?.reason ?? undefined,
	featureMessage: (
		status: { features: Array<{ name: string; message: string | null }> },
		name: string,
	) => status.features.find((f) => f.name === name)?.message ?? undefined,
	isMimeAppsError: (value: unknown) =>
		typeof value === 'object' && value !== null && 'kind' in value,
}));

const status = (
	setDefault: { available: boolean; reason?: string; message?: string } = { available: true },
) => ({
	features: [
		{
			name: 'setDefault',
			available: setDefault.available,
			reason: setDefault.reason ?? null,
			message: setDefault.message ?? null,
		},
	],
});

beforeEach(() => {
	for (const fn of Object.values(plugin)) fn.mockReset();
});

describe('the action', () => {
	it('is `set` where the system lets an application change the default', async () => {
		plugin.getStatus.mockResolvedValue(status());
		expect(await createTauriDefaultFileManagerClient().action()).toEqual({ kind: 'set' });
	});

	it('is `settings` where only the person can, which is Windows', async () => {
		plugin.getStatus.mockResolvedValue(status({ available: false, reason: 'managed-by-system' }));
		expect(await createTauriDefaultFileManagerClient().action()).toEqual({ kind: 'settings' });
	});

	it('is unavailable with the plugin’s sentence elsewhere, such as a Flatpak sandbox', async () => {
		plugin.getStatus.mockResolvedValue(
			status({ available: false, reason: 'flatpak-sandbox', message: 'A sandbox cannot do it.' }),
		);
		expect(await createTauriDefaultFileManagerClient().action()).toEqual({
			kind: 'unavailable',
			reason: 'A sandbox cannot do it.',
		});
	});
});

describe('who is default', () => {
	it('is Waypoint when the default for folders is its desktop file', async () => {
		plugin.handlers.mockResolvedValue({
			mime: DIRECTORY_TYPE,
			default: { id: WAYPOINT_DESKTOP_ID, name: 'Waypoint' },
		});
		expect(await createTauriDefaultFileManagerClient().current()).toEqual({
			isWaypoint: true,
			name: null,
		});
	});

	it('names the application that is, when it is not Waypoint', async () => {
		plugin.handlers.mockResolvedValue({
			mime: DIRECTORY_TYPE,
			default: { id: 'org.gnome.Nautilus.desktop', name: 'Files' },
		});
		expect(await createTauriDefaultFileManagerClient().current()).toEqual({
			isWaypoint: false,
			name: 'Files',
		});
	});

	it('is unknown when the type is not folders or the plugin cannot say', async () => {
		plugin.handlers.mockResolvedValue({ mime: 'text/plain', default: null });
		expect(await createTauriDefaultFileManagerClient().current()).toBeNull();
		plugin.handlers.mockRejectedValue(new Error('no'));
		expect(await createTauriDefaultFileManagerClient().current()).toBeNull();
	});
});

describe('making Waypoint the default', () => {
	it('sets the default for folders to its desktop file where that is allowed', async () => {
		plugin.getStatus.mockResolvedValue(status());
		await createTauriDefaultFileManagerClient().make();
		expect(plugin.setDefault).toHaveBeenCalledWith(DIRECTORY_TYPE, WAYPOINT_DESKTOP_ID);
		expect(plugin.openDefaultAppsSettings).not.toHaveBeenCalled();
	});

	it('opens the Default apps page where only the person can', async () => {
		plugin.getStatus.mockResolvedValue(status({ available: false, reason: 'managed-by-system' }));
		await createTauriDefaultFileManagerClient().make();
		expect(plugin.openDefaultAppsSettings).toHaveBeenCalled();
		expect(plugin.setDefault).not.toHaveBeenCalled();
	});

	it('says there is no desktop file when Waypoint is not installed as an application', async () => {
		plugin.getStatus.mockResolvedValue(status());
		plugin.setDefault.mockRejectedValue({ kind: 'appNotFound' });
		await expect(createTauriDefaultFileManagerClient().make()).rejects.toThrow(
			/not installed as an application/,
		);
	});

	it('passes the plugin’s own sentence for a failure', async () => {
		plugin.getStatus.mockResolvedValue(status());
		plugin.setDefault.mockRejectedValue({ kind: 'failed', message: 'mimeapps.list is read-only' });
		await expect(createTauriDefaultFileManagerClient().make()).rejects.toThrow(
			'mimeapps.list is read-only',
		);
	});
});
