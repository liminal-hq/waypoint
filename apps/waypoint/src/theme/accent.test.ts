// Verifies the accent rule: the brand colours, and that any accent gets text at 4.5:1 or better
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { accentFor, contrastRatio, EMBER, parseHex, resolveAccent, TEXT_CONTRAST } from './accent';

/** A small deterministic generator, so a failure names the colour that broke the rule. */
function lcg(seed: number): () => number {
	let state = seed;
	return () => {
		state = (state * 1664525 + 1013904223) % 4294967296;
		return state / 4294967296;
	};
}

describe('the brand accent', () => {
	it('keeps 4.5:1 or better in both themes (#135)', () => {
		expect(contrastRatio(EMBER.light.fill, EMBER.light.text)).toBeGreaterThanOrEqual(5.1);
		expect(contrastRatio(EMBER.dark.fill, EMBER.dark.text)).toBeGreaterThanOrEqual(6.2);
	});

	it('is what the white-on-orange report measured, before the fix', () => {
		expect(contrastRatio('#f97316', '#ffffff')).toBeCloseTo(2.8, 1);
	});
});

describe('accentFor', () => {
	it('leaves a fill that already works alone and picks its text by contrast', () => {
		expect(accentFor('#000080')).toEqual({ fill: '#000080', text: '#ffffff' });
		expect(accentFor('#ffff00')).toEqual({ fill: '#ffff00', text: '#1c1917' });
	});

	it('adjusts a mid-tone fill that neither text colour can reach 4.5:1 on', () => {
		const result = accentFor('#808080')!;
		expect(contrastRatio(result.fill, result.text)).toBeGreaterThanOrEqual(TEXT_CONTRAST);
		expect(result.fill).not.toBe('#808080');
	});

	it('gives every colour text at 4.5:1 or better', () => {
		const next = lcg(135);
		for (let i = 0; i < 2000; i += 1) {
			const hex =
				'#' +
				[next(), next(), next()]
					.map((v) =>
						Math.floor(v * 256)
							.toString(16)
							.padStart(2, '0'),
					)
					.join('');
			const result = accentFor(hex)!;
			expect(contrastRatio(result.fill, result.text), hex).toBeGreaterThanOrEqual(TEXT_CONTRAST);
		}
	});

	it('refuses what is not #rrggbb', () => {
		for (const bad of ['', 'orange', '#fff', '#12345g', '123456']) {
			expect(accentFor(bad)).toBeNull();
		}
		expect(parseHex('#0a0B0c')).toEqual([10, 11, 12]);
	});
});

describe('resolveAccent', () => {
	it('uses the brand colour per scheme, the OS accent when asked and known, or the picked one', () => {
		expect(resolveAccent({ kind: 'ember' }, 'light', null)).toEqual(EMBER.light);
		expect(resolveAccent({ kind: 'ember' }, 'dark', '#112233')).toEqual(EMBER.dark);
		expect(resolveAccent({ kind: 'os' }, 'dark', '#2563eb')).toEqual(accentFor('#2563eb'));
		// The OS reports none: the brand colour stands in.
		expect(resolveAccent({ kind: 'os' }, 'light', null)).toEqual(EMBER.light);
		expect(resolveAccent({ kind: 'custom', hex: '#0f766e' }, 'dark', null)).toEqual(
			accentFor('#0f766e'),
		);
		// A bad custom colour never leaves the page without an accent.
		expect(resolveAccent({ kind: 'custom', hex: 'nope' }, 'light', null)).toEqual(EMBER.light);
	});
});
