// Tests for the batch rename model: the stack's edits, the request it makes and the preview's verdicts
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Sources } from '@liminal-hq/waypoint-protocol/generated/Sources';
import { describe, expect, it } from 'vitest';
import {
	addRule,
	buildRequest,
	changesAnExtension,
	defaultRule,
	hasBlockingProblem,
	isReady,
	localUtcOffsetMinutes,
	moveRule,
	removeRule,
	ROW_CAP,
	RULE_KINDS,
	updateRule,
	visibleRows,
	type KeyedRule,
} from './batchRenameModel';
import { previewOf } from './fakeBatchRenameApi';

const sources: Sources = {
	kind: 'locations',
	locations: [{ display: '/a', uri: 'file:///a' }],
};

const stackOf = (...ids: number[]): KeyedRule[] =>
	ids.map((id) => ({ id, rule: defaultRule('trimWhitespace') }));

describe('defaultRule', () => {
	it('makes a rule of each kind that names its own kind', () => {
		for (const kind of RULE_KINDS) expect(defaultRule(kind).kind).toBe(kind);
	});

	it('starts a find and replace on the stem, matching any case, with nothing to find', () => {
		expect(defaultRule('findReplace')).toMatchObject({
			find: '',
			scope: 'stem',
			caseSensitive: false,
			regex: false,
			all: true,
		});
	});

	it('numbers from 1 by 1 in two digits after the name', () => {
		expect(defaultRule('counter')).toMatchObject({
			start: 1,
			step: 1,
			width: 2,
			position: 'suffix',
		});
	});
});

describe('the rule stack', () => {
	it('adds a rule at the end with the id it is given', () => {
		const next = addRule(stackOf(1), 'case', 7);
		expect(next.map((e) => e.id)).toEqual([1, 7]);
		expect(next[1]!.rule.kind).toBe('case');
	});

	it('removes a rule by id and leaves the others in order', () => {
		expect(removeRule(stackOf(1, 2, 3), 2).map((e) => e.id)).toEqual([1, 3]);
		expect(removeRule(stackOf(1), 9).map((e) => e.id)).toEqual([1]);
	});

	it('replaces the rule of one entry and keeps its place and id', () => {
		const next = updateRule(stackOf(1, 2), 2, { kind: 'changeExtension', to: 'md' });
		expect(next.map((e) => e.id)).toEqual([1, 2]);
		expect(next[1]!.rule).toEqual({ kind: 'changeExtension', to: 'md' });
		expect(next[0]!.rule.kind).toBe('trimWhitespace');
	});

	it('moves a rule one place and stops at the ends', () => {
		expect(moveRule(stackOf(1, 2, 3), 3, -1).map((e) => e.id)).toEqual([1, 3, 2]);
		expect(moveRule(stackOf(1, 2, 3), 1, 1).map((e) => e.id)).toEqual([2, 1, 3]);
		expect(moveRule(stackOf(1, 2, 3), 1, -1).map((e) => e.id)).toEqual([1, 2, 3]);
		expect(moveRule(stackOf(1, 2, 3), 3, 1).map((e) => e.id)).toEqual([1, 2, 3]);
		expect(moveRule(stackOf(1, 2, 3), 9, 1).map((e) => e.id)).toEqual([1, 2, 3]);
	});

	it('does not change the stack it was given', () => {
		const stack = stackOf(1, 2);
		moveRule(stack, 1, 1);
		removeRule(stack, 1);
		expect(stack.map((e) => e.id)).toEqual([1, 2]);
	});
});

describe('buildRequest', () => {
	it('is a batch rename of the sources with the rules in order and no time fixed', () => {
		const rules = [defaultRule('case'), defaultRule('counter')];
		const request = buildRequest(sources, rules, -300);
		expect(request.kind).toEqual({ kind: 'batchRename' });
		expect(request.sources).toBe(sources);
		expect(request.rename?.rules).toEqual(rules);
		expect(request.rename?.utcOffsetMinutes).toBe(-300);
		expect(request.rename).not.toHaveProperty('nowMs');
		expect(request.options).toEqual({ conflict: null, verify: null });
	});

	it('carries the time the preview used', () => {
		expect(buildRequest(sources, [], 0, 1234).rename?.nowMs).toBe(1234);
	});
});

describe('localUtcOffsetMinutes', () => {
	it('is the time zone ahead of UTC, not behind it', () => {
		const east = { getTimezoneOffset: () => -120 } as Date;
		const west = { getTimezoneOffset: () => 300 } as Date;
		expect(localUtcOffsetMinutes(east)).toBe(120);
		expect(localUtcOffsetMinutes(west)).toBe(-300);
		expect(Object.is(localUtcOffsetMinutes({ getTimezoneOffset: () => 0 } as Date), 0)).toBe(true);
	});
});

describe('what a preview allows', () => {
	it('is ready only with something to change and nothing wrong', () => {
		expect(isReady(null)).toBe(false);
		expect(isReady(previewOf([{ from: 'a', to: 'a' }]))).toBe(false);
		expect(isReady(previewOf([{ from: 'a', to: 'b' }]))).toBe(true);
		const clash = previewOf([{ from: 'a', to: 'b', problems: [{ kind: 'existsInFolder' }] }]);
		expect(isReady(clash)).toBe(false);
		expect(
			isReady(previewOf([{ from: 'a', to: 'b' }], { ruleErrors: [{ rule: 0, reason: 'x' }] })),
		).toBe(false);
	});

	it('does not count a skipped entry as a problem', () => {
		const skipped = previewOf([
			{ from: 'a', to: 'a', problems: [{ kind: 'unchangedSkip' }] },
			{ from: 'b', to: 'c' },
		]);
		expect(skipped.problems).toBe(0);
		expect(hasBlockingProblem(skipped.rows[0]!)).toBe(false);
		expect(isReady(skipped)).toBe(true);
	});

	it('knows when an extension changes', () => {
		expect(changesAnExtension(null)).toBe(false);
		expect(changesAnExtension(previewOf([{ from: 'a.txt', to: 'a.md' }]))).toBe(false);
		expect(
			changesAnExtension(previewOf([{ from: 'a.txt', to: 'a.md', extensionChanged: true }])),
		).toBe(true);
	});

	it('draws at most the cap and counts the rest', () => {
		const rows = Array.from({ length: ROW_CAP + 30 }, (_, i) => ({
			from: `f${i}`,
			to: `g${i}`,
		}));
		const shown = visibleRows(previewOf(rows));
		expect(shown.rows).toHaveLength(ROW_CAP);
		expect(shown.hidden).toBe(30);
		expect(visibleRows(previewOf(rows.slice(0, 3))).hidden).toBe(0);
	});
});
