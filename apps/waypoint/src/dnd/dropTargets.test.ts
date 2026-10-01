// Verifies the drop target attributes, the hit test that reads them, the marks and the edge scroll
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, describe, expect, it } from 'vitest';
import {
	clearMark,
	dropAttributes,
	dropSpotAt,
	DROP_KINDS,
	EDGE_BAND_PX,
	EDGE_MAX_STEP_PX,
	edgeScrollStep,
	entryDropAttributes,
	folderRef,
	markOver,
	OVER_ATTRIBUTE,
	parseFolderRef,
	SPRING_ATTRIBUTE,
	type HitRoot,
} from './dropTargets';

afterEach(() => {
	document.body.innerHTML = '';
});

/** An element carrying the attributes, appended to the body. */
function make(
	attributes: Record<string, string>,
	parent: HTMLElement = document.body,
	tag = 'div',
): HTMLElement {
	const element = document.createElement(tag);
	for (const [name, value] of Object.entries(attributes)) element.setAttribute(name, value);
	parent.appendChild(element);
	return element;
}

/** A hit root that reports `stack` (topmost first) wherever it is asked. */
const hitting = (...stack: Element[]): HitRoot => ({ elementsFromPoint: () => stack });

describe('dropAttributes', () => {
	it('names the kind, the ref and the label, and leaves out flags that are off', () => {
		expect(dropAttributes('place', 'file:///home', 'Home')).toEqual({
			'data-drop': 'place',
			'data-drop-ref': 'file:///home',
			'data-drop-label': 'Home',
		});
		expect(
			dropAttributes('folder', '1:2', 'docs', { readOnly: true, unavailable: true }),
		).toMatchObject({ 'data-drop-readonly': '', 'data-drop-unavailable': '' });
	});

	it('has a kind for every target the engine knows', () => {
		expect([...DROP_KINDS].sort()).toEqual(
			['chip', 'crumb', 'folder', 'pane', 'place', 'plus', 'tab', 'trash'].sort(),
		);
	});
});

describe('dropSpotAt', () => {
	it('finds the innermost marked element, so a folder row wins over its pane', () => {
		const pane = make({ ...dropAttributes('pane', 4, 'docs'), 'data-pane': '4' });
		const row = make(dropAttributes('folder', folderRef(7, 3), 'music'), pane);
		const cell = make({}, row);
		const spot = dropSpotAt({ x: 1, y: 1 }, hitting(cell, row, pane, document.body));
		expect(spot).toMatchObject({ kind: 'folder', ref: '7:3', label: 'music', pane: 4 });
		expect(spot?.element).toBe(row);
	});

	it('falls back to the pane when the pointer is over a plain row', () => {
		const pane = make({ ...dropAttributes('pane', 4, 'docs'), 'data-pane': '4' });
		const row = make({}, pane);
		expect(dropSpotAt({ x: 0, y: 0 }, hitting(row, pane))).toMatchObject({ kind: 'pane', pane: 4 });
	});

	it('reports read-only and unavailable targets, the strip and the scroller', () => {
		const strip = make({ 'data-drop-strip': '' });
		const trash = make(dropAttributes('trash', 'trash:///', 'Trash', { unavailable: true }), strip);
		const scroller = make({ 'data-drop-scroll': '' });
		const readOnly = make(dropAttributes('folder', '1:1', 'a', { readOnly: true }), scroller);
		expect(dropSpotAt({ x: 0, y: 0 }, hitting(trash))).toMatchObject({
			kind: 'trash',
			unavailable: true,
			inStrip: true,
			scroller: null,
		});
		const spot = dropSpotAt({ x: 0, y: 0 }, hitting(readOnly, scroller));
		expect(spot).toMatchObject({ readOnly: true, inStrip: false });
		expect(spot?.scroller).toBe(scroller);
	});

	it('is null over nothing marked, over an unknown kind, and without a document', () => {
		expect(dropSpotAt({ x: 0, y: 0 }, hitting(make({})))).toBeNull();
		expect(dropSpotAt({ x: 0, y: 0 }, hitting(make({ 'data-drop': 'bogus' })))).toBeNull();
		expect(
			dropSpotAt({ x: 0, y: 0 }, { elementsFromPoint: undefined } as unknown as HitRoot),
		).toBeNull();
	});

	it('reads the document by default', () => {
		const target = make(dropAttributes('plus', 'new', 'New tab'));
		document.elementsFromPoint = () => [target];
		expect(dropSpotAt({ x: 1, y: 1 })?.kind).toBe('plus');
		// @ts-expect-error restoring jsdom's lack of it
		delete document.elementsFromPoint;
	});
});

