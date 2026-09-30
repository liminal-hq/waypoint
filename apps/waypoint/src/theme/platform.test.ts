// Verifies the platform attribute is set inside Tauri and skipped outside it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';

let current: () => string;
let osVersion = '10.0.22631';

vi.mock('@tauri-apps/plugin-os', () => ({ platform: () => current(), version: () => osVersion }));

beforeEach(() => {
	vi.resetModules();
});

describe('applyPlatform', () => {
	it('tags the root with the platform name', async () => {
		current = () => 'linux';
		const { applyPlatform } = await import('./platform');
		const root = document.createElement('html');
		applyPlatform(root);
		expect(root.dataset.platform).toBe('linux');
	});

	it('leaves the attribute unset outside Tauri', async () => {
		current = () => {
			throw new Error('no tauri');
		};
		const { applyPlatform } = await import('./platform');
		const root = document.createElement('html');
		expect(() => applyPlatform(root)).not.toThrow();
		expect(root.dataset.platform).toBeUndefined();
	});

	it('flags Windows 11 by build number and leaves Windows 10 unflagged', async () => {
		current = () => 'windows';
		const { applyPlatform } = await import('./platform');
		const win11 = document.createElement('html');
		osVersion = '10.0.22631';
		applyPlatform(win11);
		expect(win11.dataset.windows11).toBe('true');

		const win10 = document.createElement('html');
		osVersion = '10.0.19045';
		applyPlatform(win10);
		expect(win10.dataset.windows11).toBe('false');
	});

	it('does not set the Windows 11 flag on other platforms', async () => {
		current = () => 'linux';
		const { applyPlatform } = await import('./platform');
		const root = document.createElement('html');
		applyPlatform(root);
		expect(root.dataset.windows11).toBeUndefined();
	});
});
