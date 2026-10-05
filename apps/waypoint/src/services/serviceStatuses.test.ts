// Verifies the plugins' own statuses become the shared shape the Services panel reads
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	collectServiceStatuses,
	integrationServiceStatus,
	mimeAppsServiceStatus,
	secretsServiceStatus,
	volumesServiceStatus,
	nativeDndServiceStatus,
	protocolServiceStatus,
	SERVICE_SOURCES,
	systemAppearanceServiceStatus,
	trashServiceStatus,
	windowEffectsServiceStatus,
} from './serviceStatuses';

const plugins = vi.hoisted(() => ({
	trash: vi.fn(),
	dnd: vi.fn(),
	ops: vi.fn(),
	vfs: vi.fn(),
	windowManager: vi.fn(),
	effects: vi.fn(),
	appearance: vi.fn(),
	integrations: vi.fn(),
	mimeApps: vi.fn(),
	secrets: vi.fn(),
	volumes: vi.fn(),
	support: vi.fn(),
}));
vi.mock('@liminal-hq/plugin-volumes', () => ({ getStatus: plugins.volumes }));
vi.mock('@liminal-hq/plugin-secrets', () => ({ getStatus: plugins.secrets }));
vi.mock('@liminal-hq/plugin-mime-apps', () => ({ getStatus: plugins.mimeApps }));
vi.mock('@liminal-hq/plugin-trash', () => ({ getStatus: plugins.trash }));
vi.mock('@liminal-hq/plugin-native-dnd', () => ({ getStatus: plugins.dnd }));
vi.mock('@liminal-hq/plugin-system-appearance', () => ({ getStatus: plugins.appearance }));
vi.mock('@liminal-hq/plugin-window-effects', () => ({ getStatus: plugins.effects }));
vi.mock('@liminal-hq/waypoint-plugin-ops', () => ({ getStatus: plugins.ops }));
vi.mock('@liminal-hq/waypoint-plugin-vfs', () => ({
	getStatus: plugins.vfs,
	connectionSupport: plugins.support,
}));
vi.mock('@liminal-hq/plugin-window-manager', () => ({ getStatus: plugins.windowManager }));
vi.mock('./tauriIntegrationsClient', () => ({
	createTauriIntegrationsClient: () => ({ statuses: plugins.integrations }),
}));

beforeEach(() => {
	for (const fn of Object.values(plugins)) fn.mockReset();
});

const feature = (available: boolean, reason: string | null = null) => ({ available, reason });

const noPalette = {
	available: false,
	source: null,
	reason: 'sourceMissing',
	detail: 'the GTK theme defines none of theme_bg_color',
};
const workingPalette = { available: true, source: 'gtkTheme', reason: null, detail: null };

const appearanceFeature = (name: string, available: boolean, detail: string | null = null) => ({
	feature: name,
	available,
	source: available ? 'portal' : null,
	reason: available ? null : 'noSource',
	detail,
});

