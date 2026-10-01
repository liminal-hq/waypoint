// Verifies the scroll-cap arithmetic and the row-height measurement
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	DEFAULT_ROW_HEIGHT,
	MAX_SCROLL_HEIGHT,
	measureRowHeight,
	rowCeiling,
	visibleRows,
} from './scrollCap';

describe('rowCeiling', () => {
	it('fits as many rows as stay under the webview cap', () => {
		expect(rowCeiling(28)).toBe(1_198_372);
		expect(rowCeiling(28) * 28).toBeLessThanOrEqual(MAX_SCROLL_HEIGHT);
		expect((rowCeiling(28) + 1) * 28).toBeGreaterThan(MAX_SCROLL_HEIGHT);
	});

	it('is lower for taller rows', () => {
		expect(rowCeiling(44)).toBeLessThan(rowCeiling(28));
		expect(rowCeiling(44) * 44).toBeLessThanOrEqual(MAX_SCROLL_HEIGHT);
	});
});

describe('visibleRows', () => {
	it('shows everything below the ceiling', () => {
		expect(visibleRows(500_000, 28)).toEqual({ shown: 500_000, hidden: 0 });
	});

	it('hides the tail above it, never drawing past the cap', () => {
		const { shown, hidden } = visibleRows(2_000_000, 28);
		expect(shown).toBe(1_198_372);
		expect(hidden).toBe(2_000_000 - 1_198_372);
		expect(shown * 28).toBeLessThanOrEqual(MAX_SCROLL_HEIGHT);
	});
});

describe('measureRowHeight', () => {
	afterEach(() => vi.restoreAllMocks());

	it('reads the stylesheet token', () => {
		vi.spyOn(window, 'getComputedStyle').mockReturnValue({
			getPropertyValue: () => ' 44px ',
		} as unknown as CSSStyleDeclaration);
		expect(measureRowHeight(document.body)).toBe(44);
	});

	it('falls back when the token is missing or not in pixels', () => {
		expect(measureRowHeight(document.body)).toBe(DEFAULT_ROW_HEIGHT);
		vi.spyOn(window, 'getComputedStyle').mockReturnValue({
			getPropertyValue: () => '2rem',
		} as unknown as CSSStyleDeclaration);
		expect(measureRowHeight(document.body)).toBe(DEFAULT_ROW_HEIGHT);
	});
});