describe('folder refs', () => {
	it('round trip, and refuse what is not one', () => {
		expect(parseFolderRef(folderRef(12, 34))).toEqual({ handle: 12, entry: 34 });
		expect(parseFolderRef('x')).toBeNull();
		expect(parseFolderRef('1:y')).toBeNull();
	});
});

describe('entryDropAttributes', () => {
	const listing = { handle: 9, readOnly: false, layout: 'folder' };
	it('marks a folder or a link to one, and nothing else', () => {
		const dir = { id: 1, name: 'd', kind: 'directory', linkTarget: null };
		const link = { id: 2, name: 'l', kind: 'symlink', linkTarget: 'directory' };
		const brokenLink = { id: 3, name: 'b', kind: 'symlink', linkTarget: 'file' };
		const file = { id: 4, name: 'f', kind: 'file', linkTarget: null };
		expect(entryDropAttributes(dir, listing)).toMatchObject({ 'data-drop-ref': '9:1' });
		expect(entryDropAttributes(link, listing)).toMatchObject({ 'data-drop-ref': '9:2' });
		expect(entryDropAttributes(brokenLink, listing)).toBeUndefined();
		expect(entryDropAttributes(file, listing)).toBeUndefined();
	});

	it('carries the listing being read-only, and marks nothing in the Trash', () => {
		const dir = { id: 1, name: 'd', kind: 'directory', linkTarget: null };
		expect(entryDropAttributes(dir, { ...listing, readOnly: true })).toMatchObject({
			'data-drop-readonly': '',
		});
		expect(entryDropAttributes(dir, { ...listing, layout: 'trash' })).toBeUndefined();
	});
});

describe('marks', () => {
	it('mark one element, move the mark and clear the spring with it', () => {
		const a = make({});
		const b = make({});
		markOver(null, a, 'ok');
		expect(a.getAttribute(OVER_ATTRIBUTE)).toBe('ok');
		a.setAttribute(SPRING_ATTRIBUTE, '');
		a.style.setProperty('--wp-drop-spring-ms', '600ms');
		markOver(a, b, 'blocked');
		expect(a.hasAttribute(OVER_ATTRIBUTE)).toBe(false);
		expect(a.hasAttribute(SPRING_ATTRIBUTE)).toBe(false);
		expect(a.style.getPropertyValue('--wp-drop-spring-ms')).toBe('');
		expect(b.getAttribute(OVER_ATTRIBUTE)).toBe('blocked');
		markOver(b, b, null);
		expect(b.hasAttribute(OVER_ATTRIBUTE)).toBe(false);
		clearMark(b);
	});
});

describe('edgeScrollStep', () => {
	it('is zero in the middle, negative near the top, positive near the bottom', () => {
		expect(edgeScrollStep(0, 400, 200)).toBe(0);
		expect(edgeScrollStep(0, 400, 5)).toBeLessThan(0);
		expect(edgeScrollStep(0, 400, 395)).toBeGreaterThan(0);
	});

	it('speeds up toward the edge and tops out, even past it', () => {
		const nearer = Math.abs(edgeScrollStep(0, 400, 10));
		const farther = Math.abs(edgeScrollStep(0, 400, EDGE_BAND_PX - 5));
		expect(nearer).toBeGreaterThan(farther);
		expect(edgeScrollStep(0, 400, -50)).toBe(-EDGE_MAX_STEP_PX);
		expect(edgeScrollStep(0, 400, 500)).toBe(EDGE_MAX_STEP_PX);
	});

	it('shrinks the band for a short list, so it does not scroll everywhere', () => {
		expect(edgeScrollStep(0, 60, 30)).toBe(0);
		expect(edgeScrollStep(0, 0, 0)).toBe(0);
	});
});
