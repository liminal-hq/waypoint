// Verifies how view positions follow a patch
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { PatchOp } from '@liminal-hq/waypoint-protocol/generated/PatchOp';
import { describe, expect, it } from 'vitest';
import { isReset, mapPosition } from './patch';

const remove = (at: number, count = 1): PatchOp => ({ kind: 'remove', at, count });
const insert = (at: number, count = 1): PatchOp => ({ kind: 'insert', at, count });

describe('mapPosition', () => {
	it('leaves a position before every edit alone', () => {
		expect(mapPosition(3, [remove(10), insert(20, 4)])).toEqual({ position: 3, removed: false });
	});

	it('moves a position up past a removal above it', () => {
		expect(mapPosition(10, [remove(2, 3)])).toEqual({ position: 7, removed: false });
	});

	it('moves a position down past an insertion at or above it', () => {
		expect(mapPosition(5, [insert(5, 2)])).toEqual({ position: 7, removed: false });
		expect(mapPosition(5, [insert(2, 2)])).toEqual({ position: 7, removed: false });
		expect(mapPosition(5, [insert(6, 2)])).toEqual({ position: 5, removed: false });
	});

	it('reports a removed position and where its successor now sits', () => {
		expect(mapPosition(4, [remove(3, 3)])).toEqual({ position: 3, removed: true });
	});

	it('applies ops in order, each in the coordinates the last one left', () => {
		// Remove 2 entries at 8 and 1 at 0 (highest first), then insert 3 at 0.
		expect(mapPosition(10, [remove(8, 2), remove(0, 1), insert(0, 3)])).toEqual({
			position: 10,
			removed: false,
		});
	});

	it('ignores updates and leaves a reset position for the caller to clamp', () => {
		expect(mapPosition(9, [{ kind: 'update', at: 0, count: 50 }])).toEqual({
			position: 9,
			removed: false,
		});
		expect(mapPosition(9, [{ kind: 'reset' }])).toEqual({ position: 9, removed: false });
	});
});

describe('isReset', () => {
	it('spots a reset among other ops', () => {
		expect(isReset([insert(0), { kind: 'reset' }])).toBe(true);
		expect(isReset([insert(0)])).toBe(false);
	});
});
