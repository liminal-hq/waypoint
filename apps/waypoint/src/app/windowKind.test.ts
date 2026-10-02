// Tests for windowKindFromLabel
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { windowKindFromLabel } from './windowKind';

describe('windowKindFromLabel', () => {
	it('classifies known labels', () => {
		expect(windowKindFromLabel('main-1')).toBe('Main');
		expect(windowKindFromLabel('settings')).toBe('Settings');
		expect(windowKindFromLabel('properties-42')).toBe('Properties');
		expect(windowKindFromLabel('ops')).toBe('Ops');
		expect(windowKindFromLabel('shelf')).toBe('Shelf');
		expect(windowKindFromLabel('tear-ghost')).toBe('TearGhost');
	});

	it('rejects unknown labels', () => {
		expect(windowKindFromLabel('mystery')).toBeNull();
		expect(windowKindFromLabel('main')).toBeNull();
		expect(windowKindFromLabel('')).toBeNull();
	});
});
