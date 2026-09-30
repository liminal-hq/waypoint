// Verifies the platform attribute is set inside Tauri and skipped outside it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { beforeEach, describe, expect, it, vi } from 'vitest';

let current: () => string;

vi.mock('@tauri-apps/plugin-os', () => ({ platform: () => current() }));

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
});
