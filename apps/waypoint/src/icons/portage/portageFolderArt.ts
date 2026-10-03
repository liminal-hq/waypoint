// The art data behind every Portage folder: outlines, standard-folder glyphs and badge glyphs
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';

// Every shape is drawn in a 64 by 64 box. A folder is a back panel, a sheet of paper, a front panel
// with a lighter lower band, and a soft highlight; `portageFolderSvg` fills the panels from a palette.
export const FOLDER_BACK =
	'M8 17a3 3 0 013-3h12l4.5 4.5H53a3 3 0 013 3V49a3 3 0 01-3 3H11a3 3 0 01-3-3z';
export const FOLDER_FRONT = 'M8 28a3 3 0 013-3h42a3 3 0 013 3v21a3 3 0 01-3 3H11a3 3 0 01-3-3z';
export const FOLDER_FRONT_BAND = 'M8 38.5h48V49a3 3 0 01-3 3H11a3 3 0 01-3-3z';
export const FOLDER_OPEN_FRONT =
	'M5 30.5a2.5 2.5 0 012.5-2.5h49a2.5 2.5 0 012.5 2.9L56.3 49.5a3 3 0 01-3 2.5H10.7a3 3 0 01-3-2.5z';
export const FOLDER_OPEN_BAND = 'M6.4 39.5h51.2l-1.3 10a3 3 0 01-3 2.5H10.7a3 3 0 01-3-2.5z';

/** The strokes drawn on a standard folder's front panel, in a 24 by 24 box. */
export const PORTAGE_FOLDER_GLYPHS = {
	home: 'M8.5 14.5l3.5-3 3.5 3M9.5 14v4h5v-4',
	desktop: 'M8.5 11.5h7v5h-7zM11 18h2',
	documents: 'M9.5 12.5h5M9.5 14.5h5M9.5 16.5h3',
	downloads: 'M12 11.5v5M9.8 14.5L12 16.7l2.2-2.2M9 18h6',
	pictures: 'M8.5 17l2.5-3 2 2 1.5-1.5 2 2.5M9.8 12.2h.01',
	music:
		'M10.5 17V12l5-1v5M10.5 17a1.4 1.4 0 11-2.8 0 1.4 1.4 0 012.8 0zM15.5 16a1.4 1.4 0 11-2.8 0 1.4 1.4 0 012.8 0z',
	videos: 'M9.5 12h5v5h-5zM14.5 13.5l2.5-1.2v4.4l-2.5-1.2',
	templates: 'M9 12h2.5v2.5H9zM12.5 12H15v2.5h-2.5zM9 15.5h2.5V18H9z',
	public:
		'M13.5 12a1.4 1.4 0 100 .01M9 15a1.4 1.4 0 100 .01M13.5 18a1.4 1.4 0 100 .01M10.3 14.3l2.9-1.6M10.3 15.7l2.9 1.6',
	projects: 'M10 11.5v6.5M10 13.5c0-1.5 5-.5 5-2.5M15 11a1 1 0 100-.01M10 18a1 1 0 100-.01',
} as const satisfies Record<SpecialFolder, string>;

export type PortageFolderGlyph = keyof typeof PORTAGE_FOLDER_GLYPHS;

/** A sticker drawn at the folder's lower right: a colour and a white glyph in a 35 by 35 box. */
export const PORTAGE_FOLDER_BADGES = {
	git: {
		colour: '#f05033',
		glyph: 'M16.3 15.4h.01M16.3 19.6h.01M16.3 15.6v3.8M19 15.4h.01M19 15.6c0 1.8-2.7 1.4-2.7 3',
	},
	remote: { colour: '#5a6b7d', glyph: 'M15 15h5v2h-5zM15 18h5v2h-5z' },
	encrypted: {
		colour: '#c9962e',
		glyph: 'M15.6 17.9h3.8v2.6h-3.8zM16.4 17.9v-.9a1.1 1.1 0 012.2 0v.9',
	},
	cloud: {
		colour: '#3d9be0',
		glyph: 'M15.8 19.6a1.5 1.5 0 010-3 2.2 2.2 0 014.2.4 1.3 1.3 0 010 2.6z',
	},
	link: { colour: '#4c7be8', glyph: 'M15.8 19.2l3.4-3.4M16.4 15.8h2.8v2.8' },
	shared: {
		colour: '#3fae5c',
		glyph: 'M19 15.4h.01M16 17.5h.01M19 19.6h.01M16.2 17.3l2.6-1.7M16.2 17.7l2.6 1.7',
	},
	trash: {
		colour: '#7d8794',
		glyph: 'M15.6 16.3h3.8M16.6 16.3v-.7h1.8v.7M16 16.3l.3 3.3h2.4l.3-3.3',
	},
} as const satisfies Record<string, { colour: string; glyph: string }>;

export type PortageFolderBadge = keyof typeof PORTAGE_FOLDER_BADGES;
