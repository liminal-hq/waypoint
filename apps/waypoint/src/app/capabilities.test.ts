// Guards the window capabilities: every routed window label is covered, and no wildcard grants access to unknown labels
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';

interface Capability {
	identifier: string;
	windows: string[];
	permissions: string[];
}

// Vite inlines the capability files, so the test needs no file-system access from the DOM environment.
const files = import.meta.glob<string>('../../src-tauri/capabilities/*.json', {
	query: '?raw',
	import: 'default',
	eager: true,
});
// The settings plugin's default permission set, which every capability that names `waypoint-settings:default` gets.
const settingsDefaults = import.meta.glob<string>(
	'../../../../plugins/waypoint-settings/permissions/default.toml',
	{ query: '?raw', import: 'default', eager: true },
);
const capabilities: Capability[] = Object.values(files).map(
	(source) => JSON.parse(source) as Capability,
);

const byId = (identifier: string) => {
	const found = capabilities.find((capability) => capability.identifier === identifier);
	if (!found) throw new Error(`missing capability ${identifier}`);
	return found;
};

const matches = (pattern: string, label: string) =>
	new RegExp(`^${pattern.replace(/[.+?^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*')}$`).test(label);
const covers = (capability: Capability, label: string) =>
	capability.windows.some((pattern) => matches(pattern, label));

// One label for each kind `main.tsx` routes to a screen with the shared title bar.
const ROUTED_LABELS = ['main-1', 'settings', 'properties-42', 'ops', 'shelf', 'tear-ghost'];

