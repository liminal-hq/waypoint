// Tests for the fuzzy matcher: what matches, what scores higher, and the ranges it reports
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { fuzzyMatch, splitByRanges } from './fuzzy';

describe('what matches', () => {
	const table: Array<[query: string, text: string, matches: boolean]> = [
		['', 'New Folder', true],
		['new', 'New Folder', true],
		['NEW', 'New Folder', true],
		['nf', 'New Folder', true],
		['newf', 'New Folder', true],
		['new f', 'New Folder', true],
		['fn', 'New Folder', false],
		['folderx', 'New Folder', false],
		['zzz', 'New Folder', false],
		['undo', 'Undo: Move 3 items to Trash', true],
		['mtt', 'Move to Trash', true],
		['é', 'Café', true],
		['longer than the text', 'short', false],
	];
	for (const [query, text, matches] of table) {
		it(`${JSON.stringify(query)} ${matches ? 'matches' : 'does not match'} ${JSON.stringify(text)}`, () => {
			expect(fuzzyMatch(query, text) !== null).toBe(matches);
		});
	}

	it('matches everything with no ranges for an empty or blank query', () => {
		expect(fuzzyMatch('', 'anything')).toEqual({ score: 0, ranges: [] });
		expect(fuzzyMatch('   ', 'anything')).toEqual({ score: 0, ranges: [] });
	});
});

describe('ranges', () => {
	const table: Array<[query: string, text: string, ranges: Array<[number, number]>]> = [
		['new', 'New Folder', [[0, 3]]],
		[
			'nf',
			'New Folder',
			[
				[0, 1],
				[4, 5],
			],
		],
		['fol', 'New Folder', [[4, 7]]],
		['tr', 'Move to Trash', [[8, 10]]],
		[
			'mt',
			'Move to Trash',
			[
				[0, 1],
				[5, 6],
			],
		],
		[
			'ab',
			'a b',
			[
				[0, 1],
				[2, 3],
			],
		],
	];
	for (const [query, text, ranges] of table) {
		it(`${JSON.stringify(query)} in ${JSON.stringify(text)}`, () => {
			expect(fuzzyMatch(query, text)?.ranges).toEqual(ranges);
		});
	}

	it('prefers the aligned word start over an earlier scattered one', () => {
		// The "t" of "to" and the "T" of "Trash" are word starts; the "r" inside "Trash" continues the run.
		const match = fuzzyMatch('tr', 'Move to Trash');
		expect(match?.ranges).toEqual([[8, 10]]);
	});

	it('cuts text into the runs a renderer draws', () => {
		expect(
			splitByRanges('New Folder', [
				[0, 1],
				[4, 5],
			]),
		).toEqual([
			{ text: 'N', match: true },
			{ text: 'ew ', match: false },
			{ text: 'F', match: true },
			{ text: 'older', match: false },
		]);
		expect(splitByRanges('abc', [])).toEqual([{ text: 'abc', match: false }]);
	});
});

describe('scoring', () => {
	const score = (query: string, text: string) => fuzzyMatch(query, text)!.score;

	it('scores a word-boundary match above a mid-word one', () => {
		expect(score('tr', 'Move to Trash')).toBeGreaterThan(score('tr', 'Entry'));
		expect(score('f', 'New Folder')).toBeGreaterThan(score('o', 'New Folder'));
	});

	it('scores a consecutive run above scattered characters', () => {
		expect(score('fold', 'xfolder')).toBeGreaterThan(score('fold', 'xfxoxlxd'));
	});

	it('scores the start of the text above a later word', () => {
		expect(score('new', 'New Folder')).toBeGreaterThan(score('new', 'Rename (new)'));
	});

	it('scores a prefix above the same letters further in, and the whole text above a prefix', () => {
		expect(score('undo', 'Undo: Move')).toBeGreaterThan(score('undo', 'Redo and Undo'));
		expect(score('undo', 'Undo')).toBeGreaterThan(score('undo', 'Undo: Move'));
	});

	it('scores a shorter gap above a longer one', () => {
		expect(score('ab', 'a-b')).toBeGreaterThan(score('ab', 'a-----b'));
	});

	it('treats a camel-case hump as a word start', () => {
		expect(score('fd', 'findDuplicates')).toBeGreaterThan(score('fd', 'fenced'));
	});

	it('gives equal texts equal scores, so a stable sort keeps their order', () => {
		expect(score('new', 'New Tab')).toBe(score('new', 'New Tab'));
	});
});
