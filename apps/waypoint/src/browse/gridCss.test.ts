// Guards the grid's paint containment: each recycled row is a layer, and icons drawn as pictures fill their frame
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const read = (file: string) => readFileSync(join(__dirname, file), 'utf8');

/** The declarations of the rule whose selector is exactly `selector` in `css`. */
function rule(css: string, selector: string): string {
	const found = [...css.matchAll(/([^{}]+)\{([^}]*)\}/g)].find(
		(match) => match[1]!.replace(/\/\*[\s\S]*?\*\//g, '').trim() === selector,
	);
	if (!found) throw new Error(`no rule for ${selector}`);
	return found[2]!.replace(/\/\*[\s\S]*?\*\//g, '');
}

describe('the grid stylesheet', () => {
	const grid = read('GridView.module.css');

	it('makes every row and group header a layer of its own, placed by transform', () => {
		for (const selector of ['.row', '.groupHeader']) {
			expect(rule(grid, selector)).toMatch(/will-change:\s*transform/);
			expect(rule(grid, selector)).toMatch(/transform:\s*translateY\(var\(--wp-row-y\)\)/);
			expect(rule(grid, selector)).toMatch(/position:\s*absolute/);
		}
	});

	it('puts no shadows, filters or transitions on the cells that a scroll redraws', () => {
		for (const selector of ['.row', '.cell', '.label']) {
			expect(rule(grid, selector)).not.toMatch(/box-shadow|filter|transition/);
		}
	});

	it('draws a picture icon as a contained background the size of its frame', () => {
		const icon = rule(read('FileIcon.module.css'), '.picture');
		expect(icon).toMatch(/background-size:\s*contain/);
		expect(icon).toMatch(/background-repeat:\s*no-repeat/);
		const frame = read('../thumbnails/Thumbnail.module.css');
		expect(frame).toMatch(/\.frame > \[data-icon-picture\]/);
		expect(frame).toMatch(/\.frame\[data-thumbnail='loaded'\] > \[data-icon-picture\]/);
	});
});