describe('window capabilities', () => {
	it('lets the main windows save the Action bar choices (the ui part only) and each folder’s remembered view, and no other command of the settings plugin beyond reading', () => {
		const main = byId('main').permissions.filter((permission) =>
			permission.startsWith('waypoint-settings:'),
		);
		expect(main.sort()).toEqual([
			'waypoint-settings:allow-get-folder-views',
			'waypoint-settings:allow-get-settings',
			'waypoint-settings:allow-get-status',
			'waypoint-settings:allow-remember-folder-view',
			'waypoint-settings:allow-reset-folder-view',
			'waypoint-settings:allow-set-ui-settings',
		]);
	});

	it('lets only the Settings window export and import the settings, and gives no window a dialog or file system permission', () => {
		const transfer = [
			'waypoint-settings:allow-export-settings',
			'waypoint-settings:allow-plan-settings-import',
			'waypoint-settings:allow-apply-settings-import',
		];
		for (const permission of transfer) {
			const holders = capabilities
				.filter((capability) => capability.permissions.includes(permission))
				.map((capability) => capability.identifier);
			expect(holders, permission).toEqual(['settings']);
		}
		// The default set (which the Settings window and others take whole) must not carry them.
		const defaults = Object.values(settingsDefaults).join('\n');
		expect(defaults).toContain('allow-get-settings');
		expect(defaults).not.toMatch(/export|import/);
		// The file dialogs open from Rust; a page that could ask for one, or for a file, could name a path.
		for (const capability of capabilities) {
			expect(
				capability.permissions.filter((p) => /^(dialog|fs):/.test(p)),
				capability.identifier,
			).toEqual([]);
		}
		expect(byId('settings').windows).toEqual(['settings']);
	});

	it('grants nothing to every window through a bare wildcard', () => {
		for (const capability of capabilities) {
			expect(capability.windows, capability.identifier).not.toContain('*');
		}
	});

	it('gives every routed window the chrome and logging permissions', () => {
		for (const identifier of ['window-chrome', 'logging']) {
			const capability = byId(identifier);
			for (const label of ROUTED_LABELS) {
				expect(covers(capability, label), `${identifier} covers ${label}`).toBe(true);
			}
		}
	});

	it('does not extend the chrome permissions to a label that is not routed', () => {
		expect(covers(byId('window-chrome'), 'mystery')).toBe(false);
		expect(covers(byId('window-chrome'), 'devtools')).toBe(false);
	});

	it('lets every routed window control itself', () => {
		const permissions = byId('window-chrome').permissions;
		for (const permission of [
			'core:window:allow-minimize',
			'core:window:allow-toggle-maximize',
			'core:window:allow-close',
			'core:window:allow-start-dragging',
			'core:window:allow-set-always-on-top',
		]) {
			expect(permissions).toContain(permission);
		}
	});

	it('gives the tear-off plugin to the main windows and the ghost only', () => {
		const capability = byId('tear-off');
		expect(capability.permissions).toEqual(['window-tearoff:default']);
		expect(covers(capability, 'main-1')).toBe(true);
		expect(covers(capability, 'main-7')).toBe(true);
		expect(covers(capability, 'tear-ghost')).toBe(true);
		for (const label of ['settings', 'properties-42', 'ops', 'shelf', 'mystery']) {
			expect(covers(capability, label), label).toBe(false);
		}
	});

	it('gives the Shelf window what its panel needs and no more: the queue and native drag and drop for drags out, and the settings to read', () => {
		for (const identifier of ['ops', 'native-dnd', 'shelf']) {
			expect(covers(byId(identifier), 'shelf'), identifier).toBe(true);
		}
		expect(byId('shelf').permissions).toEqual(['waypoint-settings:allow-get-settings']);
		// It is not a main window: no trash, no platform reads, and it may not change settings.
		expect(covers(byId('main'), 'shelf')).toBe(false);
		for (const capability of capabilities) {
			if (!covers(capability, 'shelf')) continue;
			expect(
				capability.permissions.filter((p) => /set-(ui-)?settings|set_settings/.test(p)),
				capability.identifier,
			).toEqual([]);
		}
	});

	it('lets the Properties windows read which application opens a file, the clock setting and the settings, and nothing that opens one, changes a default or changes a setting', () => {
		const capability = byId('properties');
		expect(capability.permissions).toEqual([
			'mime-apps:allow-get-status',
			'mime-apps:allow-handlers',
			'os-prefs:allow-get-time-format',
			'waypoint-settings:allow-get-settings',
		]);
		expect(covers(capability, 'properties-3')).toBe(true);
		for (const label of ['main-1', 'settings', 'ops', 'tear-ghost', 'mystery']) {
			expect(covers(capability, label), label).toBe(false);
		}
		for (const capability of capabilities) {
			if (!covers(capability, 'properties-3')) continue;
			expect(
				capability.permissions.filter((p) => /set-(ui-)?settings|set_settings/.test(p)),
				capability.identifier,
			).toEqual([]);
		}
	});

	it('lets only the Settings window change the default application, and nothing that opens or chooses a file', () => {
		const withDefaults = capabilities.filter((capability) =>
			capability.permissions.some(
				(p) => p === 'mime-apps:allow-set-default' || p === 'mime-apps:default',
			),
		);
		// The main windows hold the plugin's default set; the Settings window the few it needs.
		expect(withDefaults.map((c) => c.identifier).sort()).toEqual(['main', 'settings']);
		expect(byId('settings').permissions.filter((p) => p.startsWith('mime-apps:'))).toEqual([
			'mime-apps:allow-get-status',
			'mime-apps:allow-handlers',
			'mime-apps:allow-set-default',
			'mime-apps:allow-open-default-apps-settings',
		]);
	});

	it('lets every window that draws file icons read whether the system can supply them and refresh them, and nothing that opens a file', () => {
		const capability = byId('system-icons');
		expect(capability.permissions).toEqual([
			'mime-apps:allow-get-status',
			'mime-apps:allow-refresh-type-icons',
		]);
		// The main windows hold the plugin's default set, which includes both.
		expect(byId('main').permissions).toContain('mime-apps:default');
		for (const label of ['settings', 'properties-3', 'shelf']) {
			expect(covers(capability, label), label).toBe(true);
		}
		for (const label of ['ops', 'tear-ghost', 'mystery']) {
			expect(covers(capability, label), label).toBe(false);
		}
	});

	it('lets the Settings window read the status of every plugin the Services panel reports, and nothing more of the ones it is given only for that', () => {
		const settings = byId('settings');
		// The Services panel runs in the Settings window and asks each plugin for its status; a plugin
		// the window may not call is reported "not allowed by ACL" instead of what it can do.
		for (const plugin of [
			'os-prefs',
			'trash',
			'volumes',
			'secrets',
			'window-tearoff',
			'waypoint-ops',
		]) {
			expect(settings.permissions, plugin).toContain(`${plugin}:allow-get-status`);
		}
		for (const plugin of ['os-prefs', 'trash', 'volumes', 'secrets', 'window-tearoff']) {
			expect(
				settings.permissions.filter((permission) => permission.startsWith(`${plugin}:`)),
				plugin,
			).toEqual([`${plugin}:allow-get-status`]);
		}
	});

	it('lets no window read a secret back, and gives the keyring plugin to the main and Settings windows for its status only', () => {
		for (const capability of capabilities) {
			expect(
				capability.permissions.filter(
					(p) => p === 'secrets:allow-fetch' || p === 'secrets:default',
				),
				capability.identifier,
			).toEqual([]);
		}
		expect(
			capabilities
				.filter((capability) => capability.permissions.some((p) => p.startsWith('secrets:')))
				.map((capability) => capability.identifier),
		).toEqual(['main', 'settings']);
		// The main windows log the keyring's status at start-up; without it the log says "not allowed by ACL".
		for (const id of ['main', 'settings']) {
			expect(
				byId(id).permissions.filter((p) => p.startsWith('secrets:')),
				id,
			).toEqual(['secrets:allow-get-status']);
		}
	});
});
