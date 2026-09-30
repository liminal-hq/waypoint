// Guards the 28 px minimum hit target for the title bar's window buttons on every desktop style
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

// Read the real file from disk: Vitest swaps CSS for empty text, so importing it would make the
// assertions below pass vacuously.
const css = readFileSync(join(import.meta.dirname, 'TitleBar.module.css'), 'utf8');

/** SPEC.md: hit targets are at least 28 px on desktop. */
const MINIMUM_HIT_TARGET = 28;

/** The pixel size the last matching rule gives a style's `.button` for a property, in source order. */
function buttonSize(style: string, property: 'width' | 'height'): number | undefined {
	let size: number | undefined;
	for (const [, selectors, body] of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
		const targetsButton = selectors!
			.split(',')
			.some(
				(selector) =>
					selector.includes(`data-controls-style='${style}'`) &&
					/\.button\s*$/.test(selector.trim()),
			);
		if (!targetsButton) continue;
		const declared = body!.match(new RegExp(`(?:^|[\\s;])${property}:\\s*(\\d+)px`));
		if (declared) size = Number(declared[1]);
	}
	return size;
}

describe('title bar window button hit targets', () => {
	it('finds the stylesheet', () => {
		expect(css).toContain('.button');
	});

	it.each(['gnome', 'kde', 'cinnamon'])('gives the %s buttons a 28 px hit box', (style) => {
		expect(buttonSize(style, 'width')).toBeGreaterThanOrEqual(MINIMUM_HIT_TARGET);
		expect(buttonSize(style, 'height')).toBeGreaterThanOrEqual(MINIMUM_HIT_TARGET);
	});

	it('gives the Windows 11 buttons their full-height caption size', () => {
		expect(buttonSize('win11', 'width')).toBeGreaterThanOrEqual(MINIMUM_HIT_TARGET);
	});
});
