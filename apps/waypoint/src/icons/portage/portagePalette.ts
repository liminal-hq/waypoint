// The Portage folder colours: ten of them, each with a tone for dark surfaces and one for light
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

export const FOLDER_COLOURS = [
	'liminal',
	'gnome',
	'cinnamon',
	'kde',
	'windows11',
	'red',
	'pink',
	'orange',
	'purple',
	'rainbow',
] as const;

export type FolderColour = (typeof FOLDER_COLOURS)[number];

/** Which surface the icon sits on: `dark` tones are lighter and softer, `light` tones deeper. */
export type FolderTone = 'dark' | 'light';

/** One flat colour, or the stops of a diagonal gradient (the Rainbow folder). */
export type FolderFill = string | readonly string[];

export const FOLDER_PALETTE: Record<FolderColour, Record<FolderTone, FolderFill>> = {
	liminal: { dark: '#eaa55c', light: '#e39a3b' },
	gnome: { dark: '#99c1f1', light: '#62a0ea' },
	cinnamon: { dark: '#b5c98a', light: '#8fa876' },
	kde: { dark: '#5cc0f0', light: '#3daee9' },
	windows11: { dark: '#f7d26a', light: '#f0b429' },
	red: { dark: '#f0716b', light: '#e5484d' },
	pink: { dark: '#f291bb', light: '#e0609a' },
	orange: { dark: '#ff9a52', light: '#f07a1a' },
	purple: { dark: '#b9a0f0', light: '#8e6fe0' },
	rainbow: {
		dark: ['#ff7b7b', '#ffaa55', '#ffd84d', '#6fdc8c', '#5cb4f7', '#b497fc'],
		light: ['#e5484d', '#f08a24', '#f0b429', '#3fae5c', '#2f8fe0', '#8e6fe0'],
	},
};

/** A colour `fraction` of the way from `from` to `to`, both `#rrggbb`. */
export function mixColour(from: string, to: string, fraction: number): string {
	const parse = (hex: string) => {
		const n = parseInt(hex.slice(1), 16);
		return [n >> 16, (n >> 8) & 255, n & 255] as const;
	};
	const a = parse(from);
	const b = parse(to);
	return `#${a
		.map((v, i) =>
			Math.round(v + (b[i]! - v) * fraction)
				.toString(16)
				.padStart(2, '0'),
		)
		.join('')}`;
}