describe('the system appearance status', () => {
	it('is available through the appearance features when the titlebar has no source', async () => {
		plugins.appearance.mockResolvedValue({
			available: false,
			reason: 'no portal',
			features: [],
			appearanceAvailable: true,
			appearance: [
				appearanceFeature('colourScheme', true),
				appearanceFeature('contrast', false, 'no contrast setting'),
			],
			palette: noPalette,
		});
		expect(await systemAppearanceServiceStatus()).toEqual({
			available: true,
			reason: 'no portal',
			features: ['colourScheme'],
		});
	});

	it('lists the titlebar sources and the working appearance features, and gives the first reason', async () => {
		plugins.appearance.mockResolvedValue({
			available: true,
			reason: null,
			features: ['portal'],
			appearanceAvailable: true,
			appearance: [
				appearanceFeature('colourScheme', true),
				appearanceFeature('reducedTransparency', false, 'the portal has no such setting'),
			],
			palette: noPalette,
		});
		expect(await systemAppearanceServiceStatus()).toEqual({
			available: true,
			reason: 'the portal has no such setting',
			features: ['portal', 'colourScheme'],
		});
	});

	it('lists the palette among the working features, which "Match the system\'s colours" needs', async () => {
		plugins.appearance.mockResolvedValue({
			available: true,
			reason: null,
			features: ['portal'],
			appearanceAvailable: true,
			appearance: [appearanceFeature('colourScheme', true)],
			palette: workingPalette,
		});
		expect(await systemAppearanceServiceStatus()).toEqual({
			available: true,
			reason: null,
			features: ['portal', 'colourScheme', 'palette'],
		});
	});

	it('says why the palette is missing when it is the only thing that is', async () => {
		plugins.appearance.mockResolvedValue({
			available: true,
			reason: null,
			features: ['portal'],
			appearanceAvailable: true,
			appearance: [appearanceFeature('colourScheme', true)],
			palette: noPalette,
		});
		expect(await systemAppearanceServiceStatus()).toEqual({
			available: true,
			reason: 'the GTK theme defines none of theme_bg_color',
			features: ['portal', 'colourScheme'],
		});
	});

	it('is available through the palette alone', async () => {
		plugins.appearance.mockResolvedValue({
			available: false,
			reason: 'no portal',
			features: [],
			appearanceAvailable: false,
			appearance: [appearanceFeature('colourScheme', false, 'no source')],
			palette: workingPalette,
		});
		expect((await systemAppearanceServiceStatus()).available).toBe(true);
	});

	it('is unavailable when nothing works', async () => {
		plugins.appearance.mockResolvedValue({
			available: false,
			reason: 'no portal',
			features: [],
			appearanceAvailable: false,
			appearance: [appearanceFeature('colourScheme', false, 'no source')],
			palette: noPalette,
		});
		expect(await systemAppearanceServiceStatus()).toEqual({
			available: false,
			reason: 'no portal',
			features: [],
		});
	});
});

describe('the Trash status', () => {
	it('lists the features that work and has no reason when everything does', async () => {
		plugins.trash.mockResolvedValue({
			available: true,
			reason: null,
			flavour: 'freedesktop',
			features: [
				{ name: 'trash', ...feature(true) },
				{ name: 'list', ...feature(true) },
				{ name: 'restore', ...feature(true) },
			],
		});
		expect(await trashServiceStatus()).toEqual({
			available: true,
			reason: null,
			features: ['trash', 'list', 'restore'],
		});
	});

	it('stays available inside a sandbox, without the features it lacks, and says why', async () => {
		const only = 'the Trash portal can only move files to the trash';
		plugins.trash.mockResolvedValue({
			available: true,
			reason: null,
			flavour: 'portal',
			features: [
				{ name: 'trash', ...feature(true) },
				{ name: 'list', ...feature(false, only) },
				{ name: 'restore', ...feature(false, only) },
				{ name: 'empty', ...feature(false, only) },
			],
		});
		expect(await trashServiceStatus()).toEqual({
			available: true,
			reason: only,
			features: ['trash'],
		});
	});

	it('is unavailable with the plugin’s own reason where nothing works', async () => {
		plugins.trash.mockResolvedValue({
			available: false,
			reason: 'no trash on this system',
			flavour: 'unsupported',
			features: [{ name: 'trash', ...feature(false, 'no trash on this system') }],
		});
		expect(await trashServiceStatus()).toEqual({
			available: false,
			reason: 'no trash on this system',
			features: [],
		});
	});
});

describe('the window effects status', () => {
	const entry = (
		name: string,
		available: boolean,
		reason: string | null,
		message: string | null,
	) => ({
		name,
		available,
		reason,
		message,
	});

	it('explains a missing blur and says nothing of the features that belong to another platform', async () => {
		const why = 'GNOME does not let apps blur behind their windows';
		plugins.effects.mockResolvedValue({
			available: true,
			reason: 'compositor-has-no-blur',
			message: why,
			flavour: 'wayland',
			features: [
				entry('opacity', true, null, null),
				entry('blur', false, 'compositor-has-no-blur', why),
				entry('mica', false, 'windows-only', 'Mica is a Windows 11 material'),
				entry('shadowInset', true, null, null),
			],
		});
		expect(await windowEffectsServiceStatus()).toEqual({
			available: true,
			reason: why,
			features: ['opacity', 'shadowInset'],
		});
	});

	it('has no reason on Windows 11, where the shadow inset is not a fault', async () => {
		plugins.effects.mockResolvedValue({
			available: true,
			reason: null,
			message: null,
			flavour: 'windows',
			features: [
				entry('opacity', true, null, null),
				entry('mica', true, null, null),
				entry('shadowInset', false, 'gtk-only', 'the shadow inset is a GTK feature'),
			],
		});
		expect(await windowEffectsServiceStatus()).toEqual({
			available: true,
			reason: null,
			features: ['opacity', 'mica'],
		});
	});
});

