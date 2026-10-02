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
const ROUTED_LABELS = ['main-1', 'settings', 'properties-42', 'ops', 'tear-ghost'];

describe('window capabilities', () => {
	it('lets the main windows save the Action bar choices (the ui part only), and no other command of the settings plugin beyond reading', () => {
		const main = byId('main').permissions.filter((permission) =>
			permission.startsWith('waypoint-settings:'),
		);
		expect(main.sort()).toEqual([
			'waypoint-settings:allow-get-settings',
			'waypoint-settings:allow-get-status',
			'waypoint-settings:allow-set-ui-settings',
		]);
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
		for (const label of ['settings', 'properties-42', 'ops', 'mystery']) {
			expect(covers(capability, label), label).toBe(false);
		}
	});
});
