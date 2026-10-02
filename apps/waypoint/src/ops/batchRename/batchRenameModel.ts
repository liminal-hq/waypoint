// The batch rename dialog's pure model: the rule stack and how it is edited, the request a stack makes, and what the preview says
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { BatchPreview } from '@liminal-hq/waypoint-protocol/generated/BatchPreview';
import type { JobRequest } from '@liminal-hq/waypoint-protocol/generated/JobRequest';
import type { PreviewRow } from '@liminal-hq/waypoint-protocol/generated/PreviewRow';
import type { Problem } from '@liminal-hq/waypoint-protocol/generated/Problem';
import type { RenameRule } from '@liminal-hq/waypoint-protocol/generated/RenameRule';
import type { Sources } from '@liminal-hq/waypoint-protocol/generated/Sources';

export type RuleKind = RenameRule['kind'];

/** The rule types in the order the "add" list offers them. */
export const RULE_KINDS: readonly RuleKind[] = [
	'findReplace',
	'counter',
	'case',
	'dateToken',
	'insert',
	'remove',
	'trimWhitespace',
	'changeExtension',
];

/** How many rows of the preview table are drawn; the summary still counts them all. */
export const ROW_CAP = 500;

/** A rule with the identity that keeps its form (and its focus) in place as the stack is reordered. */
export interface KeyedRule {
	id: number;
	rule: RenameRule;
}

/** A new rule of `kind` with the values people usually start from. */
export function defaultRule(kind: RuleKind): RenameRule {
	switch (kind) {
		case 'findReplace':
			return {
				kind,
				find: '',
				replace: '',
				regex: false,
				caseSensitive: false,
				scope: 'stem',
				all: true,
			};
		case 'counter':
			return { kind, start: 1, step: 1, width: 2, position: 'suffix', separator: ' ' };
		case 'case':
			return { kind, mode: 'title', scope: 'stem' };
		case 'dateToken':
			return {
				kind,
				source: 'modified',
				format: '%Y-%m-%d',
				position: 'prefix',
				separator: ' ',
			};
		case 'insert':
			return { kind, text: '', at: { kind: 'end' } };
		case 'remove':
			return { kind, from: 0, to: 1 };
		case 'trimWhitespace':
			return { kind };
		case 'changeExtension':
			return { kind, to: '' };
	}
}

export function addRule(stack: readonly KeyedRule[], kind: RuleKind, id: number): KeyedRule[] {
	return [...stack, { id, rule: defaultRule(kind) }];
}

export function removeRule(stack: readonly KeyedRule[], id: number): KeyedRule[] {
	return stack.filter((entry) => entry.id !== id);
}

export function updateRule(stack: readonly KeyedRule[], id: number, rule: RenameRule): KeyedRule[] {
	return stack.map((entry) => (entry.id === id ? { id, rule } : entry));
}

/** Moves a rule one place earlier (`-1`) or later (`1`); the ends stay where they are. */
export function moveRule(stack: readonly KeyedRule[], id: number, by: -1 | 1): KeyedRule[] {
	const from = stack.findIndex((entry) => entry.id === id);
	const to = from + by;
	if (from < 0 || to < 0 || to >= stack.length) return [...stack];
	const next = [...stack];
	const [moved] = next.splice(from, 1);
	next.splice(to, 0, moved!);
	return next;
}

/**
 * The request for the preview and for the job: the same shape, so what is previewed is what is
 * submitted. `nowMs` is the time the preview used, which fixes what "today" means.
 */
export function buildRequest(
	sources: Sources,
	rules: readonly RenameRule[],
	utcOffsetMinutes: number,
	nowMs?: number,
): JobRequest {
	return {
		kind: { kind: 'batchRename' },
		sources,
		destination: null,
		name: null,
		options: { conflict: null, verify: null },
		// The plugin replaces this with the label of the window the request came from.
		originWindow: '',
		rename: {
			rules: [...rules],
			utcOffsetMinutes,
			...(nowMs === undefined ? {} : { nowMs }),
		},
	};
}

/** Local time's distance ahead of UTC, in minutes, for the dates the rules write. */
export function localUtcOffsetMinutes(date: Date = new Date()): number {
	// `getTimezoneOffset` counts minutes behind UTC, so east of Greenwich it is negative.
	return -date.getTimezoneOffset() || 0;
}

export function isBlocking(problem: Problem): boolean {
	return problem.kind !== 'unchangedSkip';
}

export function hasBlockingProblem(row: PreviewRow): boolean {
	return row.problems.some(isBlocking);
}

/** Whether Apply may be enabled: nothing is wrong and something changes. */
export function isReady(preview: BatchPreview | null): boolean {
	return preview !== null && preview.problems === 0 && preview.changes > 0;
}

/** Whether any entry's extension is not the one it had. */
export function changesAnExtension(preview: BatchPreview | null): boolean {
	return preview?.rows.some((row) => row.extensionChanged) ?? false;
}

/** The rows to draw, and how many were left out. */
export function visibleRows(
	preview: BatchPreview,
	cap: number = ROW_CAP,
): { rows: PreviewRow[]; hidden: number } {
	const rows = preview.rows.slice(0, cap);
	return { rows, hidden: preview.rows.length - rows.length };
}