describe('the native drag and drop status', () => {
	it('reads its feature map', async () => {
		plugins.dnd.mockResolvedValue({
			available: true,
			reason: null,
			displayServer: 'wayland',
			features: {
				'outbound-drag': feature(true),
				clipboard: feature(false, 'no clipboard portal'),
			},
		});
		expect(await nativeDndServiceStatus()).toEqual({
			available: true,
			reason: 'no clipboard portal',
			features: ['outbound-drag'],
		});
	});
});

describe('the file types status', () => {
	const mimeFeature = (name: string, available: boolean, message: string | null = null) => ({
		name,
		available,
		reason: available ? null : 'no-icon-theme',
		message,
	});

	it('lists the file and folder icon features that work, and says why the others do not', async () => {
		plugins.mimeApps.mockResolvedValue({
			available: true,
			reason: 'no-icon-theme',
			message: 'no icon theme is installed besides the fallback one',
			flavour: 'gio',
			associationFiles: [],
			features: [
				mimeFeature('typeInfo', true),
				mimeFeature('typeIcons', false, 'no icon theme is installed besides the fallback one'),
				mimeFeature('folderIcons', false, 'no icon theme is installed besides the fallback one'),
			],
		});
		expect(await mimeAppsServiceStatus()).toEqual({
			available: true,
			reason: 'no icon theme is installed besides the fallback one',
			features: ['typeInfo'],
		});
	});

	it('lists them as working on a desktop with an icon theme', async () => {
		plugins.mimeApps.mockResolvedValue({
			available: true,
			reason: null,
			message: null,
			flavour: 'gio',
			associationFiles: [],
			features: [
				mimeFeature('typeInfo', true),
				mimeFeature('typeIcons', true),
				mimeFeature('folderIcons', true),
			],
		});
		const status = await mimeAppsServiceStatus();
		expect(status.features).toEqual(['typeInfo', 'typeIcons', 'folderIcons']);
		expect(status.reason).toBeNull();
	});
});

describe('the keyring plugin', () => {
	const feature = (name: string, message: string | null) => ({
		name,
		available: message === null,
		reason: message === null ? null : 'no-keyring',
		message,
	});

	it('lists the features that work and says why none does when no keyring runs', async () => {
		const message = 'no keyring is running';
		plugins.secrets.mockResolvedValue({
			available: false,
			reason: 'no-keyring',
			message,
			flavour: 'unsupported',
			features: ['store', 'fetch', 'delete'].map((name) => feature(name, message)),
		});
		expect(await secretsServiceStatus()).toEqual({
			available: false,
			reason: message,
			features: [],
		});
	});

	it('lists every feature when a keyring answers', async () => {
		plugins.secrets.mockResolvedValue({
			available: true,
			reason: null,
			message: null,
			flavour: 'secret-service',
			features: ['store', 'fetch', 'delete'].map((name) => feature(name, null)),
		});
		expect(await secretsServiceStatus()).toEqual({
			available: true,
			reason: null,
			features: ['store', 'fetch', 'delete'],
		});
	});
});

describe('the Services panel sources', () => {
	it('reports trash, native-dnd and waypoint-ops, and one that cannot answer does not hide the others', async () => {
		expect(Object.keys(SERVICE_SOURCES)).toEqual([
			'file-system',
			'trash',
			'native-dnd',
			'window-manager',
			'waypoint-ops',
			'waypoint-session',
			'waypoint-git',
			'waypoint-settings',
			'os-prefs',
			'system-appearance',
			'window-tearoff',
			'thumbnails',
			'volumes',
			'secrets',
			'window-effects',
			'mime-apps',
			'sftp',
			'smb',
			'webdav',
			's3',
			'xdg-portal',
			'desktop-integration',
		]);
		plugins.vfs.mockResolvedValue({ available: true, reason: null, features: ['listing'] });
		plugins.trash.mockRejectedValue(new Error('permission denied'));
		plugins.dnd.mockResolvedValue({
			available: false,
			reason: 'no drag and drop here',
			displayServer: 'x11',
			features: {},
		});
		plugins.ops.mockResolvedValue({ available: true, reason: null, features: ['queue', 'trash'] });
		plugins.windowManager.mockResolvedValue({
			available: true,
			reason: null,
			features: ['system-window-menu'],
		});
		const statuses = await collectServiceStatuses();
		expect(statuses['trash']).toEqual({
			available: false,
			reason: 'permission denied',
			features: [],
		});
		expect(statuses['native-dnd']?.available).toBe(false);
		expect(statuses['waypoint-ops']?.features).toEqual(['queue', 'trash']);
		expect(statuses['file-system']?.features).toEqual(['listing']);
		// The Shelf window's always-on-top is among the features the window manager reports (or lacks).
		expect(statuses['window-manager']?.features).toEqual(['system-window-menu']);
	});
});

