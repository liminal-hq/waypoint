// Tests for what "Reset to defaults" restores on the Transparency page
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { resetTransparency, transparencyAtDefaults } from './transparencyReset';

const defaults = DEFAULT_SETTINGS.transparency;

describe('the default opacities', () => {
	it('reproduce the look the fixed offsets from 82 gave', () => {
		expect(defaults.opacity).toBe(82);
		expect([defaults.rowsOpacity, defaults.sidebarOpacity, defaults.contentOpacity]).toEqual([
			90, 94, 98,
		]);
	});
});

describe('resetTransparency', () => {
	it('restores every setting on the page', () => {
		const changed = {
			enabled: false,
			opacity: 50,
			rowsOpacity: 60,
			sidebarOpacity: 70,
			contentOpacity: 80,
			blur: 'high' as const,
			regions: { sidebar: false, content: true, titleBar: false },
			menus: true,
			menuOpacity: 70,
			solidWhenUnfocused: false,
		};
		expect(resetTransparency(changed)).toEqual({ ...defaults, enabled: false });
	});

	it('leaves the master switch as it is, on or off', () => {
		expect(resetTransparency({ ...defaults, enabled: true }).enabled).toBe(true);
		expect(resetTransparency({ ...defaults, enabled: false }).enabled).toBe(false);
	});
});

describe('transparencyAtDefaults', () => {
	it('is true at the defaults whatever the master switch says', () => {
		expect(transparencyAtDefaults(defaults)).toBe(true);
		expect(transparencyAtDefaults({ ...defaults, enabled: true })).toBe(true);
	});

	it('is false when any one setting differs', () => {
		const differs = [
			{ opacity: 81 },
			{ rowsOpacity: 91 },
			{ sidebarOpacity: 95 },
			{ contentOpacity: 97 },
			{ blur: 'high' as const },
			{ menus: true },
			{ menuOpacity: 95 },
			{ solidWhenUnfocused: false },
			{ regions: { ...defaults.regions, content: true } },
			{ regions: { ...defaults.regions, titleBar: false } },
		];
		for (const change of differs) {
			expect(transparencyAtDefaults({ ...defaults, ...change }), JSON.stringify(change)).toBe(
				false,
			);
		}
	});
});
