// Maps the OS colour palette onto Waypoint's tokens, lifting any pair that falls under the contrast floor (D144)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Palette, PaletteEntry } from '../services/osPaletteClient';
import { TEXT_CONTRAST, contrastRatio, luminance, parseHex, toHex } from './accent';

/** The least contrast a graphic (the focus ring) may have against what it sits on (WCAG 1.4.11). */
export const GRAPHIC_CONTRAST = 3;

type Rgb = [number, number, number];

/** The colours Waypoint draws where the OS gives none, as `tokens.css` has them for each variant. */
export const FALLBACK = {
	light: { danger: '#b91c1c', success: '#15803d', warning: '#a16207', focus: '#2563eb' },
	dark: { danger: '#f87171', success: '#4ade80', warning: '#facc15', focus: '#60a5fa' },
} as const;

/** One colour the page moved to keep a pair readable. */
export interface LiftedColour {
	/** The token that was moved (without the `--wp-` prefix). */
	token: string;
	/** What the OS (or the fallback) gave. */
	from: string;
	/** What is drawn instead. */
	to: string;
	/** The least contrast asked for. */
	required: number;
}

/** What the palette makes of Waypoint's tokens, for one variant. */
export interface MappedPalette {
	variant: 'light' | 'dark';
	/** Custom properties to set on the root, by full name (`--wp-text-primary`). */
	tokens: Record<string, string>;
	lifted: LiftedColour[];
}

/** The custom properties the mapping can set, so a change of mind removes exactly those. */
export const PALETTE_TOKENS: readonly string[] = [
	'--wp-solid-window',
	'--wp-solid-sidebar',
	'--wp-solid-content',
	'--wp-bg-raised',
	'--wp-bg-hover',
	'--wp-bg-selected',
	'--wp-border-subtle',
	'--wp-text-primary',
	'--wp-text-secondary',
	'--wp-text-muted',
	'--wp-danger',
	'--wp-success',
	'--wp-warning',
	'--wp-focus-ring',
];

/** The colour of an entry, if it has a usable one. */
function colourOf(entry: PaletteEntry): string | null {
	return entry.colour && parseHex(entry.colour) ? entry.colour.toLowerCase() : null;
}

/** Moves `from` toward `toward` (0 black, 255 white) by `amount` (0 to 1). */
function blend(from: Rgb, toward: Rgb, amount: number): Rgb {
	return from.map((v, i) => v + (toward[i]! - v) * amount) as Rgb;
}

function mixHex(a: string, b: string, amountOfB: number): string {
	return toHex(blend(parseHex(a)!, parseHex(b)!, amountOfB));
}

/** Whether `hex` is a dark colour, by the WCAG midpoint. */
export function isDark(hex: string): boolean {
	const rgb = parseHex(hex);
	return rgb !== null && luminance(rgb) < 0.179;
}

const passes = (colour: string, others: readonly string[], min: number): boolean =>
	others.every((other) => contrastRatio(colour, other) >= min);

/**
 * The colour nearest `colour` that keeps at least `min` contrast against every one of `others`:
 * it is moved in small steps toward white or black, whichever it can reach the floor with, and
 * the way the colours already point wins a tie. A colour that already passes is returned as it is.
 */
export function liftToContrast(colour: string, others: readonly string[], min: number): string {
	const rgb = parseHex(colour);
	if (!rgb || passes(colour, others, min)) return colour;
	const mean =
		others.reduce((sum, other) => sum + luminance(parseHex(other) ?? [0, 0, 0]), 0) /
		Math.max(others.length, 1);
	// Away from the backgrounds first: lighter on dark ones, darker on light ones.
	const order: Rgb[] =
		mean < 0.179
			? [
					[255, 255, 255],
					[0, 0, 0],
				]
			: [
					[0, 0, 0],
					[255, 255, 255],
				];
	for (const target of order) {
		for (let step = 1; step <= 100; step += 1) {
			const candidate = toHex(blend(rgb, target, step / 100));
			if (passes(candidate, others, min)) return candidate;
		}
	}
	// No colour reaches the floor against these (a mid-grey pair): the one that gets nearest.
	return order[0]![0] === 255 ? '#ffffff' : '#000000';
}

/**
 * Maps the palette onto Waypoint's tokens for the variant its window background is, or `null`
 * when the palette cannot stand in for the window's colours (no window background or text).
 *
 * Surfaces and text come from the OS; the sidebar, hover, secondary and muted text and a missing
 * border are derived from them, and the status and focus colours are the OS's where it has them
 * and Waypoint's own where it has not. Then every pair is held to the floor: text 4.5:1 on every
 * surface it sits on, the selected row's background 4.5:1 against its text, status colours (drawn
 * as text and icons) 4.5:1 and the focus ring 3:1 against the surfaces. A colour that fails moves
 * to the nearest one that passes and is listed in `lifted`.
 */
