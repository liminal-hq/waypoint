// Verifies keyboard movement through groups: headers skipped, folded groups stopped on, columns kept
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupRun } from '@liminal-hq/waypoint-protocol/generated/GroupRun';
import { describe, expect, it } from 'vitest';
import { GroupLayout, groupId } from './groupLayout';
import { firstTarget, navigate } from './groupNav';

const run = (extension: string, start: number, count: number): GroupRun => ({
	key: { kind: 'type', extension },
	start,
	count,
});
const GROUPS = [run('a', 0, 3), run('b', 3, 2), run('c', 5, 5)];
const fold = (...runs: GroupRun[]) => new Set(runs.map((r) => groupId(r.key)));
const at = (position: number) => ({ position });

describe('in a list', () => {
	const list = new GroupLayout(GROUPS, new Set(), 10);
	const go = (key: string, from: number | null) =>
		navigate(list, key, from === null ? null : at(from), 5);

	it('moves one entry at a time and steps over the headers between groups', () => {
		expect(go('ArrowDown', 1)).toEqual(at(2));
		expect(go('ArrowDown', 2)).toEqual(at(3));
		expect(go('ArrowUp', 3)).toEqual(at(2));
		expect(go('ArrowDown', 9)).toEqual(at(9));
		expect(go('ArrowUp', 0)).toEqual(at(0));
	});

	it('goes to the ends and by pages', () => {
		expect(go('Home', 6)).toEqual(at(0));
		expect(go('End', 0)).toEqual(at(9));
		expect(go('PageDown', 0)).toEqual(at(4));
		// Five rows up from the last entry is a header, which is stepped over.
		expect(go('PageUp', 9)).toEqual(at(4));
	});

	it('starts on the first entry when the keyboard is nowhere yet', () => {
		expect(go('ArrowDown', null)).toEqual(at(0));
		expect(firstTarget(list)).toEqual(at(0));
	});

	it('steps up onto the header of its group with Left, and has nothing for Right', () => {
		expect(go('ArrowLeft', 4)).toEqual({ header: 1 });
		expect(go('ArrowRight', 4)).toBeNull();
		expect(go('x', 4)).toBeNull();
	});

	it('goes down from a header to its first entry and up to the group before', () => {
		expect(navigate(list, 'ArrowDown', { header: 1 }, 5)).toEqual(at(3));
		expect(navigate(list, 'ArrowUp', { header: 1 }, 5)).toEqual(at(2));
	});
});

describe('with a folded group', () => {
	const list = new GroupLayout(GROUPS, fold(GROUPS[1]!), 10);

	it('stops on the folded header, which has no entry to land on, and carries on from it', () => {
		expect(navigate(list, 'ArrowDown', at(2), 5)).toEqual({ header: 1 });
		expect(navigate(list, 'ArrowDown', { header: 1 }, 5)).toEqual(at(5));
		expect(navigate(list, 'ArrowUp', at(5), 5)).toEqual({ header: 1 });
		expect(navigate(list, 'ArrowUp', { header: 1 }, 5)).toEqual(at(2));
	});

	it('ends on the header when the last group is folded', () => {
		const last = new GroupLayout(GROUPS, fold(GROUPS[2]!), 10);
		expect(navigate(last, 'End', at(0), 5)).toEqual({ header: 2 });
	});

	it('starts on the header when the first group is folded', () => {
		expect(firstTarget(new GroupLayout(GROUPS, fold(GROUPS[0]!), 10))).toEqual({ header: 0 });
	});
});

describe('in a grid of three columns', () => {
	// Groups 0-2, 3-4 and 5-9 are lines: [0 1 2], [3 4], [5 6 7], [8 9].
	const grid = new GroupLayout(GROUPS, new Set(), 10, 3);
	const go = (key: string, from: number) => navigate(grid, key, at(from), 2);

	it('keeps the column going down across a group, and ends in a short line', () => {
		expect(go('ArrowDown', 1)).toEqual(at(4));
		expect(go('ArrowDown', 2)).toEqual(at(4));
		expect(go('ArrowDown', 4)).toEqual(at(6));
		expect(go('ArrowDown', 7)).toEqual(at(9));
		expect(go('ArrowUp', 6)).toEqual(at(4));
		expect(go('ArrowUp', 3)).toEqual(at(0));
	});

	it('moves a cell at a time past the headers', () => {
		expect(go('ArrowRight', 2)).toEqual(at(3));
		expect(go('ArrowRight', 9)).toBeNull();
		expect(go('ArrowLeft', 4)).toEqual(at(3));
		expect(go('ArrowLeft', 3)).toEqual({ header: 1 });
	});

	it('skips folded groups sideways', () => {
		const folded = new GroupLayout(GROUPS, fold(GROUPS[1]!), 10, 3);
		expect(navigate(folded, 'ArrowRight', at(2), 2)).toEqual(at(5));
	});

	it('goes to the first and last cells', () => {
		expect(go('Home', 8)).toEqual(at(0));
		expect(go('End', 0)).toEqual(at(9));
	});
});
