// Test helper: every shape in an outline icon is either marked to take the Filled style or is an open stroke
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { expect } from 'vitest';

type SubpathKind = 'closed' | 'line' | 'open';

/** How each subpath of a path's `d` ends up when filled: closed, a single straight segment (it has no area) or an open curve or polyline (it would fill as a wedge). */
export function subpathKinds(d: string): SubpathKind[] {
	const kinds: SubpathKind[] = [];
	let drawing = 0;
	let straight = true;
	let closed = false;
	let started = false;
	const flush = () => {
		if (!started) return;
		kinds.push(closed ? 'closed' : drawing <= 1 && straight ? 'line' : 'open');
	};
	for (const [text, letter] of d.matchAll(
		/([MmLlHhVvCcSsQqTtAaZz])[^MmLlHhVvCcSsQqTtAaZz]*/g,
	) as Iterable<[string, string]>) {
		const command = text;
		if (letter === 'M' || letter === 'm') {
			flush();
			started = true;
			drawing = 0;
			straight = true;
			closed = false;
		} else if (letter === 'Z' || letter === 'z') {
			closed = true;
		} else {
			// A command may repeat for each further set of numbers (`l4 4 4-4` is two segments).
			const numbers = (command.match(/[-+]?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?/g) ?? []).length;
			if ('LlHhVv'.includes(letter)) drawing += 'Hh Vv'.includes(letter) ? numbers : numbers / 2;
			else straight = false;
		}
	}
	flush();
	return kinds;
}

const CLOSED_ELEMENTS = new Set(['rect', 'circle', 'ellipse', 'polygon']);

/** The problems with one drawn element, empty when it is fine. */
function problems(element: Element): string[] {
	const tag = element.tagName.toLowerCase();
	if (tag === 'svg' || tag === 'g' || tag === 'defs' || tag === 'title') return [];
	// `data-outline` on an element or any ancestor says the closed shapes below are deliberately never filled.
	if (element.closest('[data-outline]')) return [];
	const fillable = element.hasAttribute('data-fill');
	// A shape the glyph fills itself (solid dots, a knocked-out half) is not the style's to fill.
	const own = element.closest('[fill]')?.getAttribute('fill');
	const solid = own !== undefined && own !== null && own !== 'none';
	if (solid) return [];
	if (tag === 'path') {
		const kinds = subpathKinds(element.getAttribute('d') ?? '');
		if (fillable) {
			return kinds.includes('open')
				? [
						`a fillable path has an open curve that would fill as a wedge: ${element.getAttribute('d')}`,
					]
				: [];
		}
		return kinds.includes('closed')
			? [`a closed path is not marked data-fill: ${element.getAttribute('d')}`]
			: [];
	}
	if (CLOSED_ELEMENTS.has(tag)) {
		return fillable ? [] : [`a <${tag}> is not marked data-fill`];
	}
	// Lines and polylines are open strokes whatever they hold.
	return fillable ? [`<${tag}> is an open stroke and is marked data-fill`] : [];
}

/** Every problem across the elements in `markup` (an `<svg>` or several), as readable lines. */
export function fillProblems(markup: string): string[] {
	const host = document.createElement('div');
	host.innerHTML = markup;
	return [...host.querySelectorAll('*')].flatMap(problems);
}

/**
 * Fails, listing the shapes, unless every element of every icon is marked `data-fill` (a closed
 * shape the Filled style tints) or is an open stroke (a chevron, an arrow, a check, a line), or sits
 * under `data-outline`. A new icon cannot ship without choosing, so Filled never draws a wedge.
 */
export function expectFillableIcons(icons: Record<string, string>): void {
	const entries = Object.entries(icons);
	expect(entries.length).toBeGreaterThan(0);
	const found: Record<string, string[]> = {};
	for (const [name, markup] of entries) {
		expect(markup, name).toContain('<svg');
		const issues = fillProblems(markup);
		if (issues.length > 0) found[name] = issues;
	}
	expect(found).toEqual({});
}
