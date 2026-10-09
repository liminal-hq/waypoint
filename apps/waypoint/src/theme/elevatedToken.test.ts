// Verifies the Administrator Mode colour is defined for light, dark and high contrast, and is readable on the window colours
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { contrastRatio, TEXT_CONTRAST } from './accent';

// Read the real file from disk: Vitest swaps CSS for empty text.
const css = readFileSync(join(import.meta.dirname, 'tokens.css'), 'utf8').replace(
	/\/\*[\s\S]*?\*\//g,
	'',
);

/** The value of `token` in the rule that opens with `selector` (not inside a media block's other rules). */
function valueIn(selector: string, token: string): string {
	const start = css.indexOf(`${selector} {`);
	if (start < 0) throw new Error(`no rule for ${selector}`);
	const body = css.slice(start, css.indexOf('}', start));
	const found = new RegExp(`${token}:\\s*(#[0-9a-fA-F]{6})`).exec(body);
	if (!found) throw new Error(`${token} is not set in ${selector}`);
	return found[1]!;
}

const GRAPHIC_CONTRAST = 3;

describe('--wp-elevated', () => {
	it('is set for light, both dark rules and both high-contrast schemes', () => {
		for (const selector of [
			':root',
			":root:not([data-theme='light'])",
			":root[data-theme='dark']",
			":root[data-contrast='high']",
			":root[data-contrast='high'][data-theme='dark']",
		]) {
			expect(valueIn(selector, '--wp-elevated'), selector).toMatch(/^#/);
		}
	});

	it('is readable as text and as a frame on every window colour', () => {
		const cases = [
			[':root', '--wp-solid-window'],
			[':root', '--wp-solid-content'],
			[':root', '--wp-bg-raised'],
			[":root[data-theme='dark']", '--wp-solid-window'],
			[":root[data-theme='dark']", '--wp-solid-content'],
			[":root[data-theme='dark']", '--wp-bg-raised'],
		] as const;
		for (const [selector, surface] of cases) {
			const elevated = valueIn(selector, '--wp-elevated');
			const ground = valueIn(selector, surface);
			expect(contrastRatio(elevated, ground), `${selector} ${surface}`).toBeGreaterThanOrEqual(
				TEXT_CONTRAST,
			);
			expect(contrastRatio(elevated, ground)).toBeGreaterThanOrEqual(GRAPHIC_CONTRAST);
		}
	});

	it('keeps the high-contrast colours at 7:1 against their windows', () => {
		expect(
			contrastRatio(
				valueIn(":root[data-contrast='high']", '--wp-elevated'),
				valueIn(":root[data-contrast='high']", '--wp-bg-window'),
			),
		).toBeGreaterThanOrEqual(7);
		expect(
			contrastRatio(
				valueIn(":root[data-contrast='high'][data-theme='dark']", '--wp-elevated'),
				valueIn(":root[data-contrast='high'][data-theme='dark']", '--wp-bg-window'),
			),
		).toBeGreaterThanOrEqual(7);
	});
});
