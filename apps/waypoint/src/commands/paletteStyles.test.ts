// Guards the palette's stylesheet: tokens only, defined tokens, bold for a match, and no motion under a reduced-motion preference
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

// Read the real files from disk: Vitest swaps CSS for empty text, so importing them would make
// every assertion pass vacuously.
const here = import.meta.dirname;
const css = readFileSync(join(here, 'CommandPalette.module.css'), 'utf8');
const appTokens = readFileSync(join(here, '../theme/tokens.css'), 'utf8');
const tokens =
	appTokens + readFileSync(join(here, '../../../../packages/chrome/src/tokens.css'), 'utf8');

describe('the palette’s stylesheet', () => {
	it('has no colour literals and no inline fallbacks', () => {
		expect(css).not.toMatch(/#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/);
		expect(css).not.toMatch(/var\(--wp-[a-z0-9-]+\s*,/);
	});

	it('reads only tokens that are defined', () => {
		const defined = new Set([...tokens.matchAll(/(--wp-[a-z0-9-]+)\s*:/g)].map((m) => m[1]));
		const used = [...css.matchAll(/var\((--wp-[a-z0-9-]+)/g)].map((m) => m[1]!);
		expect(used.length).toBeGreaterThan(10);
		for (const name of used) expect(defined.has(name), name).toBe(true);
	});

	it('defines its own tokens in terms of the theme’s, so light and dark both follow', () => {
		const own = [...appTokens.matchAll(/(--wp-palette-[a-z-]+):\s*([^;]+);/g)];
		expect(own.length).toBeGreaterThan(8);
		for (const [, name, value] of own) {
			if (/(width|top|height|motion)$/.test(name!)) continue;
			expect(value, name).toMatch(/var\(--wp-/);
		}
	});

	it('draws a match bold, so it is not shown by colour alone', () => {
		expect(css).toMatch(/\.match\s*\{[^}]*font-weight:\s*700/);
	});

	it('stops its animation under a reduced-motion preference', () => {
		expect(css).toMatch(
			/@media \(prefers-reduced-motion: reduce\)\s*\{[^}]*animation:\s*none\s*!important/,
		);
		expect(css).toMatch(/\.reducedMotion[^{]*\{[^}]*animation:\s*none/);
	});

	it('marks the active option with more than a fill: an outline', () => {
		expect(css).toMatch(/\[aria-selected='true'\][^}]*outline:/);
	});

	it('keeps a visible focus ring on the input', () => {
		expect(css).toMatch(/\.input:focus-visible[^}]*outline:/);
	});
});
