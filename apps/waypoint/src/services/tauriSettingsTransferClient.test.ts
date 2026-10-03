// Verifies the real SettingsTransferClient reaches the plugin's export and import commands with only what Rust needs, and that only the Settings window is granted them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const calls: Array<[string, unknown]> = [];
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(async (command: string, args?: unknown) => {
		calls.push([command.replace('plugin:waypoint-settings|', ''), args]);
		return null;
	}),
}));

import { createTauriSettingsTransferClient } from './tauriSettingsTransferClient';

const appRoot = join(import.meta.dirname, '../..');
const read = (path: string) => readFileSync(join(appRoot, path), 'utf8');

interface Capability {
	identifier: string;
	windows: string[];
	permissions: string[];
}

beforeEach(() => {
	calls.length = 0;
});

describe('createTauriSettingsTransferClient', () => {
	it('exports with the time zone offset and nothing else: no path, no document', async () => {
		await createTauriSettingsTransferClient().exportSettings();
		expect(calls).toHaveLength(1);
		const [command, args] = calls[0] as [string, { utcOffsetMinutes: number }];
		expect(command).toBe('export_settings');
		expect(Object.keys(args)).toEqual(['utcOffsetMinutes']);
		expect(args.utcOffsetMinutes).toBe(-new Date().getTimezoneOffset());
	});

	it('plans an import with no arguments and applies it by the plan number alone', async () => {
		const client = createTauriSettingsTransferClient();
		await client.planImport();
		await client.applyImport(12);
		expect(calls).toEqual([
			['plan_settings_import', undefined],
			['apply_settings_import', { planId: 12 }],
		]);
	});
});

describe('the capabilities for exporting and importing', () => {
	const settingsWindow = JSON.parse(read('src-tauri/capabilities/settings.json')) as Capability;
	const main = JSON.parse(read('src-tauri/capabilities/main.json')) as Capability;
	const allowed = new Set(
		[
			...read('../../plugins/waypoint-settings/permissions/default.toml').matchAll(
				/"allow-([a-z-]+)"/g,
			),
		].map((m) => m[1]),
	);

	it('grants every command the client calls to the Settings window by name, and keeps them out of the default set', async () => {
		const client = createTauriSettingsTransferClient();
		await client.exportSettings();
		await client.planImport();
		await client.applyImport(1);
		expect(calls.length).toBe(3);
		for (const [command] of calls) {
			const name = command.replaceAll('_', '-');
			expect(settingsWindow.permissions, command).toContain(`waypoint-settings:allow-${name}`);
			expect(allowed.has(name), `${command} is not in the default set`).toBe(false);
			expect(main.permissions, command).not.toContain(`waypoint-settings:allow-${name}`);
		}
	});

	it('gives the Settings window no file dialog or file system permission: the dialogs open from Rust', () => {
		expect(settingsWindow.permissions.filter((p) => /^(dialog|fs|opener):/.test(p))).toEqual([]);
	});
});
