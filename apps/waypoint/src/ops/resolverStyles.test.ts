// Guards the resolver dialogs' stylesheets: no colour literals, no inline fallbacks, and only tokens that exist
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

// Read the real files from disk: Vitest swaps CSS for empty text, so importing them would make
// every assertion pass vacuously.
const here = import.meta.dirname;
const sheets = ['ConflictDialog.module.css', 'OperationErrorDialog.module.css'].map((name) => ({
	name,
	css: readFileSync(join(here, name), 'utf8'),
}));
const tokens =
	readFileSync(join(here, '../theme/tokens.css'), 'utf8') +
	readFileSync(join(here, '../../../../packages/chrome/src/tokens.css'), 'utf8');

describe('the resolver dialogs’ stylesheets', () => {
	it.each(sheets)('$name has no colour literals and no inline fallbacks', ({ css }) => {
		expect(css).not.toMatch(/#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/);
		expect(css).not.toMatch(/var\(--wp-[a-z0-9-]+\s*,/);
	});

	it.each(sheets)('$name reads only tokens that are defined', ({ css }) => {
		const defined = new Set([...tokens.matchAll(/(--wp-[a-z0-9-]+)\s*:/g)].map((m) => m[1]));
		for (const [, name] of css.matchAll(/var\((--wp-[a-z0-9-]+)/g)) {
			expect(defined.has(name!), name).toBe(true);
		}
	});

	it('marks a danger choice with more than colour: the stylesheet pairs it with weight', () => {
		const css = sheets[0]!.css;
		expect(css).toMatch(/\[data-danger\][^}]*font-weight:\s*600/);
	});
});
