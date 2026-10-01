// Verifies the drop regions a strip registers and how a hit on one is read back
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { buildDropRegions, parseRegionId, sameRegions, slotRegionId } from './dropRegions';

const STRIP = { left: 10, top: 5, right: 410, bottom: 35 };

describe('buildDropRegions', () => {
	it('gives the strip as a whole and two halves for each tab', () => {
		const regions = buildDropRegions(STRIP, [
			{ index: 0, left: 10, right: 110 },
			{ index: 1, left: 110, right: 210 },
		]);
		expect(regions).toEqual([
			{ id: 'strip', x: 10, y: 5, width: 400, height: 30 },
			{ id: 'slot:0', x: 10, y: 5, width: 50, height: 30 },
			{ id: 'slot:1', x: 60, y: 5, width: 50, height: 30 },
			{ id: 'slot:1', x: 110, y: 5, width: 50, height: 30 },
			{ id: 'slot:2', x: 160, y: 5, width: 50, height: 30 },
		]);
	});

	it('gives the gap between two tabs and the padding before the first to the nearer tab, as the line does', () => {
		const slots = [
			{ index: 0, left: 20, right: 120 },
			{ index: 1, left: 122, right: 222 },
		];
		const regions = buildDropRegions(STRIP, slots);
		// The padding 10–20 is the first tab's left half; the gap 120–122 continues its right half.
		expect(regions).toContainEqual({ id: 'slot:0', x: 10, y: 5, width: 60, height: 30 });
		expect(regions).toContainEqual({ id: 'slot:1', x: 70, y: 5, width: 52, height: 30 });
		// Nothing is left to the strip's "end" region between tabs.
		for (const x of [15, 121]) {
			const hit = regions.filter((r) => r.id !== 'strip' && x >= r.x && x < r.x + r.width);
			expect(hit).toHaveLength(1);
		}
	});

	it('clips a tab that a scrolled strip has partly hidden, and drops one that is out of sight', () => {
		const regions = buildDropRegions(STRIP, [
			{ index: 3, left: -30, right: 70 },
			{ index: 4, left: 380, right: 480 },
			{ index: 5, left: 500, right: 600 },
		]);
		expect(regions.slice(1)).toEqual([
			{ id: 'slot:3', x: 10, y: 5, width: 10, height: 30 },
			{ id: 'slot:4', x: 20, y: 5, width: 360, height: 30 },
			{ id: 'slot:4', x: 380, y: 5, width: 30, height: 30 },
		]);
	});

	it('puts an expanded group chip before the group, and halves a collapsed one around the whole group', () => {
		const regions = buildDropRegions(
			STRIP,
			[{ index: 0, left: 10, right: 110 }],
			[
				{ firstIndex: 1, count: 2, collapsed: false, left: 110, right: 150 },
				{ firstIndex: 3, count: 4, collapsed: true, left: 150, right: 250 },
			],
		);
		expect(regions.slice(3)).toEqual([
			{ id: 'slot:1', x: 110, y: 5, width: 40, height: 30 },
			{ id: 'slot:3', x: 150, y: 5, width: 50, height: 30 },
			{ id: 'slot:7', x: 200, y: 5, width: 50, height: 30 },
		]);
	});

	it('clips a chip to the strip and drops one that is scrolled out of it', () => {
		const regions = buildDropRegions(
			STRIP,
			[],
			[
				{ firstIndex: 0, count: 1, collapsed: false, left: -50, right: 30 },
				{ firstIndex: 4, count: 1, collapsed: false, left: -90, right: -10 },
			],
		);
		expect(regions.map((region) => region.id)).toEqual(['strip', 'slot:0']);
		expect(regions[1]).toMatchObject({ x: 10, width: 20 });
	});

	it('registers nothing for a strip with no area', () => {
		expect(buildDropRegions({ left: 0, top: 0, right: 0, bottom: 30 }, [])).toEqual([]);
	});
});

describe('parseRegionId', () => {
	it('reads the strip and a slot, and nothing else', () => {
		expect(parseRegionId('strip')).toEqual({ kind: 'end' });
		expect(parseRegionId(slotRegionId(7))).toEqual({ kind: 'slot', index: 7 });
		expect(parseRegionId('slot:')).toBeNull();
		expect(parseRegionId('slot:-1')).toBeNull();
		expect(parseRegionId('slot:2x')).toBeNull();
		expect(parseRegionId('tab-strip')).toBeNull();
	});
});

describe('sameRegions', () => {
	it('compares every field', () => {
		const a = buildDropRegions(STRIP, [{ index: 0, left: 10, right: 110 }]);
		expect(sameRegions(a, buildDropRegions(STRIP, [{ index: 0, left: 10, right: 110 }]))).toBe(
			true,
		);
		expect(sameRegions(a, buildDropRegions(STRIP, [{ index: 0, left: 10, right: 120 }]))).toBe(
			false,
		);
		expect(sameRegions(a, a.slice(1))).toBe(false);
	});
});
