// Verifies the OS palette's mapping onto the tokens, the contrast floor and the decision to draw it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import type { Palette, PaletteEntry } from '../services/osPaletteClient';
import { TEXT_CONTRAST, contrastRatio } from './accent';
import {
	FALLBACK,
	GRAPHIC_CONTRAST,
	PALETTE_TOKENS,
	decidePalette,
	isDark,
	liftToContrast,
	mapPalette,
} from './palette';

const found = (colour: string): PaletteEntry => ({
	colour,
	source: 'gtkTheme',
	reason: null,
	detail: null,
});
const missing: PaletteEntry = {
	colour: null,
	source: null,
	reason: 'sourceMissing',
	detail: 'the GTK theme defines none of popover_bg_color, card_bg_color',
};

/** Adwaita dark as GTK reports it: no surface, focus or accent names. */
function adwaitaDark(change: Partial<Palette> = {}): Palette {
	return {
		revision: 1,
		status: { available: true, source: 'gtkTheme', reason: null, detail: null },
		windowBackground: found('#242424'),
		windowForeground: found('#ffffff'),
		viewBackground: found('#1e1e1e'),
		viewForeground: found('#ffffff'),
		surfaceBackground: missing,
		selectionBackground: found('#3584e4'),
		selectionForeground: found('#ffffff'),
		border: found('#121212'),
		focus: missing,
		warning: found('#cd9309'),
		error: found('#c01c28'),
		success: found('#26a269'),
		titleBarBackground: missing,
		titleBarBackgroundEnd: missing,
		...change,
	};
}

function adwaitaLight(): Palette {
	return adwaitaDark({
		windowBackground: found('#fafafa'),
		windowForeground: found('#2e3436'),
		viewBackground: found('#ffffff'),
		viewForeground: found('#2e3436'),
	});
}

const NONE: Palette = adwaitaDark({
	status: { available: false, source: null, reason: 'sourceMissing', detail: 'x' },
	windowBackground: missing,
	windowForeground: missing,
});

describe('liftToContrast', () => {
	it('leaves a colour that already passes', () => {
		expect(liftToContrast('#ffffff', ['#242424'], TEXT_CONTRAST)).toBe('#ffffff');
	});

	it('lightens a colour on a dark background until it passes, and stops there', () => {
		const lifted = liftToContrast('#3a3a8c', ['#242424'], TEXT_CONTRAST);
		expect(contrastRatio(lifted, '#242424')).toBeGreaterThanOrEqual(TEXT_CONTRAST);
		// One step short would not have passed: it is the nearest colour that does.
		expect(lifted).not.toBe('#ffffff');
	});

	it('darkens a colour on a light background', () => {
		const lifted = liftToContrast('#cd9309', ['#ffffff'], TEXT_CONTRAST);
		expect(contrastRatio(lifted, '#ffffff')).toBeGreaterThanOrEqual(TEXT_CONTRAST);
		expect(isDark(lifted)).toBe(false);
	});

	it('holds against every background at once', () => {
		const lifted = liftToContrast('#808080', ['#ffffff', '#101010'], 7);
		// No colour is 7:1 against both white and near-black; it ends at the nearest extreme.
		expect(['#ffffff', '#000000']).toContain(lifted);
		const both = liftToContrast('#808080', ['#ffffff', '#303030'], 3);
		expect(contrastRatio(both, '#ffffff')).toBeGreaterThanOrEqual(3);
		expect(contrastRatio(both, '#303030')).toBeGreaterThanOrEqual(3);
	});

	it('moves a graphic to a lower floor than text', () => {
		const text = liftToContrast('#5588cc', ['#ffffff'], TEXT_CONTRAST);
		const graphic = liftToContrast('#5588cc', ['#ffffff'], GRAPHIC_CONTRAST);
		expect(contrastRatio(graphic, '#ffffff')).toBeGreaterThanOrEqual(GRAPHIC_CONTRAST);
		expect(contrastRatio(text, '#ffffff')).toBeGreaterThanOrEqual(TEXT_CONTRAST);
	});
});

