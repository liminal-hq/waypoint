// Verifies the real SettingsClient reaches each plugin command and event, and that the capabilities grant exactly what each window uses
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const calls: Array<[string, unknown]> = [];
const handlers = new Map<string, (event: { payload: unknown }) => void>();
const unlisten = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(async (command: string, args?: unknown) => {
		calls.push([command.replace('plugin:waypoint-settings|', ''), args]);
		return { revision: 3, settings: {} };
	}),
}));
vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
		handlers.set(name, handler);
		return unlisten;
	}),
}));

import { createTauriSettingsClient } from './tauriSettingsClient';

const appRoot = join(import.meta.dirname, '../..');
const read = (path: string) => readFileSync(join(appRoot, path), 'utf8');

interface Capability {
	identifier: string;
	windows: string[];
	permissions: string[];
}

const matches = (pattern: string, label: string) =>
	new RegExp(`^${pattern.replace(/[.+?^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*')}$`).test(label);

const appliesTo = (capability: Capability, label: string) =>
	capability.windows.some((pattern) => matches(pattern, label));

beforeEach(() => {
	calls.length = 0;
	handlers.clear();
	unlisten.mockClear();
});

describe('createTauriSettingsClient', () => {
	it('changes only the ui settings through its own command', async () => {
		await createTauriSettingsClient().setUi({ actionBar: false });
		expect(calls).toEqual([['set_ui_settings', { change: { actionBar: false } }]]);
	});

	it('reads and changes the settings through the plugin, by its snake_case names', async () => {
		const client = createTauriSettingsClient();
		const settings = { general: {}, dnd: {} } as never;
		expect(await client.snapshot()).toEqual({ revision: 3, settings: {} });
		await client.set(settings);
		expect(calls).toEqual([
			['get_settings', undefined],
			['set_settings', { settings }],
		]);
	});

	it('hears the change event with its snapshot and stops when asked, even before the listener is ready', async () => {
		const client = createTauriSettingsClient();
		const heard: unknown[] = [];
		const stop = client.onChanged((snapshot) => heard.push(snapshot));
		await Promise.resolve();
		handlers.get('waypoint-settings://changed')!({ payload: { revision: 4, settings: {} } });
		expect(heard).toEqual([{ revision: 4, settings: {} }]);
		stop();
		expect(unlisten).toHaveBeenCalledTimes(1);

		const early = client.onChanged(() => {});
		early();
		await Promise.resolve();
		await Promise.resolve();
		expect(unlisten).toHaveBeenCalledTimes(2);
	});
});

describe('the capabilities for the settings', () => {
	const settingsWindow = JSON.parse(read('src-tauri/capabilities/settings.json')) as Capability;
	const main = JSON.parse(read('src-tauri/capabilities/main.json')) as Capability;
	const ops = JSON.parse(read('src-tauri/capabilities/ops.json')) as Capability;
	const permissionSet = read('../../plugins/waypoint-settings/permissions/default.toml');
	const allowed = new Set([...permissionSet.matchAll(/"allow-([a-z-]+)"/g)].map((m) => m[1]));

	it('lets only the Settings window use the plugin and change the operations settings', () => {
		expect(settingsWindow.windows).toEqual(['settings']);
		expect(settingsWindow.permissions).toContain('waypoint-settings:default');
		expect(settingsWindow.permissions).toContain('waypoint-ops:allow-get-settings');
		expect(settingsWindow.permissions).toContain('waypoint-ops:allow-set-settings');
		// Nothing of the operations plugin beyond its two settings commands and a read of its status,
		// which the Services panel reports.
		expect(settingsWindow.permissions.filter((p) => p.startsWith('waypoint-ops:')).sort()).toEqual([
			'waypoint-ops:allow-get-settings',
			'waypoint-ops:allow-get-status',
			'waypoint-ops:allow-set-settings',
		]);
		expect(appliesTo(ops, 'settings')).toBe(false);
	});

	it('lets the main windows read the settings and change only their ui part, without the default set', () => {
		expect(appliesTo(main, 'main-1')).toBe(true);
		expect(main.permissions).toContain('waypoint-settings:allow-get-settings');
		expect(main.permissions).toContain('waypoint-settings:allow-get-status');
		expect(main.permissions).not.toContain('waypoint-settings:default');
		// Not the whole-document write: a stale window would silently turn the Settings window's changes back.
		expect(main.permissions).not.toContain('waypoint-settings:allow-set-settings');
		expect(main.permissions).toContain('waypoint-settings:allow-set-ui-settings');
		// The operations plugin's default set (the main windows have it) reads its settings and never writes them.
		const opsDefault = read('../../plugins/waypoint-ops/permissions/default.toml');
		expect(ops.permissions).toContain('waypoint-ops:default');
		expect(opsDefault).toContain('"allow-get-settings"');
		expect(opsDefault).not.toContain('"allow-set-settings"');
	});

	it('grants every command the client calls', async () => {
		const client = createTauriSettingsClient();
		await client.snapshot();
		await client.set({} as never);
		await client.setUi({ actionBar: true });
		for (const [command] of calls) {
			expect(allowed.has(command.replaceAll('_', '-')), command).toBe(true);
		}
	});

	it('lists the settings window for logging and the window chrome like every other window', () => {
		for (const file of ['logging.json', 'window-chrome.json']) {
			const capability = JSON.parse(read(`src-tauri/capabilities/${file}`)) as Capability;
			expect(appliesTo(capability, 'settings'), file).toBe(true);
		}
	});
});
