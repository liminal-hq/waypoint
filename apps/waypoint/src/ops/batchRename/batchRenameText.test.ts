// Tests for the words the dialog shows about the preview
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { problemText, rowNotes, summaryText } from './batchRenameText';
import { previewOf } from './fakeBatchRenameApi';

describe('problemText', () => {
	const rows = previewOf([
		{ from: 'a', to: 'x' },
		{ from: 'b', to: 'x', problems: [{ kind: 'duplicateTarget', with: 0 }] },
	]).rows;

	it('says why a name is not allowed in the words Rust gave', () => {
		expect(problemText({ kind: 'invalid', reason: 'a name cannot contain `/`' }, rows)).toBe(
			'a name cannot contain `/`',
		);
	});

	it('names the entry a duplicate clashes with', () => {
		expect(problemText({ kind: 'duplicateTarget', with: 0 }, rows)).toBe('The same name as “x”');
	});

	it('names the folder a nested entry is inside', () => {
		expect(problemText({ kind: 'nestedSelection', with: 0 }, rows)).toBe(
			'Inside “a”, which is also being renamed; select one or the other',
		);
	});

	it('says a name is taken, and that nothing changes', () => {
		expect(problemText({ kind: 'existsInFolder' }, rows)).toBe(
			'A file or folder with this name is already here',
		);
		expect(problemText({ kind: 'unchangedSkip' }, rows)).toBe('No change');
	});
});

describe('rowNotes', () => {
	it('labels a problem as one, so it never rests on colour', () => {
		const preview = previewOf([{ from: 'a', to: 'b', problems: [{ kind: 'existsInFolder' }] }]);
		expect(rowNotes(preview.rows[0]!, preview.rows)).toEqual([
			'Problem: A file or folder with this name is already here',
		]);
	});

	it('notes an unchanged entry and a changed extension', () => {
		const preview = previewOf([
			{ from: 'a', to: 'a', problems: [{ kind: 'unchangedSkip' }] },
			{ from: 'b.txt', to: 'b.md', extensionChanged: true },
			{ from: 'c', to: 'd' },
		]);
		expect(rowNotes(preview.rows[0]!, preview.rows)).toEqual(['No change']);
		expect(rowNotes(preview.rows[1]!, preview.rows)).toEqual(['The extension changes']);
		expect(rowNotes(preview.rows[2]!, preview.rows)).toEqual([]);
	});
});

describe('summaryText', () => {
	it('counts problems before anything else', () => {
		const preview = previewOf([
			{ from: 'a', to: 'b', problems: [{ kind: 'existsInFolder' }] },
			{ from: 'c', to: 'd', problems: [{ kind: 'existsInFolder' }] },
			{ from: 'e', to: 'f', problems: [{ kind: 'existsInFolder' }] },
			{ from: 'g', to: 'h' },
		]);
		expect(summaryText(preview)).toBe('3 problems');
		expect(
			summaryText(previewOf([{ from: 'a', to: 'b', problems: [{ kind: 'existsInFolder' }] }])),
		).toBe('1 problem');
	});

	it('counts what will be renamed, or says nothing changes', () => {
		expect(summaryText(previewOf([{ from: 'a', to: 'b' }]))).toBe('1 item will be renamed');
		expect(
			summaryText(
				previewOf([
					{ from: 'a', to: 'b' },
					{ from: 'c', to: 'd' },
				]),
			),
		).toBe('2 items will be renamed');
		expect(summaryText(previewOf([{ from: 'a', to: 'a' }]))).toBe('No names change');
	});
});
