// Picks the fill and the text colour of the accent so the text always keeps 4.5:1 (D114)
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { AccentChoice } from '@liminal-hq/waypoint-protocol/generated/AccentChoice';

/** The least contrast text may have against its fill (WCAG AA, normal text). */
export const TEXT_CONTRAST = 4.5;

/** The brand accent per scheme, with the text that sits on it (D114). */
export const EMBER = {
	light: { fill: '#c2410c', text: '#ffffff' },
	dark: { fill: '#f97316', text: '#1c1917' },
} as const;

const NEAR_BLACK = '#1c1917';
const WHITE = '#ffffff';

export interface AccentColours {
	fill: string;
	text: string;
}

/** Parses `#rrggbb`; `null` for anything else. */
export function parseHex(hex: string): [number, number, number] | null {
	const match = /^#([0-9a-fA-F]{2})([0-9a-fA-F]{2})([0-9a-fA-F]{2})$/.exec(hex);
	if (!match) return null;
	return [parseInt(match[1]!, 16), parseInt(match[2]!, 16), parseInt(match[3]!, 16)];
}

export function toHex([r, g, b]: [number, number, number]): string {
	return '#' + [r, g, b].map((v) => Math.round(v).toString(16).padStart(2, '0')).join('');
}

function channel(value: number): number {
	const s = value / 255;
	return s <= 0.03928 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
}

/** The WCAG relative luminance of an sRGB colour. */
export function luminance([r, g, b]: [number, number, number]): number {
	return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

/** The WCAG contrast ratio of two `#rrggbb` colours, 1 to 21. */
export function contrastRatio(a: string, b: string): number {
	const pa = parseHex(a);
	const pb = parseHex(b);
	if (!pa || !pb) return 1;
	const [hi, lo] = [luminance(pa), luminance(pb)].sort((x, y) => y - x) as [number, number];
	return (hi + 0.05) / (lo + 0.05);
}

/** Moves a colour toward black or white by `amount` (0 to 1). */
function mix(
	rgb: [number, number, number],
	toward: 0 | 255,
	amount: number,
): [number, number, number] {
	return rgb.map((v) => v + (toward - v) * amount) as [number, number, number];
}

/**
 * The fill and the text for an accent colour: black or white text, whichever contrasts more, and
 * when neither reaches 4.5:1 the fill is darkened (for white text) or lightened (for dark text)
 * in small steps until one does. A fill that already works is returned unchanged.
 */
export function accentFor(hex: string): AccentColours | null {
	const rgb = parseHex(hex);
	if (!rgb) return null;
	const normal = toHex(rgb);
	const white = contrastRatio(normal, WHITE);
	const dark = contrastRatio(normal, NEAR_BLACK);
	if (Math.max(white, dark) >= TEXT_CONTRAST) {
		return { fill: normal, text: white >= dark ? WHITE : NEAR_BLACK };
	}
	// Neither works: move the fill toward the side that helps the better text.
	const text = white >= dark ? WHITE : NEAR_BLACK;
	const toward = text === WHITE ? 0 : 255;
	for (let step = 1; step <= 100; step += 1) {
		const fill = toHex(mix(rgb, toward, step / 100));
		if (contrastRatio(fill, text) >= TEXT_CONTRAST) return { fill, text };
	}
	return { fill: toward === 0 ? '#000000' : WHITE, text };
}

/**
 * The accent to draw with: the brand colour for the scheme, the OS's accent when the person chose
 * it and the OS reports one, or the colour they picked. `null` hex values fall back to the brand.
 */
export function resolveAccent(
	choice: AccentChoice,
	scheme: 'light' | 'dark',
	osAccent: string | null,
): AccentColours {
	const picked = choice.kind === 'custom' ? choice.hex : choice.kind === 'os' ? osAccent : null;
	return (picked ? accentFor(picked) : null) ?? { ...EMBER[scheme] };
}
