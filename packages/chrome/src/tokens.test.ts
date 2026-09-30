// Guards the token layer: no colour literals outside `tokens.css`, and every custom property a component reads has a default
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

// Read the real files from disk: Vitest swaps CSS for empty text, so importing them would make
// every assertion below pass vacuously.
const root = import.meta.dirname;
const tokens = readFileSync(join(root, 'tokens.css'), 'utf8');

function moduleStylesheets(directory: string): Record<string, string> {
	const found: Record<string, string> = {};
	for (const entry of readdirSync(directory, { withFileTypes: true })) {
		const path = join(directory, entry.name);
		if (entry.isDirectory()) Object.assign(found, moduleStylesheets(path));
		else if (entry.name.endsWith('.module.css'))
			found[path.slice(root.length + 1)] = readFileSync(path, 'utf8');
	}
	return found;
}
const componentStyles = moduleStylesheets(root);

const COLOUR_LITERAL = /#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/;

describe('the chrome token layer', () => {
	it('finds the component stylesheets', () => {
		expect(Object.keys(componentStyles).length).toBeGreaterThanOrEqual(4);
	});

	it('keeps colour literals out of component CSS', () => {
		for (const [file, source] of Object.entries(componentStyles)) {
			expect(source, file).not.toMatch(COLOUR_LITERAL);
		}
	});

	it('gives every custom property a component reads a default', () => {
		const defined = new Set(
			[...tokens.matchAll(/^\s*(--wp-[a-z0-9-]+)\s*:/gm)].map((match) => match[1]),
		);
		for (const [file, source] of Object.entries(componentStyles)) {
			for (const [, name] of source.matchAll(/var\((--wp-[a-z0-9-]+)/g)) {
				// Set from script by the component that owns the rule, so it needs no default.
				if (name === '--wp-menu-x' || name === '--wp-menu-y') continue;
				expect(defined.has(name as string), `${file} reads ${name}`).toBe(true);
			}
		}
	});

	it('uses no inline fallback values in component CSS', () => {
		for (const [file, source] of Object.entries(componentStyles)) {
			expect(source, file).not.toMatch(/var\(--wp-[a-z0-9-]+\s*,/);
		}
	});
});
