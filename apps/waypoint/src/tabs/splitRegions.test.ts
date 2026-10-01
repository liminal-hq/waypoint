// Verifies the split regions' geometry: the four hit regions, their boundaries, the previews and the hysteresis
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { previewRect, zoneAt, zoneRect } from './splitRegions';

// 600 wide so the thirds are 200, and 400 tall so the centre column splits at 300.
const area = { left: 100, top: 100, right: 700, bottom: 500 };

describe('zoneAt', () => {
	it('names the left third, the right third, and the centre column by halves', () => {
		expect(zoneAt(area, { x: 150, y: 120 })).toBe('left');
		expect(zoneAt(area, { x: 150, y: 480 })).toBe('left');
		expect(zoneAt(area, { x: 650, y: 120 })).toBe('right');
		expect(zoneAt(area, { x: 650, y: 480 })).toBe('right');
		expect(zoneAt(area, { x: 400, y: 150 })).toBe('top');
		expect(zoneAt(area, { x: 400, y: 450 })).toBe('bottom');
	});

	it('puts the boundaries on the first region they meet, and has no gaps', () => {
		expect(zoneAt(area, { x: 299, y: 300 })).toBe('left');
		expect(zoneAt(area, { x: 301, y: 300 })).toBe('top');
		expect(zoneAt(area, { x: 400, y: 299 })).toBe('top');
		expect(zoneAt(area, { x: 400, y: 301 })).toBe('bottom');
		expect(zoneAt(area, { x: 499, y: 400 })).toBe('bottom');
		expect(zoneAt(area, { x: 501, y: 400 })).toBe('right');
		// The corners of the area belong to the side regions.
		expect(zoneAt(area, { x: 100, y: 100 })).toBe('left');
		expect(zoneAt(area, { x: 700, y: 500 })).toBe('right');
	});

	it('is null outside the area, and for an area with no size', () => {
		expect(zoneAt(area, { x: 99, y: 300 })).toBeNull();
		expect(zoneAt(area, { x: 400, y: 99 })).toBeNull();
		expect(zoneAt(area, { x: 701, y: 300 })).toBeNull();
		expect(zoneAt(area, { x: 400, y: 501 })).toBeNull();
		expect(zoneAt({ left: 0, top: 0, right: 0, bottom: 0 }, { x: 0, y: 0 })).toBeNull();
	});

	it('keeps the previous region within the slack of its border and of the area', () => {
		// Just into the centre column, but still the left region's while the slack covers it.
		expect(zoneAt(area, { x: 304, y: 300 }, 'left', 6)).toBe('left');
		expect(zoneAt(area, { x: 310, y: 300 }, 'left', 6)).toBe('top');
		// Just past the area's own edge.
		expect(zoneAt(area, { x: 96, y: 300 }, 'left', 6)).toBe('left');
		expect(zoneAt(area, { x: 90, y: 300 }, 'left', 6)).toBeNull();
		// Entering from outside has no slack: a pointer just outside is not in.
		expect(zoneAt(area, { x: 96, y: 300 }, null, 6)).toBeNull();
	});
});

describe('previewRect', () => {
	it('is the half of the area the new pane would take', () => {
		expect(previewRect(area, 'left')).toEqual({ left: 100, top: 100, right: 400, bottom: 500 });
		expect(previewRect(area, 'right')).toEqual({ left: 400, top: 100, right: 700, bottom: 500 });
		expect(previewRect(area, 'top')).toEqual({ left: 100, top: 100, right: 700, bottom: 300 });
		expect(previewRect(area, 'bottom')).toEqual({ left: 100, top: 300, right: 700, bottom: 500 });
	});
});

describe('zoneRect', () => {
	it('tiles the area: the regions share borders and cover its width and height', () => {
		expect(zoneRect(area, 'left')).toEqual({ left: 100, top: 100, right: 300, bottom: 500 });
		expect(zoneRect(area, 'right')).toEqual({ left: 500, top: 100, right: 700, bottom: 500 });
		expect(zoneRect(area, 'top')).toEqual({ left: 300, top: 100, right: 500, bottom: 300 });
		expect(zoneRect(area, 'bottom')).toEqual({ left: 300, top: 300, right: 500, bottom: 500 });
	});
});
