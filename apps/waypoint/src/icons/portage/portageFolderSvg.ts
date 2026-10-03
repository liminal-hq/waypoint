// A pure function that draws a Portage folder in any colour, tone, state, badge and standard-folder glyph
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	FOLDER_BACK,
	FOLDER_FRONT,
	FOLDER_FRONT_BAND,
	FOLDER_OPEN_BAND,
	FOLDER_OPEN_FRONT,
	PORTAGE_FOLDER_BADGES,
	PORTAGE_FOLDER_GLYPHS,
	type PortageFolderBadge,
	type PortageFolderGlyph,
} from './portageFolderArt';
import { FOLDER_PALETTE, mixColour, type FolderColour, type FolderTone } from './portagePalette';

/** `closed` is the plain folder, `open` shows a sheet leaning out, `empty` has no paper and a paler front. */
export type PortageFolderVariant = 'closed' | 'open' | 'empty';

export interface PortageFolderOptions {
	colour?: FolderColour;
	tone?: FolderTone;
	variant?: PortageFolderVariant;
	badge?: PortageFolderBadge;
	/** The mark on the front panel: one of the user's standard folders. */
	glyph?: PortageFolderGlyph;
}

const PAPER = '#f6f2ea';
const HIGHLIGHT = ' fill="#fff" fill-opacity=".42"';
const RAINBOW_INK = '#2a1f3a';

/** A gradient whose first half is the colour and whose second half is a lighter one: the lower band. */
function bandedGradient(id: string, colour: string, y1: number, y2: number): string {
	const light = mixColour(colour, '#ffffff', 0.22);
	return (
		`<linearGradient id="${id}" x1="0" y1="${y1}" x2="0" y2="${y2}" gradientUnits="userSpaceOnUse">` +
		`<stop offset="0" stop-color="${colour}"/><stop offset=".5" stop-color="${colour}"/>` +
		`<stop offset=".5" stop-color="${light}"/><stop offset="1" stop-color="${light}"/></linearGradient>`
	);
}

/**
 * The folder as a standalone 64 by 64 SVG. Ids (the shadow filter and any gradients) are derived from
 * the options, so any number of folders can be inlined in one page and two draws of the same folder
 * share definitions that are identical.
 */
export function portageFolderSvg(options: PortageFolderOptions = {}): string {
	const { colour = 'liminal', tone = 'dark', variant = 'closed', badge, glyph } = options;
	const fill = FOLDER_PALETTE[colour][tone];
	const stops = typeof fill === 'string' ? null : fill;
	const open = variant === 'open';
	const empty = variant === 'empty';
	const id = ['pf', colour, tone, variant, badge, glyph].filter(Boolean).join('-');

	const defs = [
		`<filter id="${id}-sh" x="-50%" y="-200%" width="200%" height="500%"><feGaussianBlur stdDeviation="1.2"/></filter>`,
	];
	let back: string;
	let front: string;
	let ink: string;
	if (stops) {
		defs.push(
			`<linearGradient id="${id}-g" x1="8" y1="14" x2="56" y2="52" gradientUnits="userSpaceOnUse">` +
				stops
					.map(
						(stop, i) =>
							`<stop offset="${+(i / (stops.length - 1)).toFixed(3)}" stop-color="${stop}"/>`,
					)
					.join('') +
				`</linearGradient>`,
		);
		back = `url(#${id}-g)`;
		front = back;
		ink = RAINBOW_INK;
	} else {
		const panel = empty ? mixColour(fill as string, '#ffffff', 0.4) : (fill as string);
		back = mixColour(panel, '#000000', 0.2);
		front = panel;
		ink = mixColour(fill as string, '#000000', 0.6);
	}

	const parts: string[] = [
		`<ellipse cx="32" cy="57.5" rx="18" ry="2.4" fill="#000" fill-opacity=".22" filter="url(#${id}-sh)"/>`,
		`<path d="${FOLDER_BACK}" fill="${back}"/>`,
	];
	if (stops) parts.push(`<path d="${FOLDER_BACK}" fill="#000" fill-opacity=".2"/>`);
	if (open) {
		parts.push(
			`<rect x="16" y="15" width="30" height="14" rx="2" fill="#fff" transform="rotate(-7 31 22)"/>`,
		);
	}
	if (!empty) {
		parts.push(
			`<rect x="13" y="20" width="38" height="12" rx="2" fill="${PAPER}"/>`,
			`<rect x="13" y="20" width="38" height="3" rx="1.5" fill="#000" fill-opacity=".08"/>`,
		);
	}
	parts.push(
		`<path d="${open ? FOLDER_OPEN_FRONT : FOLDER_FRONT}" fill="${front}"/>`,
		`<path d="${open ? FOLDER_OPEN_BAND : FOLDER_FRONT_BAND}" fill="#fff" fill-opacity=".22"/>`,
		open
			? `<rect x="10" y="30.5" width="20" height="2.4" rx="1.2"${HIGHLIGHT}/>`
			: `<rect x="12" y="27.5" width="20" height="2.4" rx="1.2"${HIGHLIGHT}/>` +
					`<rect x="11" y="30.5" width="2.4" height="6" rx="1.2"${HIGHLIGHT}/>`,
	);
	if (glyph) {
		const opacity = stops ? ' stroke-opacity=".75"' : '';
		parts.push(
			`<path d="${PORTAGE_FOLDER_GLYPHS[glyph]}" stroke="${ink}"${opacity} stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" transform="translate(32 39.5) scale(1.9) translate(-12 -14.75)"/>`,
		);
	}
	if (badge) {
		const { colour: badgeColour, glyph: badgeGlyph } = PORTAGE_FOLDER_BADGES[badge];
		defs.push(bandedGradient(`${id}-b`, badgeColour, 36.5, 57.5));
		parts.push(
			`<circle cx="47" cy="47" r="12.5" fill="${PAPER}"/>`,
			`<circle cx="47" cy="47" r="10.5" fill="url(#${id}-b)"/>`,
			`<rect x="41" y="39.5" width="8" height="2.2" rx="1.1"${HIGHLIGHT}/>`,
			`<path d="${badgeGlyph}" stroke="#fff" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" transform="translate(47 47) scale(2) translate(-17.5 -17.5)"/>`,
		);
	}
	return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" fill="none"><defs>${defs.join('')}</defs>${parts.join('')}</svg>`;
}
