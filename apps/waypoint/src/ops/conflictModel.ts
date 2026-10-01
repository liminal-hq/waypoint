// The conflict resolver's rules: which choices are legal for a clash, how a batch is covered, and the answer it sends
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Conflict } from '@liminal-hq/waypoint-protocol/generated/Conflict';
import type { ConflictKind } from '@liminal-hq/waypoint-protocol/generated/ConflictKind';
import type { ConflictPolicy } from '@liminal-hq/waypoint-protocol/generated/ConflictPolicy';
import type { Resolution } from '@liminal-hq/waypoint-protocol/generated/Resolution';

/** How many rows the dialog draws before it offers "Show all". */
export const ROW_CAP = 200;

/** The order choices are listed in. */
export const POLICY_ORDER: readonly ConflictPolicy[] = [
	'replace',
	'skip',
	'keepBoth',
	'mergeFolders',
	'replaceIfNewer',
];

/**
 * The choices that can settle a clash of `kind`. The engine refuses `Replace` for a file against a
 * folder and ignores `Merge folders` and `Replace if newer` where they do not apply, so the dialog
 * offers only what works. Folders list Merge first, files list Replace first.
 */
export function legalChoices(kind: ConflictKind): readonly ConflictPolicy[] {
	switch (kind) {
		case 'fileOverFile':
			return ['replace', 'skip', 'keepBoth', 'replaceIfNewer'];
		case 'folderOverFolder':
			return ['mergeFolders', 'skip', 'keepBoth', 'replace'];
		case 'fileOverFolder':
		case 'folderOverFile':
			return ['skip', 'keepBoth'];
	}
}

export function isLegal(policy: ConflictPolicy, kind: ConflictKind): boolean {
	return legalChoices(kind).includes(policy);
}

/** Whether the existing entry is a folder (what a replace would remove). */
export function existingIsFolder(kind: ConflictKind): boolean {
	return kind === 'folderOverFolder' || kind === 'fileOverFolder';
}

/** Whether the choice destroys something the person has: only `Replace` does. */
export function isDestructive(policy: ConflictPolicy): boolean {
	return policy === 'replace';
}

export type DateHint = 'newer' | 'older' | 'same' | 'unknown';

/**
 * How the incoming entry's date compares with the existing one's. Only files are compared: a
 * folder's date moves whenever something inside it does, so it says nothing about which is newer.
 */
export function dateHint(conflict: Conflict): DateHint {
	const { sourceModifiedMs: incoming, existingModifiedMs: existing } = conflict;
	if (conflict.kind !== 'fileOverFile' || conflict.withinBatch) return 'unknown';
	if (incoming === null || existing === null) return 'unknown';
	if (incoming > existing) return 'newer';
	if (incoming < existing) return 'older';
	return 'same';
}

/** What the person has answered so far. */
export interface Answers {
	/** The choice for each clash, by the source's URI. */
	rows: ReadonlyMap<string, ConflictPolicy>;
	/** The "apply to all remaining" choice, or `null`. */
	bulk: ConflictPolicy | null;
	/** Whether the bulk choice also settles clashes the job meets later. */
	later: boolean;
}

export const NO_ANSWERS: Answers = { rows: new Map(), bulk: null, later: false };

/** The choice that settles `conflict`: its own, or the bulk one where that is legal for it. */
export function effectivePolicy(conflict: Conflict, answers: Answers): ConflictPolicy | null {
	const own = answers.rows.get(conflict.source.uri);
	if (own) return own;
	if (answers.bulk && isLegal(answers.bulk, conflict.kind)) return answers.bulk;
	return null;
}

export interface Coverage {
	answered: number;
	total: number;
	/** The clashes with an answer of their own or a bulk one, counted per choice. */
	byPolicy: Record<ConflictPolicy, number>;
	/** Clashes with no answer of their own, and how many of those the bulk choice reaches. */
	open: number;
	coveredByBulk: number;
}

export function coverage(conflicts: readonly Conflict[], answers: Answers): Coverage {
	const byPolicy: Record<ConflictPolicy, number> = {
		replace: 0,
		skip: 0,
		keepBoth: 0,
		mergeFolders: 0,
		replaceIfNewer: 0,
	};
	let answered = 0;
	let open = 0;
	let coveredByBulk = 0;
	for (const conflict of conflicts) {
		const own = answers.rows.has(conflict.source.uri);
		if (!own) {
			open += 1;
			if (answers.bulk && isLegal(answers.bulk, conflict.kind)) coveredByBulk += 1;
		}
		const policy = effectivePolicy(conflict, answers);
		if (policy) {
			answered += 1;
			byPolicy[policy] += 1;
		}
	}
	return { answered, total: conflicts.length, byPolicy, open, coveredByBulk };
}

/** The choices the bulk bar offers: those that are legal for at least one clash without an answer. */
export function bulkChoices(conflicts: readonly Conflict[], answers: Answers): ConflictPolicy[] {
	const open = conflicts.filter((conflict) => !answers.rows.has(conflict.source.uri));
	const pool = open.length > 0 ? open : conflicts;
	return POLICY_ORDER.filter((policy) => pool.some((conflict) => isLegal(policy, conflict.kind)));
}

/** A key that changes when the set of clashes does, so a dialog starts afresh for new ones. */
export function conflictsKey(conflicts: readonly Conflict[]): string {
	return conflicts.map((conflict) => conflict.source.uri).join('\n');
}

/**
 * The answer to send, or `null` while some clash has none. Clashes with an answer of their own
 * get a decision each. A bulk choice that also applies later goes as the job's choice for
 * everything else; one that does not is spelled out per clash, so the job asks again about
 * anything it meets later.
 */
export function buildAnswer(
	conflicts: readonly Conflict[],
	answers: Answers,
): { decisions: Resolution[]; applyToAll?: ConflictPolicy } | null {
	const decisions: Resolution[] = [];
	for (const conflict of conflicts) {
		const own = answers.rows.get(conflict.source.uri);
		const policy = effectivePolicy(conflict, answers);
		if (!policy) return null;
		if (own || !answers.later) decisions.push({ source: conflict.source, policy });
	}
	return answers.later && answers.bulk ? { decisions, applyToAll: answers.bulk } : { decisions };
}
