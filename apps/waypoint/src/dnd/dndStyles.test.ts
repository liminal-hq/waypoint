// Guards the file drag's stylesheets: tokens only, light and dark values, and what Reduce motion turns off
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';

// Read from disk: Vitest swaps CSS for empty text, so importing would make every assertion vacuous.
const here = import.meta.dirname;
const sheets = ['dropTargets.css', 'DragStack.module.css'].map((name) => ({
	name,
	css: readFileSync(join(here, name), 'utf8'),
}));
const tokens = readFileSync(join(here, '../theme/tokens.css'), 'utf8');
const chromeTokens = readFileSync(join(here, '../../../../packages/chrome/src/tokens.css'), 'utf8');

/** Set by the drag as it runs, not by the theme. */
const RUNTIME = new Set(['--wp-drag-x', '--wp-drag-y', '--wp-stack-depth', '--wp-drop-spring-ms']);

describe('the file drag stylesheets', () => {
	it.each(sheets)('$name has no colour literals', ({ css }) => {
		expect(css).not.toMatch(/#[0-9a-fA-F]{3,8}\b|\brgba?\(|\bhsla?\(/);
	});

	it.each(sheets)('$name reads only tokens that are defined', ({ css }) => {
		const defined = new Set(
			[...(tokens + chromeTokens).matchAll(/(--wp-[a-z0-9-]+)\s*:/g)].map((m) => m[1]),
		);
		for (const [, name] of css.matchAll(/var\((--wp-[a-z0-9-]+)/g)) {
			expect(defined.has(name!) || RUNTIME.has(name!), name).toBe(true);
		}
	});
});

describe('the drag tokens', () => {
	const names = [...tokens.matchAll(/(--wp-(?:drop|file-drag)-[a-z-]+)\s*:/g)].map((m) => m[1]!);

	it('are defined once for the light theme', () => {
		const root = tokens.slice(
			tokens.indexOf(':root {'),
			tokens.indexOf('@media (prefers-color-scheme: dark)'),
		);
		for (const name of new Set(names)) expect(root, name).toContain(`${name}:`);
	});

	it('give the washes their own dark values, in the system dark theme and the chosen one', () => {
		const dark = tokens.slice(
			tokens.indexOf('@media (prefers-color-scheme: dark)'),
			tokens.indexOf(":root[data-theme='light']"),
		);
		const chosen = tokens.slice(
			tokens.indexOf(":root[data-theme='dark']"),
			tokens.indexOf('Window frame per platform'),
		);
		for (const name of ['--wp-drop-ok-fill', '--wp-drop-blocked-fill']) {
			expect(dark, name).toContain(`${name}:`);
			expect(chosen, name).toContain(`${name}:`);
		}
	});
});

describe('Reduce motion', () => {
	it('stops the spring ring animating and shows a steady dotted edge instead', () => {
		const css = sheets[0]!.css;
		const reduced = css.slice(css.indexOf('@media (prefers-reduced-motion: reduce)'));
		expect(reduced).toMatch(/animation:\s*none/);
		expect(reduced).toMatch(/outline-style:\s*dotted/);
	});

	it('stops the shake of a refusing target and keeps the words', () => {
		expect(tokens).toMatch(/prefers-reduced-motion: reduce\)\s*\{[^@]*--wp-file-drag-shake:\s*0px/);
		// The shake is the only motion the pill has, and it is driven by that token.
		expect(sheets[1]!.css).toContain('var(--wp-file-drag-shake)');
	});

	it('does not tell a refusal by colour alone: the edge is dashed', () => {
		expect(sheets[0]!.css).toMatch(/\[data-drop-over='blocked'\]\s*\{[^}]*dashed/);
		expect(sheets[1]!.css).toMatch(/\.pill\[data-kind='blocked'\]\s*\{[^}]*dashed/);
	});
});
