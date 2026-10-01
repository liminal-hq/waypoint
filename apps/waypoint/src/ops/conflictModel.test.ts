// Verifies which choices are legal for each kind of clash, how a batch is covered and the answer that is sent
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { conflictFor } from '../test/opsHarness';
import {
	NO_ANSWERS,
	buildAnswer,
	bulkChoices,
	conflictsKey,
	coverage,
	dateHint,
	effectivePolicy,
	isLegal,
	legalChoices,
	type Answers,
} from './conflictModel';

const answers = (
	rows: Record<string, 'replace' | 'skip' | 'keepBoth'>,
	rest: Partial<Answers> = {},
) =>
	({
		rows: new Map(Object.entries(rows).map(([name, policy]) => [`file:///src/${name}`, policy])),
		bulk: null,
		later: false,
		...rest,
	}) satisfies Answers;

describe('legalChoices', () => {
	it('offers four choices for a file over a file and four for a folder over a folder', () => {
		expect(legalChoices('fileOverFile')).toEqual(['replace', 'skip', 'keepBoth', 'replaceIfNewer']);
		expect(legalChoices('folderOverFolder')).toEqual([
			'mergeFolders',
			'skip',
			'keepBoth',
			'replace',
		]);
	});

	it('allows only Skip and Keep both when a file and a folder share a name', () => {
		for (const kind of ['fileOverFolder', 'folderOverFile'] as const) {
			expect(legalChoices(kind)).toEqual(['skip', 'keepBoth']);
			expect(isLegal('replace', kind)).toBe(false);
			expect(isLegal('mergeFolders', kind)).toBe(false);
		}
	});

	it('does not merge files or replace-if-newer folders', () => {
		expect(isLegal('mergeFolders', 'fileOverFile')).toBe(false);
		expect(isLegal('replaceIfNewer', 'folderOverFolder')).toBe(false);
	});
});

describe('dateHint', () => {
	it('says whether the incoming entry is newer, older or the same', () => {
		expect(dateHint(conflictFor('a'))).toBe('newer');
		expect(dateHint(conflictFor('a', { sourceModifiedMs: 1, existingModifiedMs: 5 }))).toBe(
			'older',
		);
		expect(dateHint(conflictFor('a', { sourceModifiedMs: 5, existingModifiedMs: 5 }))).toBe('same');
	});

	it('says nothing when a date is unknown or nothing exists yet', () => {
		expect(dateHint(conflictFor('a', { existingModifiedMs: null }))).toBe('unknown');
		expect(dateHint(conflictFor('a', { withinBatch: true }))).toBe('unknown');
		expect(dateHint(conflictFor('a', { kind: 'folderOverFolder' }))).toBe('unknown');
	});
});

describe('coverage', () => {
	const conflicts = [
		conflictFor('a'),
		conflictFor('b'),
		conflictFor('c', { kind: 'fileOverFolder' }),
	];

	it('starts with nothing answered', () => {
		const stats = coverage(conflicts, NO_ANSWERS);
		expect(stats).toMatchObject({ answered: 0, total: 3, open: 3, coveredByBulk: 0 });
	});

	it('counts rows answered one by one', () => {
		const stats = coverage(conflicts, answers({ a: 'skip', b: 'replace' }));
		expect(stats.answered).toBe(2);
		expect(stats.byPolicy).toMatchObject({ skip: 1, replace: 1 });
	});

	it('lets a bulk choice reach only the clashes it is legal for', () => {
		const withBulk = answers({}, { bulk: 'replace' });
		const stats = coverage(conflicts, withBulk);
		expect(stats).toMatchObject({ answered: 2, open: 3, coveredByBulk: 2 });
		expect(effectivePolicy(conflicts[2]!, withBulk)).toBeNull();
	});

	it('prefers a row’s own answer to the bulk choice', () => {
		const mixed = answers({ a: 'keepBoth' }, { bulk: 'skip' });
		expect(effectivePolicy(conflicts[0]!, mixed)).toBe('keepBoth');
		expect(effectivePolicy(conflicts[1]!, mixed)).toBe('skip');
	});
});

describe('bulkChoices', () => {
	it('offers the choices some unanswered clash can use', () => {
		expect(bulkChoices([conflictFor('a')], NO_ANSWERS)).toEqual([
			'replace',
			'skip',
			'keepBoth',
			'replaceIfNewer',
		]);
		expect(bulkChoices([conflictFor('a', { kind: 'fileOverFolder' })], NO_ANSWERS)).toEqual([
			'skip',
			'keepBoth',
		]);
	});

	it('looks only at the clashes still without an answer', () => {
		const conflicts = [conflictFor('a'), conflictFor('b', { kind: 'fileOverFolder' })];
		expect(bulkChoices(conflicts, answers({ a: 'skip' }))).toEqual(['skip', 'keepBoth']);
	});
});

describe('buildAnswer', () => {
	const conflicts = [conflictFor('a'), conflictFor('b')];

	it('is null while a clash has no answer', () => {
		expect(buildAnswer(conflicts, answers({ a: 'skip' }))).toBeNull();
	});

	it('sends a decision for each row answered', () => {
		const result = buildAnswer(conflicts, answers({ a: 'skip', b: 'keepBoth' }));
		expect(result).toEqual({
			decisions: [
				{ source: conflicts[0]!.source, policy: 'skip' },
				{ source: conflicts[1]!.source, policy: 'keepBoth' },
			],
		});
	});

	it('spells a bulk choice out per clash when it is not to apply later', () => {
		const result = buildAnswer(conflicts, answers({}, { bulk: 'skip' }));
		expect(result?.applyToAll).toBeUndefined();
		expect(result?.decisions).toHaveLength(2);
	});

	it('sends a bulk choice as the job’s own when it applies later, with only the rows that differ', () => {
		const result = buildAnswer(
			conflicts,
			answers({ a: 'keepBoth' }, { bulk: 'skip', later: true }),
		);
		expect(result).toEqual({
			decisions: [{ source: conflicts[0]!.source, policy: 'keepBoth' }],
			applyToAll: 'skip',
		});
	});
});

describe('conflictsKey', () => {
	it('changes with the set of clashes, not with their order of arrival of equal ones', () => {
		expect(conflictsKey([conflictFor('a')])).toBe(conflictsKey([conflictFor('a')]));
		expect(conflictsKey([conflictFor('a')])).not.toBe(conflictsKey([conflictFor('b')]));
	});
});
