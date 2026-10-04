// Verifies the real GitClient reaches each plugin command and event, and that the capabilities grant what each window uses
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const calls: Array<[string, unknown]> = [];
const handlers = new Map<string, (event: { payload: unknown }) => void>();
const unlisten = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(async (command: string, args?: unknown) => {
		calls.push([command.replace('plugin:waypoint-git|', ''), args]);
		return null;
	}),
}));
vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn(async (name: string, handler: (event: { payload: unknown }) => void) => {
		handlers.set(name, handler);
		return unlisten;
	}),
}));

import { createTauriGitClient } from './tauriGitClient';

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

const HOME = { display: '/home/me', uri: 'file:///home/me' };

describe('createTauriGitClient', () => {
	it('watches, stops and asks for badges through the plugin, by its snake_case names', async () => {
		const client = createTauriGitClient();
		await client.watch(HOME);
		await client.unwatch(4);
		await client.badges([HOME]);
		expect(calls).toEqual([
			['git_watch', { location: HOME }],
			['git_unwatch', { id: 4 }],
			['git_badges', { locations: [HOME] }],
		]);
	});

	it('hears the change event and stops when asked, even before the listener is ready', async () => {
		const client = createTauriGitClient();
		const heard: unknown[] = [];
		const stop = client.onChanged((changed) => heard.push(changed));
		await Promise.resolve();
		const changed = { id: 1, revision: 2, summary: {} };
		handlers.get('waypoint-git://changed')!({ payload: changed });
		expect(heard).toEqual([changed]);
		stop();
		expect(unlisten).toHaveBeenCalledTimes(1);

		const early = client.onChanged(() => {});
		early();
		await Promise.resolve();
		await Promise.resolve();
		expect(unlisten).toHaveBeenCalledTimes(2);
	});
});

describe('the capabilities for Git', () => {
	const main = JSON.parse(read('src-tauri/capabilities/main.json')) as Capability;
	const settingsWindow = JSON.parse(read('src-tauri/capabilities/settings.json')) as Capability;
	const permissionSet = read('../../plugins/waypoint-git/permissions/default.toml');
	const allowed = new Set([...permissionSet.matchAll(/"allow-([a-z-]+)"/g)].map((m) => m[1]));

	it('grants every command the client calls', async () => {
		const client = createTauriGitClient();
		await client.watch(HOME);
		await client.unwatch(1);
		await client.badges([]);
		for (const [command] of calls) {
			expect(allowed.has(command.replaceAll('_', '-')), command).toBe(true);
		}
	});

	it('lets the main windows watch repositories and the Settings window read only the status', () => {
		expect(appliesTo(main, 'main-1')).toBe(true);
		expect(main.permissions).toContain('waypoint-git:default');
		expect(settingsWindow.permissions.filter((p) => p.startsWith('waypoint-git:'))).toEqual([
			'waypoint-git:allow-get-status',
		]);
	});

	it('gives no other window the plugin: the Shelf, Operations and Properties windows read no repository', () => {
		for (const file of readdirCapabilities()) {
			const capability = JSON.parse(read(`src-tauri/capabilities/${file}`)) as Capability;
			if (['main.json', 'settings.json'].includes(file)) continue;
			expect(
				capability.permissions.filter((p) => String(p).startsWith('waypoint-git:')),
				file,
			).toEqual([]);
		}
	});
});

function readdirCapabilities(): string[] {
	return readdirSync(join(appRoot, 'src-tauri/capabilities')).filter((file) =>
		file.endsWith('.json'),
	);
}
