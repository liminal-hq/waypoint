// Guards the window's base styles: text is not selectable like a web page, except where it should be
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

// Read the real file from disk: Vitest swaps CSS for empty text, so importing it would make the
// assertions pass vacuously.
const css = readFileSync(join(import.meta.dirname, 'tokens.css'), 'utf8').replace(
	/\/\*[\s\S]*?\*\//g,
	'',
);

/** The declarations of every rule whose selector list contains `selector`, joined. */
function declarationsFor(selector: string): string {
	const found: string[] = [];
	for (const [, selectors, body] of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
		if (selectors!.split(',').some((s) => s.trim() === selector)) found.push(body!);
	}
	return found.join('\n');
}

describe('base styles', () => {
	it('makes the window text unselectable, with the WebKit prefix', () => {
		const body = declarationsFor('body');
		expect(body).toMatch(/-webkit-user-select:\s*none/);
		expect(body).toMatch(/(?<!-webkit-)user-select:\s*none/);
	});

	it('draws the options of a native dropdown in the window’s own colours, not the system’s', () => {
		for (const selector of ['select option', 'select optgroup']) {
			const rule = declarationsFor(selector);
			expect(rule, selector).toMatch(/background-color:\s*var\(--wp-bg-raised\)/);
			expect(rule, selector).toMatch(/(?<!-)color:\s*var\(--wp-text-primary\)/);
		}
	});

	it.each(['input', 'textarea', "[role='alert']", '[data-selectable]'])(
		'keeps %s selectable',
		(selector) => {
			const rule = declarationsFor(selector);
			expect(rule).toMatch(/-webkit-user-select:\s*text/);
			expect(rule).toMatch(/(?<!-webkit-)user-select:\s*text/);
		},
	);
});