export function mapPalette(palette: Palette): MappedPalette | null {
	const window = colourOf(palette.windowBackground);
	const windowText = colourOf(palette.windowForeground);
	if (!window || !windowText) return null;
	const variant = isDark(window) ? 'dark' : 'light';
	const content = colourOf(palette.viewBackground) ?? window;
	const viewText = colourOf(palette.viewForeground) ?? windowText;
	const raised = colourOf(palette.surfaceBackground) ?? content;
	const sidebar = mixHex(window, windowText, 0.04);
	const hover = mixHex(window, windowText, 0.09);
	const surfaces = [window, content, raised, sidebar, hover];
	const lifted: LiftedColour[] = [];
	const hold = (token: string, from: string, against: readonly string[], min: number): string => {
		const to = liftToContrast(from, against, min);
		if (to !== from) lifted.push({ token, from, to, required: min });
		return to;
	};

	// Text on every surface; the view's text colour is the primary one where the two differ, as
	// the file area is where most text is drawn.
	const primary = hold('text-primary', viewText, surfaces, TEXT_CONTRAST);
	const secondary = hold('text-secondary', mixHex(primary, window, 0.25), surfaces, TEXT_CONTRAST);
	const muted = hold('text-muted', mixHex(primary, window, 0.4), surfaces, TEXT_CONTRAST);

	// The selected row keeps the primary and secondary text, so its background is what moves.
	const selected = hold(
		'bg-selected',
		colourOf(palette.selectionBackground) ?? mixHex(content, primary, 0.15),
		[primary, secondary],
		TEXT_CONTRAST,
	);

	const fallback = FALLBACK[variant];
	const status = (token: string, entry: PaletteEntry, own: string): string =>
		hold(token, colourOf(entry) ?? own, [window, content, raised], TEXT_CONTRAST);
	const danger = status('danger', palette.error, fallback.danger);
	const success = status('success', palette.success, fallback.success);
	const warning = status('warning', palette.warning, fallback.warning);
	const focus = hold(
		'focus-ring',
		colourOf(palette.focus) ?? fallback.focus,
		[window, content, raised],
		GRAPHIC_CONTRAST,
	);

	return {
		variant,
		lifted,
		tokens: {
			'--wp-solid-window': window,
			'--wp-solid-sidebar': sidebar,
			'--wp-solid-content': content,
			'--wp-bg-raised': raised,
			'--wp-bg-hover': hover,
			'--wp-bg-selected': selected,
			'--wp-border-subtle': colourOf(palette.border) ?? mixHex(window, primary, 0.18),
			'--wp-text-primary': primary,
			'--wp-text-secondary': secondary,
			'--wp-text-muted': muted,
			'--wp-danger': danger,
			'--wp-success': success,
			'--wp-warning': warning,
			'--wp-focus-ring': focus,
		},
	};
}

/** Why the palette is, or is not, in force. */
export type PaletteState =
	/** The option is off. */
	| 'off'
	/** The palette is drawn. */
	| 'applied'
	/** The option is on but the system has no usable palette (yet). */
	| 'unavailable'
	/** Forced or high-contrast colours already take the system's own. */
	| 'high-contrast'
	/** The mode is forced to the other variant than the palette's, so Waypoint's own colours are drawn. */
	| 'other-variant';

export interface PaletteDecision {
	state: PaletteState;
	/** The mapping when `state` is `applied`; otherwise `null`. */
	mapped: MappedPalette | null;
	/** The mapping whatever the state, for the page's note about lifted colours (`null` with no usable palette). */
	preview: MappedPalette | null;
}

/**
 * Whether the palette is drawn: the option is on, the system has a usable palette, high contrast
 * is not in force (it takes the system's colours already), and the colour mode is "System" or the
 * variant the palette describes.
 */
export function decidePalette(options: {
	enabled: boolean;
	palette: Palette | null;
	mode: 'system' | 'light' | 'dark';
	highContrast: boolean;
}): PaletteDecision {
	const preview = options.palette?.status.available ? mapPalette(options.palette) : null;
	const decision = (state: PaletteState, mapped: MappedPalette | null = null): PaletteDecision => ({
		state,
		mapped,
		preview,
	});
	if (!options.enabled) return decision('off');
	if (options.highContrast) return decision('high-contrast');
	if (!preview) return decision('unavailable');
	if (options.mode !== 'system' && options.mode !== preview.variant) {
		return decision('other-variant');
	}
	return decision('applied', preview);
}
