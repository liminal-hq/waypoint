// Verifies the real OpsClient reaches each plugin command, and that the capabilities let the windows that use it call them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const calls: string[] = [];
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn(async (command: string) => {
		calls.push(command.replace('plugin:waypoint-ops|', ''));
		return null;
	}),
	Channel: class {
		onmessage: ((message: unknown) => void) | null = null;
	},
}));
vi.mock('@tauri-apps/api/event', () => ({
	listen: vi.fn(async () => () => {}),
}));

import { createTauriOpsClient } from './tauriOpsClient';

const appRoot = join(import.meta.dirname, '../..');
const read = (path: string) => readFileSync(join(appRoot, path), 'utf8');

interface Capability {
	identifier: string;
	windows: string[];
	permissions: string[];
}

const matches = (pattern: string, label: string) =>
	new RegExp(`^${pattern.replace(/[.+?^${}()|[\]\\]/g, '\\$&').replace(/\*/g, '.*')}$`).test(label);

const location = { display: '/a', uri: 'file:///a' };

beforeEach(() => {
	calls.length = 0;
});

describe('createTauriOpsClient', () => {
	it('calls each command of the plugin, by its snake_case name', async () => {
		const client = createTauriOpsClient();
		const request = {} as never;
		await client.snapshot();
		await client.plan(request);
		await client.submit(request);
		await client.pause(1);
		await client.resume(1);
		await client.cancel(1);
		await client.retry(1);
		await client.dismiss(1);
		await client.dismissFinished();
		await client.reorder(1, 0);
		await client.resolve(1, []);
		await client.resolveError(1, 'skip');
		await client.undo();
		await client.redo();
		await client.journalSummaries();
		await client.jobsTargeting(location);
		await client.getClipboard();
		await client.setClipboard('copy', []);
		await client.getSettings();
		await client.setSettings({} as never);
		await client.takeRecoveryReport();
		const stop = await client.subscribeProgress(() => {});
		await stop();
		expect(calls).toEqual([
			'get_snapshot',
			'plan',
			'submit',
			'pause',
			'resume',
			'cancel',
			'retry',
			'dismiss',
			'dismiss_finished',
			'reorder',
			'resolve',
			'resolve_error',
			'undo',
			'redo',
			'journal_summaries',
			'jobs_targeting',
			'get_clipboard',
			'set_clipboard',
			'get_settings',
			'set_settings',
			'take_recovery_report',
			'subscribe_progress',
			'unsubscribe_progress',
		]);
	});

	it('listens to the broadcast events and stops when asked', () => {
		const client = createTauriOpsClient();
		const stop = client.onEvent(() => {});
		client.onClipboard(() => {});
		client.onRecovered(() => {});
		expect(() => stop()).not.toThrow();
	});
});

describe('the capabilities for the operations windows', () => {
	const capability = JSON.parse(read('src-tauri/capabilities/ops.json')) as Capability;
	const permissionSet = read('../../plugins/waypoint-ops/permissions/default.toml');
	const allowed = new Set([...permissionSet.matchAll(/"allow-([a-z-]+)"/g)].map((m) => m[1]));

	it('lets the Operations window and the main windows use the plugin, and no other', () => {
		expect(capability.permissions).toContain('waypoint-ops:default');
		for (const label of ['ops', 'main-1', 'main-12']) {
			expect(
				capability.windows.some((pattern) => matches(pattern, label)),
				label,
			).toBe(true);
		}
		for (const label of ['settings', 'properties-1', 'tear-ghost']) {
			expect(
				capability.windows.some((pattern) => matches(pattern, label)),
				label,
			).toBe(false);
		}
	});

	it('grants every command the client calls', async () => {
		const client = createTauriOpsClient();
		calls.length = 0;
		await client.snapshot();
		await client.subscribeProgress(() => {});
		await (
			await client.subscribeProgress(() => {})
		)();
		for (const command of new Set(calls)) {
			expect(allowed.has(command.replaceAll('_', '-')), command).toBe(true);
		}
	});

	it('is covered by the window chrome capability for events and logging in the Operations window', () => {
		const chrome = JSON.parse(read('src-tauri/capabilities/window-chrome.json')) as Capability;
		const logging = JSON.parse(read('src-tauri/capabilities/logging.json')) as Capability;
		expect(chrome.windows).toContain('ops');
		expect(chrome.permissions).toContain('core:default');
		expect(logging.windows.some((pattern) => matches(pattern, 'ops'))).toBe(true);
	});
});
