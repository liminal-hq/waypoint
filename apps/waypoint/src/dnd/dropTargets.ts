// Where a file drag can land: the data attributes that mark a drop target, and the hit test that reads them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Point } from './dragSession';

/** What a marked element is: where a release over it would put the files. */
export type DropKind =
	/** A folder row in a list or grid: drop into that folder. */
	| 'folder'
	/** A pane's file area: drop into its current folder. */
	| 'pane'
	/** A sidebar place, favourite or folder: drop into that folder. */
	| 'place'
	/** The sidebar's Trash: drop moves to the Trash. */
	| 'trash'
	/** A breadcrumb segment: drop into that folder. */
	| 'crumb'
	/** A tab: drop into that tab's current folder. */
	| 'tab'
	/** The + button: drop opens the folders in new tabs. */
	| 'plus'
	/** A group chip: drop opens the folders as new tabs in that group. */
	| 'chip';

export const DROP_KINDS: readonly DropKind[] = [
	'folder',
	'pane',
	'place',
	'trash',
	'crumb',
	'tab',
	'plus',
	'chip',
];

/** The attributes a view puts on an element to make it a target; `data-drop` is the kind. */
export const DROP_ATTRIBUTE = 'data-drop';
const REF = 'data-drop-ref';
const LABEL = 'data-drop-label';
const READ_ONLY = 'data-drop-readonly';
const UNAVAILABLE = 'data-drop-unavailable';
/** Marks the element that scrolls a list, so a drag near its edge scrolls it. */
export const SCROLL_ATTRIBUTE = 'data-drop-scroll';
/** What the hit test writes on the element under the pointer; the stylesheet draws it. */
export const OVER_ATTRIBUTE = 'data-drop-over';
export const SPRING_ATTRIBUTE = 'data-drop-spring';

export interface DropAttributeOptions {
	/** The folder cannot be written to, so nothing can be dropped on it. */
	readOnly?: boolean | undefined;
	/** The target is shown but cannot be used (a Trash that cannot be read). */
	unavailable?: boolean | undefined;
}

/**
 * The props that make an element a drop target. `ref` says which one (a tab id, a `uri`, a
 * `handle:entry`), `label` is what the pill calls it. Spread them on the element.
 */
export function dropAttributes(
	kind: DropKind,
	ref: string | number,
	label: string,
	options: DropAttributeOptions = {},
): Record<string, string> {
	// Flags that are off are left out, not set to `undefined`, so a spread after this cannot be wiped by one.
	return {
		[DROP_ATTRIBUTE]: kind,
		[REF]: String(ref),
		[LABEL]: label,
		...(options.readOnly ? { [READ_ONLY]: '' } : {}),
		...(options.unavailable ? { [UNAVAILABLE]: '' } : {}),
	};
}

/** A target found under the pointer. */
export interface DropSpot {
	kind: DropKind;
	ref: string;
	label: string;
	readOnly: boolean;
	unavailable: boolean;
	element: HTMLElement;
	/** The pane (tab id) the element sits in, when it sits in one. */
	pane: number | null;
	/** The element is in the tab strip (`data-drop-strip`, which wraps the tabs, the arrows and the + button). */
	inStrip: boolean;
	/** The list or grid that scrolls under the pointer, if any. */
	scroller: HTMLElement | null;
}

/** The part of the document the hit test needs, so a test can stand one in. */
export interface HitRoot {
	elementsFromPoint(x: number, y: number): Element[];
}

function isKind(value: string | null): value is DropKind {
	return value !== null && (DROP_KINDS as readonly string[]).includes(value);
}

/**
 * The innermost marked element under `point`, so a folder row wins over the pane it is in. The
 * drag's own ghost and pill are `pointer-events: none` and never hit.
 */
