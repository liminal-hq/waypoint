// The Quick Look picture box must size its one cell, or a portrait image spills out of it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const css = readFileSync(join(__dirname, 'QuickLook.module.css'), 'utf8');

/** The declarations of the rule whose selector is exactly `selector`. */
function rule(selector: string): string {
	const found = [...css.matchAll(/([^{}]+)\{([^}]*)\}/g)].find(
		(match) =>
			match[1]!
				.replace(/\/\*[\s\S]*?\*\//g, '')
				.replace(/\s+/g, ' ')
				.trim() === selector,
	);
	if (!found) throw new Error(`no rule for ${selector}`);
	return found[2]!;
}

describe('the Quick Look picture box', () => {
	it('gives its one cell the box size, so the image is contained and not stretched to the width', () => {
		expect(rule('.picture')).toMatch(
			/grid-template:\s*minmax\(0,\s*1fr\)\s*\/\s*minmax\(0,\s*1fr\)/,
		);
		expect(rule('.image, .underlay')).toMatch(/max-block-size:\s*100%/);
		expect(rule('.image, .underlay')).toMatch(/object-fit:\s*contain/);
	});
});
