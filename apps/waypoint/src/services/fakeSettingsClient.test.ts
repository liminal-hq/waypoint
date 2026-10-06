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

describe('the milestone 5 sections', () => {
	it('start at the documented defaults, every integration off', async () => {
		const { settings } = await createFakeSettingsClient().snapshot();
		expect(settings.appearance).toEqual({
			mode: 'system',
			themeSource: 'liminal',
			accent: { kind: 'ember' },
			density: 'comfortable',
			iconStyle: 'regular',
			iconTheme: 'waypoint',
			folderColour: 'liminal',
			matchSystemColours: false,
		});
		expect(settings.transparency.enabled).toBe(false);
		expect(settings.accessibility.textSize).toBe(100);
		expect(settings.locale.language).toBe('system');
		expect(settings.integrations).toEqual({
			notifications: false,
			notificationActions: true,
			launcherProgress: false,
			preventSleep: false,
			rememberVolumePassphrases: false,
			defaultFileManager: false,
			globalShortcutEnabled: false,
			globalShortcut: null,
		});
		expect(settings.experimental).toEqual({
			sftp: false,
			smb: false,
			webdav: false,
			s3: false,
			nativeContextMenus: false,
		});
	});

	it('refuses what Rust refuses, naming the field', async () => {
		const fake = createFakeSettingsClient();
		const bad: [Settings, string][] = [
			[
				{ ...DEFAULT_SETTINGS, transparency: { ...DEFAULT_SETTINGS.transparency, opacity: 39 } },
				'transparency.opacity',
			],
			...(['rowsOpacity', 'sidebarOpacity', 'contentOpacity'] as const).map(
				(key): [Settings, string] => [
					{ ...DEFAULT_SETTINGS, transparency: { ...DEFAULT_SETTINGS.transparency, [key]: 39 } },
					`transparency.${key}`,
				],
			),
			[
				{
					...DEFAULT_SETTINGS,
					transparency: { ...DEFAULT_SETTINGS.transparency, menuOpacity: 59 },
				},
				'transparency.menuOpacity',
			],
			[
				{ ...DEFAULT_SETTINGS, previews: { ...DEFAULT_SETTINGS.previews, maxFileMb: 0 } },
				'previews.maxFileMb',
			],
			[
				{
					...DEFAULT_SETTINGS,
					accessibility: { ...DEFAULT_SETTINGS.accessibility, textSize: 110 },
				},
				'accessibility.textSize',
			],
			[
				{
					...DEFAULT_SETTINGS,
					appearance: { ...DEFAULT_SETTINGS.appearance, accent: { kind: 'custom', hex: 'orange' } },
				},
				'appearance.accent',
			],
			[
				{ ...DEFAULT_SETTINGS, locale: { ...DEFAULT_SETTINGS.locale, language: 'de-DE' } },
				'locale.language',
			],
			[
				{
					...DEFAULT_SETTINGS,
					integrations: { ...DEFAULT_SETTINGS.integrations, globalShortcut: ' ' },
				},
				'integrations.globalShortcut',
			],
		];
		for (const [settings, field] of bad) {
			await expect(fake.set(settings)).rejects.toMatchObject({ kind: 'invalid', field });
		}
		expect((await fake.snapshot()).revision).toBe(0);
	});

	it('accepts the edges of each range and the offered choices', async () => {
		const fake = createFakeSettingsClient();
		const ok: Settings = {
			...DEFAULT_SETTINGS,
			appearance: { ...DEFAULT_SETTINGS.appearance, accent: { kind: 'custom', hex: '#f97316' } },
			transparency: {
				...DEFAULT_SETTINGS.transparency,
				enabled: true,
				opacity: 40,
				rowsOpacity: 40,
				sidebarOpacity: 100,
				contentOpacity: 40,
				menuOpacity: 60,
			},
			accessibility: { ...DEFAULT_SETTINGS.accessibility, textSize: 130 },
			locale: { language: 'fr-CA', direction: 'ltr' },
			previews: { ...DEFAULT_SETTINGS.previews, maxFileMb: 2048 },
			integrations: { ...DEFAULT_SETTINGS.integrations, globalShortcut: 'Ctrl+Alt+W' },
		};
		expect((await fake.set(ok)).revision).toBe(1);
	});
});
