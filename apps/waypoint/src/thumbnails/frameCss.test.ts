// A thumbnail frame must size its one cell, or a tall picture spills over the name below it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const css = readFileSync(join(__dirname, 'Thumbnail.module.css'), 'utf8');

/** The declarations of the rule whose selector is exactly `selector`. */
function rule(selector: string): string {
	const found = [...css.matchAll(/([^{}]+)\{([^}]*)\}/g)].find(
		(match) => match[1]!.replace(/\/\*[\s\S]*?\*\//g, '').trim() === selector,
	);
	if (!found) throw new Error(`no rule for ${selector}`);
	return found[2]!;
}

describe('the thumbnail frame', () => {
	it('gives its one cell the frame size, so a portrait picture is contained', () => {
		expect(rule('.frame')).toMatch(/grid-template:\s*minmax\(0,\s*1fr\)\s*\/\s*minmax\(0,\s*1fr\)/);
		expect(rule('.frame')).toMatch(/overflow:\s*hidden/);
		expect(rule('.picture')).toMatch(/max-height:\s*100%/);
		expect(rule('.picture')).toMatch(/object-fit:\s*contain/);
	});
});