describe('the remote protocols', () => {
	const support = (schemes: string[], off: string[]) =>
		plugins.support.mockResolvedValue({ schemes, off, keyring: null });

	it('are available while their provider is registered', async () => {
		support(['sftp', 'dav', 'davs'], ['smb']);
		expect(await protocolServiceStatus(['sftp'])()).toEqual({
			available: true,
			reason: null,
			features: ['sftp'],
		});
		expect(await protocolServiceStatus(['dav', 'davs'])()).toMatchObject({ available: true });
	});

	it('say they are turned off in Settings → Experimental while their switch is off', async () => {
		support([], ['sftp', 'smb']);
		expect(await protocolServiceStatus(['smb'])()).toEqual({
			available: false,
			reason: 'Turned off in Settings → Experimental',
			features: [],
		});
	});

	it('say a protocol the build does not have is not included, and each has its own line in the panel', async () => {
		support([], ['sftp']);
		expect(await protocolServiceStatus(['s3'])()).toEqual({
			available: false,
			reason: 'Not included in this build',
			features: [],
		});
		const all = await collectServiceStatuses();
		expect(all['sftp']?.reason).toBe('Turned off in Settings → Experimental');
		expect(all['s3']?.reason).toBe('Not included in this build');
		expect(Object.keys(all)).toEqual(expect.arrayContaining(['sftp', 'smb', 'webdav', 's3']));
	});
});

describe('the Rust-only plugins', () => {
	const portal = { available: false, reason: 'No desktop portal is running.', features: [] };
	const desktop = { available: true, reason: null, features: ['notify'] };

	it('are in the panel and report through the app command, asked once for both', async () => {
		plugins.integrations.mockResolvedValue({
			'xdg-portal': portal,
			'desktop-integration': desktop,
		});
		const sources = {
			'xdg-portal': SERVICE_SOURCES['xdg-portal']!,
			'desktop-integration': SERVICE_SOURCES['desktop-integration']!,
		};
		const [a, b] = await Promise.all([sources['xdg-portal'](), sources['desktop-integration']()]);
		expect(a).toEqual(portal);
		expect(b).toEqual(desktop);
		expect(plugins.integrations).toHaveBeenCalledTimes(1);
	});

	it('are unavailable when the command does not report one, and the panel survives a failed command', async () => {
		plugins.integrations.mockResolvedValue({});
		expect(await integrationServiceStatus('xdg-portal')()).toEqual({
			available: false,
			reason: null,
			features: [],
		});
		plugins.integrations.mockRejectedValue(new Error('no command'));
		const all = await collectServiceStatuses();
		expect(all['xdg-portal']).toMatchObject({ available: false, reason: 'no command' });
		expect(all['desktop-integration']).toMatchObject({ available: false, reason: 'no command' });
	});
});

describe('the volumes plugin', () => {
	const feature = (name: string, reason: string | null) => ({
		name,
		available: reason === null,
		reason,
		message: reason === null ? null : `${name}: ${reason}`,
	});
	const status = (remember: string | null) => ({
		available: true,
		reason: null,
		message: null,
		flavour: 'udisks2',
		features: [feature('list', null), feature('unlock', null), feature('remember', remember)],
	});

	it('does not show remembering as a fault while it is off or not offered', async () => {
		for (const reason of ['disabled', 'not-configured']) {
			plugins.volumes.mockResolvedValue(status(reason));
			expect(await volumesServiceStatus()).toEqual({
				available: true,
				reason: null,
				features: ['list', 'unlock'],
			});
		}
	});

	it('gives the keyring as the reason remembering is not working when it is on', async () => {
		plugins.volumes.mockResolvedValue(status('no-keyring'));
		const result = await volumesServiceStatus();
		expect(result.reason).toBe('remember: no-keyring');
		expect(result.features).toEqual(['list', 'unlock']);
	});
});
