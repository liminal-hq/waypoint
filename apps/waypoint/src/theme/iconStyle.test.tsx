// Verifies the Icon style setting reaches the icons: the root attribute, the stroke and fill tokens, the high contrast floor and the page's samples
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { IconStyle } from '@liminal-hq/waypoint-protocol/generated/IconStyle';
import { ICON_STYLES, IconStyleRow } from '../settings/IconChoices';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { applyAppearance, NO_OS_APPEARANCE, resolveAppearance } from './appearance';
import { ThemeRoot } from './ThemeRoot';

const tokens = readFileSync(join(import.meta.dirname, 'tokens.css'), 'utf8');
const chromeTokens = readFileSync(
	join(import.meta.dirname, '../../../../packages/chrome/src/tokens.css'),
	'utf8',
);

/** The declarations of every rule in `tokens.css` whose selector list includes a selector containing `needle`. */
function declarations(needle: string): string[] {
	const found: string[] = [];
	for (const [, selectors, body] of tokens.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
		const list = (selectors as string).replace(/\/\*[\s\S]*?\*\//g, '');
		if (list.split(',').some((selector) => selector.includes(needle))) found.push(body as string);
	}
	return found;
}

/** A custom property's value in the rules for `needle`, or `undefined` where none sets it. */
function valueFor(needle: string, property: string): string | undefined {
	for (const body of declarations(needle).reverse()) {
		const match = body.match(new RegExp(`${property}:\\s*([^;]+);`));
		if (match) return match[1]!.trim();
	}
	return undefined;
}

/** A custom property's value where `tokens.css` first sets it: the base `:root` rule. */
function base(property: string): string | undefined {
	return tokens.match(new RegExp(`${property}:\\s*([^;]+);`))?.[1]?.trim();
}

afterEach(() => {
	cleanup();
	const root = document.documentElement;
	for (const key of Object.keys(root.dataset)) delete root.dataset[key];
	root.removeAttribute('style');
});

describe('the root attribute', () => {
	const options = { touchPointer: false, systemLanguage: 'en-CA', hasFinePointer: true };
	const withStyle = (iconStyle: IconStyle) => ({
		...DEFAULT_SETTINGS,
		appearance: { ...DEFAULT_SETTINGS.appearance, iconStyle },
	});

	it('is written for each of the four styles', () => {
		for (const style of ICON_STYLES) {
			const root = document.createElement('html');
			applyAppearance(root, resolveAppearance(withStyle(style), NO_OS_APPEARANCE, options));
			expect(root.dataset.iconStyle, style).toBe(style);
		}
	});

	it('updates the moment the setting changes, next to the icon theme', async () => {
		const fake = createFakeSettingsClient();
		await act(async () => {
			render(
				<ThemeRoot client={fake} os={{ read: () => NO_OS_APPEARANCE, subscribe: () => () => {} }}>
					<p>content</p>
				</ThemeRoot>,
			);
		});
		const root = document.documentElement;
		expect(root.dataset.iconStyle).toBe('regular');
		expect(root.dataset.iconTheme).toBe('waypoint');
		for (const style of ['bold', 'filled', 'light'] as const) {
			await act(async () => {
				await fake.set(withStyle(style));
			});
			expect(root.dataset.iconStyle, style).toBe(style);
		}
	});
});

describe('the stroke token', () => {
	it('is the base weight, with Light thinner and Bold heavier, in a 16 px box', () => {
		const regular = parseFloat(base('--wp-icon-weight')!);
		const light = valueFor(":root[data-icon-style='light']", '--wp-icon-weight');
		const bold = valueFor(":root[data-icon-style='bold']", '--wp-icon-weight');
		expect(regular).toBeGreaterThan(1.2);
		expect(light).toBe('1px');
		expect(bold).toBe('2px');
		expect(parseFloat(light!)).toBeLessThan(regular);
		expect(parseFloat(bold!)).toBeGreaterThan(regular);
		// Regular is the base: it has no rule of its own.
		expect(valueFor(":root[data-icon-style='regular']", '--wp-icon-weight')).toBeUndefined();
	});

	it('is the weight raised to a floor that only high contrast sets', () => {
		expect(tokens).toMatch(
			/--wp-icon-stroke:\s*max\(var\(--wp-icon-weight\),\s*var\(--wp-icon-weight-min\)\)/,
		);
		expect(base('--wp-icon-weight-min')).toBe('0px');
		const floor = parseFloat(valueFor(":root[data-contrast='high']", '--wp-icon-weight-min')!);
		const stroke = (weight: number) => Math.max(weight, floor);
		// Regular and Light are lifted to the floor under high contrast, and Bold, already above it, is left alone.
		expect(stroke(parseFloat(base('--wp-icon-weight')!))).toBe(floor);
		expect(stroke(1)).toBe(floor);
		expect(stroke(2)).toBe(2);
		expect(floor).toBeGreaterThan(parseFloat(base('--wp-icon-weight')!));
	});

	it('is read by every outline icon family, with the chrome falling back to its own default', () => {
		for (const file of [
			'icons/AppIcons.module.css',
			'browse/FileIcon.module.css',
			'../../../packages/chrome/src/icons/icons.module.css',
		]) {
			const css = readFileSync(join(import.meta.dirname, '..', file), 'utf8');
			expect(css, file).toMatch(/stroke-width:\s*var\(--wp-icon-stroke\)/);
		}
		expect(chromeTokens).toMatch(/--wp-icon-stroke:\s*1\.25px/);
	});
});

describe('the Filled style', () => {
	it('tints closed shapes, and only Filled does', () => {
		expect(base('--wp-icon-fill')).toBe('none');
		const filled = valueFor(":root[data-icon-style='filled']", '--wp-icon-fill');
		expect(filled).toMatch(/^color-mix\(in srgb, currentcolor \d+%, transparent\)$/);
		for (const style of ['light', 'regular', 'bold']) {
			expect(
				valueFor(`:root[data-icon-style='${style}']`, '--wp-icon-fill'),
				style,
			).toBeUndefined();
		}
	});

	it('tints a little more under high contrast', () => {
		const percent = (value: string | undefined) => Number(value?.match(/(\d+)%/)?.[1]);
		const normal = valueFor(":root[data-icon-style='filled']", '--wp-icon-fill');
		const strong = valueFor(
			":root[data-contrast='high'][data-icon-style='filled']",
			'--wp-icon-fill',
		);
		expect(percent(strong)).toBeGreaterThan(percent(normal));
	});

	it('marks fillable shapes on the glyph frames and never fills the whole svg', () => {
		for (const file of ['icons/AppIcons.module.css', 'browse/FileIcon.module.css']) {
			const css = readFileSync(join(import.meta.dirname, '..', file), 'utf8');
			expect(css, file).toMatch(/\[data-fill\]\s*\{\s*fill:\s*var\(--wp-icon-fill\)/);
		}
		const frame = readFileSync(join(import.meta.dirname, '../icons/AppIcons.module.css'), 'utf8');
		expect(frame).not.toMatch(/^\.glyph\s*\{[^}]*\bfill:/m);
	});

	it('gives a folder its own, stronger accent tint', () => {
		expect(base('--wp-folder-fill')).toMatch(/var\(--wp-accent\) 25%/);
		expect(valueFor(":root[data-icon-style='filled']", '--wp-folder-fill')).toMatch(
			/var\(--wp-accent\) 45%/,
		);
	});
});

describe('Portage', () => {
	it('is left as it is: every style rule for the window skips the Portage theme', () => {
		for (const style of ['light', 'bold', 'filled']) {
			const rules = [
				...tokens.matchAll(new RegExp(`:root\\[[^{]*data-icon-style='${style}'\\][^{,]*`, 'g')),
			];
			expect(rules.length, style).toBeGreaterThan(0);
			for (const [rule] of rules)
				expect(rule, style).toContain(":not([data-icon-theme='portage'])");
		}
	});
});

describe('the Icon style row', () => {
	function mount(value: IconStyle, onChange = vi.fn(), disabled = false) {
		render(<IconStyleRow value={value} onChange={onChange} disabled={disabled} />);
		return onChange;
	}

	it('offers the four styles as radios, each with a sample strip drawn in that style', () => {
		mount('regular');
		const group = screen.getByRole('radiogroup');
		const radios = within(group).getAllByRole('radio');
		expect(radios.map((radio) => radio.getAttribute('data-value'))).toEqual([...ICON_STYLES]);
		expect(radios.map((radio) => radio.getAttribute('aria-checked'))).toEqual([
			'false',
			'true',
			'false',
			'false',
		]);
		for (const radio of radios) {
			const strip = radio.querySelector('[data-icon-style-preview]')!;
			expect(strip.getAttribute('data-icon-style-preview')).toBe(radio.getAttribute('data-value'));
			// A folder, a file and three toolbar glyphs, all decorative.
			const icons = strip.querySelectorAll('svg');
			expect(icons).toHaveLength(5);
			for (const icon of icons) expect(icon.getAttribute('aria-hidden')).toBe('true');
			expect(strip.querySelector('svg[data-group="folder"]')).not.toBeNull();
			expect(strip.querySelector('svg[data-group="document"]')).not.toBeNull();
			// The visible name is the accessible name, so the sample is not the only cue.
			expect(radio).toHaveTextContent(/\S/);
		}
	});

	it('draws the Waypoint set in the samples even where the window shows Portage', () => {
		document.documentElement.dataset.iconTheme = 'portage';
		mount('regular');
		for (const folder of document.querySelectorAll(
			'[data-icon-style-preview] svg[data-group="folder"]',
		)) {
			expect(folder.getAttribute('viewBox')).toBe('0 0 16 16');
		}
	});

	it('marks closed shapes of the folder sample fillable so Filled shows in its card', () => {
		mount('filled');
		const strip = document.querySelector('[data-icon-style-preview="filled"]')!;
		expect(strip.querySelector('svg[data-group="folder"] [data-fill]')).not.toBeNull();
	});

	it('chooses a style on click and moves with the arrow keys', () => {
		const onChange = mount('regular');
		fireEvent.click(screen.getByRole('radio', { name: /Bold/ }));
		expect(onChange).toHaveBeenLastCalledWith('bold');
		fireEvent.keyDown(screen.getByRole('radiogroup'), { key: 'ArrowRight' });
		expect(onChange).toHaveBeenLastCalledWith('bold');
		fireEvent.keyDown(screen.getByRole('radiogroup'), { key: 'End' });
		expect(onChange).toHaveBeenLastCalledWith('filled');
	});

	it('is dimmed and cannot be chosen when disabled, as under Portage', () => {
		const onChange = mount('regular', vi.fn(), true);
		const radios = screen.getAllByRole('radio') as HTMLButtonElement[];
		expect(radios.every((radio) => radio.disabled)).toBe(true);
		fireEvent.click(radios[2]!);
		expect(onChange).not.toHaveBeenCalled();
	});
});
