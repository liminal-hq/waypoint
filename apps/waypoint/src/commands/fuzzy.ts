// The palette's fuzzy matcher: a case-insensitive subsequence match scored for word starts and runs, with the ranges to highlight
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** A half-open run `[start, end)` of UTF-16 units in the matched text. */
export type MatchRange = readonly [start: number, end: number];

export interface FuzzyMatch {
	/** Higher is better; comparable only between matches of the same query. */
	score: number;
	/** The matched characters as runs, in order, for drawing them bold. */
	ranges: MatchRange[];
}

const POINT = 1;
const CONSECUTIVE = 8;
const WORD_START = 6;
const TEXT_START = 4;
const GAP = 1;
const GAP_CAP = 4;
const PREFIX = 12;
const WHOLE = 12;

/** A word starts after a separator, or where a lower-case letter is followed by an upper-case one. */
function startsWord(text: string, at: number): boolean {
	if (at === 0) return true;
	const before = text[at - 1]!;
	const here = text[at]!;
	if (/[\s\-_/.:…(),]/.test(before)) return true;
	return before !== before.toUpperCase() && here !== here.toLowerCase();
}

const same = (a: string, b: string) => a === b || a.toLowerCase() === b.toLowerCase();

/**
 * Matches `query` against `text` as a subsequence, ignoring case and the spaces of the query (so
 * "newf" and "new f" both find "New Folder"). The best-scoring alignment wins: each matched
 * character scores a point, with more for one that follows the previous match, one that starts a
 * word and one that starts the text, less a small cost per skipped character, and a bonus for a
 * match that is the whole start of the text or all of it. Returns `null` when the characters do
 * not occur in order; an empty query matches everything with a score of 0 and no ranges.
 */
export function fuzzyMatch(query: string, text: string): FuzzyMatch | null {
	const needle = [...query].filter((char) => !/\s/.test(char));
	if (needle.length === 0) return { score: 0, ranges: [] };
	const rows = needle.length;
	const length = text.length;
	if (rows > length) return null;

	// best[i][j]: the best score with needle[i] matched at text[j]; from[i][j]: where needle[i - 1] was.
	const best: number[][] = Array.from({ length: rows }, () =>
		new Array<number>(length).fill(-Infinity),
	);
	const from: number[][] = Array.from({ length: rows }, () => new Array<number>(length).fill(-1));

	const pointAt = (j: number) =>
		POINT + (startsWord(text, j) ? WORD_START : 0) + (j === 0 ? TEXT_START : 0);

	for (let j = 0; j < length; j++) {
		if (!same(needle[0]!, text[j]!)) continue;
		best[0]![j] = pointAt(j) - Math.min(j, GAP_CAP) * GAP;
	}
	for (let i = 1; i < rows; i++) {
		for (let j = i; j < length; j++) {
			if (!same(needle[i]!, text[j]!)) continue;
			let top = -Infinity;
			let origin = -1;
			for (let k = i - 1; k < j; k++) {
				const earlier = best[i - 1]![k]!;
				if (earlier === -Infinity) continue;
				const link = k === j - 1 ? CONSECUTIVE : -Math.min(j - k - 1, GAP_CAP) * GAP;
				if (earlier + link > top) {
					top = earlier + link;
					origin = k;
				}
			}
			if (origin >= 0) {
				best[i]![j] = top + pointAt(j);
				from[i]![j] = origin;
			}
		}
	}

	let end = -1;
	let score = -Infinity;
	for (let j = 0; j < length; j++) {
		const value = best[rows - 1]![j]!;
		// The earliest of equal ends wins, so ties keep the match toward the front.
		if (value > score) {
			score = value;
			end = j;
		}
	}
	if (end < 0) return null;

	const at: number[] = new Array<number>(rows);
	for (let i = rows - 1, j = end; i >= 0; i--) {
		at[i] = j;
		j = from[i]![j]!;
	}

	const ranges: Array<[number, number]> = [];
	for (const position of at) {
		const last = ranges[ranges.length - 1];
		if (last && last[1] === position) last[1] = position + 1;
		else ranges.push([position, position + 1]);
	}

	const lower = text.toLowerCase().replace(/\s/g, '');
	const joined = needle.join('').toLowerCase();
	if (lower.startsWith(joined)) score += lower === joined ? PREFIX + WHOLE : PREFIX;
	return { score, ranges };
}

/** `text` cut into the runs a renderer draws, each flagged when it is a matched one. */
export function splitByRanges(
	text: string,
	ranges: readonly MatchRange[],
): Array<{ text: string; match: boolean }> {
	const parts: Array<{ text: string; match: boolean }> = [];
	let cursor = 0;
	for (const [start, end] of ranges) {
		if (start > cursor) parts.push({ text: text.slice(cursor, start), match: false });
		parts.push({ text: text.slice(start, end), match: true });
		cursor = end;
	}
	if (cursor < text.length) parts.push({ text: text.slice(cursor), match: false });
	return parts;
}