describe('mapPalette', () => {
	it('maps a dark palette onto the surfaces and the text, with the variant it describes', () => {
		const mapped = mapPalette(adwaitaDark())!;
		expect(mapped.variant).toBe('dark');
		expect(mapped.tokens['--wp-solid-window']).toBe('#242424');
		expect(mapped.tokens['--wp-solid-content']).toBe('#1e1e1e');
		expect(mapped.tokens['--wp-text-primary']).toBe('#ffffff');
		expect(mapped.tokens['--wp-border-subtle']).toBe('#121212');
		// No surface colour: the raised surface is the view's.
		expect(mapped.tokens['--wp-bg-raised']).toBe('#1e1e1e');
	});

	it('maps a light palette to the light variant', () => {
		expect(mapPalette(adwaitaLight())!.variant).toBe('light');
	});

	const isTitleBarToken = (token: string): boolean => token.startsWith('--wp-title-bar-');

	it('sets every token it owns, and only those, leaving the title bar flat with no title bar colour', () => {
		const mapped = mapPalette(adwaitaDark())!;
		expect(Object.keys(mapped.tokens).sort()).toEqual(
			PALETTE_TOKENS.filter((token) => !isTitleBarToken(token)).sort(),
		);
	});

	describe('the title bar', () => {
		const shaded = (): Palette =>
			adwaitaDark({
				titleBarBackground: found('#303030'),
				titleBarBackgroundEnd: found('#262626'),
			});

		it('sets its two tones and their unfocused pair when the OS reports a top colour', () => {
			const mapped = mapPalette(shaded())!;
			expect(Object.keys(mapped.tokens).sort()).toEqual([...PALETTE_TOKENS].sort());
			expect(mapped.tokens['--wp-title-bar-top']).toBe('#303030');
			expect(mapped.tokens['--wp-title-bar-bottom']).toBe('#262626');
		});

		it('draws a flat bar when only the top colour is given', () => {
			const mapped = mapPalette(adwaitaDark({ titleBarBackground: found('#303030') }))!;
			expect(mapped.tokens['--wp-title-bar-bottom']).toBe('#303030');
		});

		it('ignores a bottom colour that has no top', () => {
			const mapped = mapPalette(adwaitaDark({ titleBarBackgroundEnd: found('#262626') }))!;
			expect(Object.keys(mapped.tokens).some(isTitleBarToken)).toBe(false);
		});

		it('keeps the title text at 4.5:1 on every tone, focused or not', () => {
			const { tokens } = mapPalette(shaded())!;
			for (const token of ['--wp-title-bar-top', '--wp-title-bar-bottom']) {
				expect(contrastRatio(tokens['--wp-text-primary']!, tokens[token]!)).toBeGreaterThanOrEqual(
					TEXT_CONTRAST,
				);
			}
			for (const token of ['--wp-title-bar-top-unfocused', '--wp-title-bar-bottom-unfocused']) {
				expect(contrastRatio(tokens['--wp-text-muted']!, tokens[token]!)).toBeGreaterThanOrEqual(
					TEXT_CONTRAST,
				);
			}
		});

		it('lifts a title bar colour the title text cannot be read on, and says so', () => {
			const mapped = mapPalette(adwaitaDark({ titleBarBackground: found('#d0d0d0') }))!;
			const lifted = mapped.lifted.find((entry) => entry.token === 'title-bar-top')!;
			expect(lifted.from).toBe('#d0d0d0');
			expect(contrastRatio(mapped.tokens['--wp-text-primary']!, lifted.to)).toBeGreaterThanOrEqual(
				TEXT_CONTRAST,
			);
		});
	});

	it('is null when the palette has no window colours', () => {
		expect(mapPalette(NONE)).toBeNull();
	});

	it('keeps every text colour at 4.5:1 on every surface, in both variants', () => {
		for (const palette of [adwaitaDark(), adwaitaLight()]) {
			const { tokens } = mapPalette(palette)!;
			const surfaces = ['--wp-solid-window', '--wp-solid-content', '--wp-bg-raised'];
			for (const text of ['--wp-text-primary', '--wp-text-secondary', '--wp-text-muted']) {
				for (const surface of surfaces) {
					expect(contrastRatio(tokens[text]!, tokens[surface]!)).toBeGreaterThanOrEqual(
						TEXT_CONTRAST,
					);
				}
			}
			expect(
				contrastRatio(tokens['--wp-text-primary']!, tokens['--wp-bg-selected']!),
			).toBeGreaterThanOrEqual(TEXT_CONTRAST);
			expect(
				contrastRatio(tokens['--wp-focus-ring']!, tokens['--wp-solid-window']!),
			).toBeGreaterThanOrEqual(GRAPHIC_CONTRAST);
		}
	});

	it('lifts a failing pair to a passing colour and reports it', () => {
		// A theme whose text is a dim grey on its dark window: 2.5:1 or so.
		const mapped = mapPalette(
			adwaitaDark({ windowForeground: found('#555555'), viewForeground: found('#555555') }),
		)!;
		const text = mapped.lifted.find((one) => one.token === 'text-primary')!;
		expect(text.from).toBe('#555555');
		expect(text.to).toBe(mapped.tokens['--wp-text-primary']);
		expect(text.required).toBe(TEXT_CONTRAST);
		expect(contrastRatio(text.to, '#242424')).toBeGreaterThanOrEqual(TEXT_CONTRAST);
	});

	it('lifts the selection background, not the text, when the text fails on it', () => {
		const mapped = mapPalette(adwaitaDark())!;
		// White on Adwaita blue is about 3.7:1, short of 4.5.
		const selected = mapped.lifted.find((one) => one.token === 'bg-selected')!;
		expect(selected.from).toBe('#3584e4');
		expect(mapped.tokens['--wp-bg-selected']).toBe(selected.to);
		expect(mapped.tokens['--wp-text-primary']).toBe('#ffffff');
	});

	it('lifts a status colour that is too faint on the surfaces', () => {
		const mapped = mapPalette(adwaitaLight())!;
		const warning = mapped.lifted.find((one) => one.token === 'warning')!;
		expect(warning.from).toBe('#cd9309');
		expect(contrastRatio(warning.to, '#ffffff')).toBeGreaterThanOrEqual(TEXT_CONTRAST);
	});

	it('reports nothing when everything already passes', () => {
		const palette = adwaitaDark({
			selectionBackground: found('#1a3a5c'),
			warning: found('#facc15'),
			error: found('#f87171'),
			success: found('#4ade80'),
			focus: found('#60a5fa'),
		});
		expect(mapPalette(palette)!.lifted).toEqual([]);
	});

	it('uses Waypoint’s own status and focus colours for the variant when the OS gives none', () => {
		const mapped = mapPalette(
			adwaitaDark({ warning: missing, error: missing, success: missing, focus: missing }),
		)!;
		expect(mapped.tokens['--wp-danger']).toBe(FALLBACK.dark.danger);
		expect(mapped.tokens['--wp-focus-ring']).toBe(FALLBACK.dark.focus);
	});

	it('ignores a colour that is not #rrggbb', () => {
		const mapped = mapPalette(adwaitaDark({ border: found('not-a-colour') }))!;
		expect(mapped.tokens['--wp-border-subtle']).toMatch(/^#[0-9a-f]{6}$/);
		expect(mapped.tokens['--wp-border-subtle']).not.toBe('not-a-colour');
	});
});

describe('the fallback colours', () => {
	it('are the ones tokens.css draws, per variant', () => {
		// Read from disk: Vitest swaps imported CSS for empty text.
		const css = readFileSync(join(import.meta.dirname, 'tokens.css'), 'utf8');
		const block = (start: string): string => {
			const from = css.indexOf(start);
			return css.slice(from, css.indexOf('}', from));
		};
		const value = (text: string, name: string): string =>
			new RegExp(`--wp-${name}:\\s*(#[0-9a-f]{6})`).exec(text)![1]!;
		const light = block(':root {');
		const dark = block(":root[data-theme='dark'] {");
		for (const [name, token] of [
			['danger', 'danger'],
			['success', 'success'],
			['warning', 'warning'],
			['focus', 'focus-ring'],
		] as const) {
			expect(FALLBACK.light[name]).toBe(value(light, token));
			expect(FALLBACK.dark[name]).toBe(value(dark, token));
		}
	});
});

describe('decidePalette', () => {
	const base = {
		enabled: true,
		palette: adwaitaDark(),
		mode: 'system' as const,
		highContrast: false,
	};

	it('draws the palette when it is on, usable and the mode is System', () => {
		const decision = decidePalette(base);
		expect(decision.state).toBe('applied');
		expect(decision.mapped?.variant).toBe('dark');
	});

	it('draws nothing when the option is off, but still previews the mapping', () => {
		const decision = decidePalette({ ...base, enabled: false });
		expect(decision.state).toBe('off');
		expect(decision.mapped).toBeNull();
		expect(decision.preview).not.toBeNull();
	});

	it('is ignored under high contrast', () => {
		expect(decidePalette({ ...base, highContrast: true })).toMatchObject({
			state: 'high-contrast',
			mapped: null,
		});
	});

	it('is unavailable with no palette, or one with no window colours', () => {
		expect(decidePalette({ ...base, palette: null }).state).toBe('unavailable');
		expect(decidePalette({ ...base, palette: NONE }).state).toBe('unavailable');
	});

	it('draws a forced mode only when it is the palette’s own variant', () => {
		expect(decidePalette({ ...base, mode: 'dark' }).state).toBe('applied');
		expect(decidePalette({ ...base, mode: 'light' }).state).toBe('other-variant');
		expect(decidePalette({ ...base, palette: adwaitaLight(), mode: 'light' }).state).toBe(
			'applied',
		);
	});
});
