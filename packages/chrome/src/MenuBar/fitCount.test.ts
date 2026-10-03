// Tests for the menu bar's fit: how many menus show before the rest collapse into More
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { fitCount } from './fitCount';

describe('fitCount', () => {
	it('shows every menu when they all fit, with no room kept for More', () => {
		expect(fitCount([40, 40, 40], 128, 4, 36)).toBe(3);
	});

	it('keeps room for More and drops the trailing menus', () => {
		expect(fitCount([40, 40, 40], 100, 4, 36)).toBe(1);
	});

	it('can show none', () => {
		expect(fitCount([40, 40], 50, 4, 36)).toBe(0);
	});
});
