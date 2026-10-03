// The tab strip's drop cue is a layer over the tabs, so a tab never hides it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

const css = readFileSync(join(__dirname, 'TabStrip.module.css'), 'utf8');

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

describe('the tab strip drop cue', () => {
	it('is drawn on a layer above the tabs that the pointer passes through', () => {
		const layer = rule('.scroller[data-drop-over]::after');
		expect(layer).toMatch(/position:\s*absolute/);
		expect(layer).toMatch(/z-index:\s*[1-9]/);
		expect(layer).toMatch(/pointer-events:\s*none/);
		expect(layer).toMatch(/outline:\s*2px solid/);
		// The scroller itself draws nothing (the tabs would cover it).
		const scroller = rule('.scroller[data-drop-over]');
		expect(scroller).toMatch(/outline:\s*none/);
		expect(scroller).toMatch(/background-image:\s*none/);
	});

	it('is dashed when the drop would be refused', () => {
		expect(rule(".scroller[data-drop-over='blocked']::after")).toMatch(/dashed/);
	});
});