export function dropSpotAt(
	point: Point,
	root: HitRoot | undefined = typeof document === 'undefined' ? undefined : document,
): DropSpot | null {
	if (!root || typeof root.elementsFromPoint !== 'function') return null;
	const stack = root.elementsFromPoint(point.x, point.y);
	let scroller: HTMLElement | null = null;
	for (const hit of stack) {
		if (!(hit instanceof HTMLElement)) continue;
		scroller ??= hit.closest<HTMLElement>(`[${SCROLL_ATTRIBUTE}]`);
	}
	for (const hit of stack) {
		if (!(hit instanceof HTMLElement)) continue;
		const element = hit.closest<HTMLElement>(`[${DROP_ATTRIBUTE}]`);
		const kind = element?.getAttribute(DROP_ATTRIBUTE) ?? null;
		if (!element || !isKind(kind)) continue;
		const pane = element.closest<HTMLElement>('[data-pane]')?.getAttribute('data-pane') ?? null;
		return {
			kind,
			ref: element.getAttribute(REF) ?? '',
			label: element.getAttribute(LABEL) ?? '',
			readOnly: element.hasAttribute(READ_ONLY),
			unavailable: element.hasAttribute(UNAVAILABLE),
			element,
			pane: pane === null || Number.isNaN(Number(pane)) ? null : Number(pane),
			inStrip: element.closest('[data-drop-strip]') !== null,
			scroller,
		};
	}
	return null;
}

/** A folder row's `ref`: its listing and its entry, which Rust turns into a location when asked. */
export function folderRef(handle: number, entry: number): string {
	return `${handle}:${entry}`;
}

export function parseFolderRef(ref: string): { handle: number; entry: number } | null {
	const [handle, entry] = ref.split(':').map(Number);
	return Number.isInteger(handle) && Number.isInteger(entry)
		? { handle: handle as number, entry: entry as number }
		: null;
}

/** Marks `next` as the element a release would act on (`ok` or `blocked`) and clears the one before it. */
export function markOver(
	previous: HTMLElement | null,
	next: HTMLElement | null,
	state: 'ok' | 'blocked' | null,
): void {
	if (previous && previous !== next) clearMark(previous);
	if (next && state) next.setAttribute(OVER_ATTRIBUTE, state);
	else if (next) clearMark(next);
}

export function clearMark(element: HTMLElement): void {
	element.removeAttribute(OVER_ATTRIBUTE);
	element.removeAttribute(SPRING_ATTRIBUTE);
	element.style.removeProperty('--wp-drop-spring-ms');
}

/** The width of the band along a list's top and bottom edge in which a drag scrolls it, and the fastest it scrolls per tick. */
export const EDGE_BAND_PX = 40;
export const EDGE_MAX_STEP_PX = 20;

/**
 * How far to scroll for a pointer at `y` over a scroller spanning `top` to `bottom`: negative
 * near the top, positive near the bottom, faster the closer to (or past) the edge, zero between.
 * A band wider than a third of the scroller shrinks, so a short list does not scroll everywhere.
 */
export function edgeScrollStep(top: number, bottom: number, y: number): number {
	const band = Math.min(EDGE_BAND_PX, Math.max(0, (bottom - top) / 3));
	if (band <= 0) return 0;
	if (y < top + band) {
		return -Math.ceil(EDGE_MAX_STEP_PX * Math.min(1, (top + band - y) / band));
	}
	if (y > bottom - band) {
		return Math.ceil(EDGE_MAX_STEP_PX * Math.min(1, (y - (bottom - band)) / band));
	}
	return 0;
}

/** The drop attributes of a listing's row: a folder (or a link to one) in a listing that can be dropped into; nothing otherwise. */
export function entryDropAttributes(
	entry: { id: number; name: string; kind: string; linkTarget: string | null },
	listing: { handle: number; readOnly: boolean; layout: string },
): Record<string, string | undefined> | undefined {
	const folder =
		entry.kind === 'directory' || (entry.kind === 'symlink' && entry.linkTarget === 'directory');
	if (!folder || listing.layout === 'trash') return undefined;
	return dropAttributes('folder', folderRef(listing.handle, entry.id), entry.name, {
		readOnly: listing.readOnly,
	});
}
