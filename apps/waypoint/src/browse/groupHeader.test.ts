// Verifies the words of a group header: every key has a heading, and the label says the count and whether it is open
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupKey } from '@liminal-hq/waypoint-protocol/generated/GroupKey';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { ModifiedBucket } from '@liminal-hq/waypoint-protocol/generated/ModifiedBucket';
import type { SizeBand } from '@liminal-hq/waypoint-protocol/generated/SizeBand';
import { describe, expect, it } from 'vitest';
import { groupCount, groupLabel, groupTitle } from './groupHeader';

describe('the heading of a group', () => {
	it('names each kind of file in the plural', () => {
		const kinds: IconGroup[] = [
			'folder',
			'image',
			'audio',
			'video',
			'archive',
			'code',
			'document',
			'other',
		];
		expect(kinds.map((group) => groupTitle({ kind: 'kind', group }))).toEqual([
			'Folders',
			'Images',
			'Audio',
			'Videos',
			'Archives',
			'Code',
			'Documents',
			'Other files',
		]);
	});

	it('names each modified band, and an earlier year by its number', () => {
		const buckets: ModifiedBucket[] = [
			'today',
			'yesterday',
			'earlierThisWeek',
			'last7Days',
			'last30Days',
			'thisYear',
			'unknown',
		];
		expect(buckets.map((bucket) => groupTitle({ kind: 'modified', bucket }))).toEqual([
			'Today',
			'Yesterday',
			'Earlier this week',
			'Last 7 days',
			'Last 30 days',
			'This year',
			'Unknown date',
		]);
		expect(groupTitle({ kind: 'year', year: 2019 })).toBe('2019');
	});

	it('names each size band', () => {
		const bands: SizeBand[] = [
			'unspecified',
			'empty',
			'tiny',
			'small',
			'medium',
			'large',
			'huge',
			'gigantic',
		];
		for (const band of bands) expect(groupTitle({ kind: 'size', band })).not.toMatch(/^browse\./);
		expect(groupTitle({ kind: 'size', band: 'gigantic' })).toBe('Gigantic (over 128 MB)');
	});

	it('shows a letter as it is, symbols in words and a type by its upper-case extension', () => {
		expect(groupTitle({ kind: 'name', initial: 'É' })).toBe('É');
		expect(groupTitle({ kind: 'name', initial: '#' })).toBe('Digits and symbols');
		expect(groupTitle({ kind: 'type', extension: 'txt' })).toBe('TXT files');
		expect(groupTitle({ kind: 'type', extension: '' })).toBe('No extension');
	});
});

describe('the count and the label', () => {
	it('counts items in the singular and the plural', () => {
		expect(groupCount(1)).toBe('1 item');
		expect(groupCount(1200)).toBe('1,200 items');
	});

	it('reads heading, count and whether the group is open', () => {
		const key: GroupKey = { kind: 'modified', bucket: 'today' };
		expect(groupLabel(key, 3, false)).toBe('Today, 3 items, expanded');
		expect(groupLabel(key, 1, true)).toBe('Today, 1 item, collapsed');
	});
});
