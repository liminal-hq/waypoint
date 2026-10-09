// Guards the Administrator Mode styles: tokens only, the frame accent over the window and never in the way of the pointer
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

// Read the real files from disk: Vitest swaps CSS for empty text, so importing them would make
// every assertion pass vacuously.
const here = import.meta.dirname;
const read = (path: string) => readFileSync(join(here, path), 'utf8');
const sheets = {
	badge: read('ElevatedBadge.module.css'),
	prompt: read('ElevationPrompt.module.css'),
	frame: read('../app/MainScreen.module.css'),
	tabs: read('../tabs/TabStrip.module.css'),
};
const tokens = read('../theme/tokens.css') + read('../../../../packages/chrome/src/tokens.css');
const defined = new Set([...tokens.matchAll(/(--wp-[a-z0-9-]+)\s*:/g)].map((m) => m[1]));

describe('the Administrator Mode stylesheets', () => {
	it.each(Object.entries(sheets))('%s has no colour literals', (_name, css) => {
		expect(css).not.toMatch(/#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/);
	});

	it.each(['badge', 'prompt'] as const)('%s reads only tokens that are defined', (name) => {
		const used = [...sheets[name].matchAll(/var\((--wp-[a-z0-9-]+)/g)].map((m) => m[1]!);
		expect(used.length).toBeGreaterThan(3);
		for (const token of used) expect(defined.has(token), token).toBe(true);
	});

	it('draws the window’s frame accent from the token, over the regions, and lets the pointer through', () => {
		const start = sheets.frame.indexOf('.elevated::after');
		expect(start).toBeGreaterThan(-1);
		const rule = sheets.frame.slice(start, sheets.frame.indexOf('}', start));
		expect(rule).toMatch(/border:\s*3px solid var\(--wp-elevated\)/);
		expect(rule).toMatch(/pointer-events:\s*none/);
		expect(rule).toMatch(/z-index:/);
	});

	it('marks an elevated tab with the token on the start edge', () => {
		expect(sheets.tabs).toMatch(
			/\.slot\[data-elevated\] \.tab\s*\{[^}]*border-inline-start:\s*3px solid var\(--wp-elevated\)/,
		);
	});
});
