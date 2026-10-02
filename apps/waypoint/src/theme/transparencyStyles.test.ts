// Guards the stylesheet half of transparency: regions mix their alpha only while it is on, and no root paints under them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { ALPHA_PROPERTIES } from './transparencyDom';

// Read the real files from disk: Vitest swaps CSS for empty text.
const read = (path: string) =>
	readFileSync(join(import.meta.dirname, path), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
const tokens = read('tokens.css');

/** The body of the rule whose selector is exactly `selector`. */
function rule(selector: string): string {
	for (const [, selectors, body] of tokens.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
		if (selectors!.trim() === selector) return body!;
	}
	return '';
}

describe('transparency styles', () => {
	const on = rule(":root[data-transparency='on']");

	it('mixes every region with its alpha only under data-transparency=on', () => {
		for (const region of ['window', 'sidebar', 'content']) {
			expect(on).toMatch(new RegExp(`--wp-bg-${region}:\\s*color-mix\\(`));
		}
		// Off, the regions are the solid colours.
		const root = tokens.slice(0, tokens.indexOf('@media'));
		expect(root).toMatch(/--wp-bg-window:\s*var\(--wp-solid-window\)/);
		expect(root).not.toMatch(/color-mix\([^)]*--wp-alpha/);
	});

	it('reads every alpha ThemeRoot writes', () => {
		for (const name of Object.values(ALPHA_PROPERTIES)) expect(on, name).toContain(`var(${name})`);
	});

	it('leaves the window roots unpainted while the regions are translucent, so alphas do not stack', () => {
		expect(on).toMatch(/--wp-bg-screen:\s*transparent/);
		expect(on).toMatch(/--wp-bg-body:\s*transparent/);
		for (const screen of ['MainScreen', 'SettingsScreen', 'PlaceholderScreen', 'OpsScreen']) {
			const css = read(`../app/${screen}.module.css`);
			const block = css.slice(css.indexOf('.screen {'), css.indexOf('}', css.indexOf('.screen {')));
			expect(block, screen).toContain('var(--wp-bg-screen)');
		}
	});
});
