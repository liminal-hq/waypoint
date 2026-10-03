// Maps icon groups and standard folders to Portage artwork, and draws the SVG for either
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import type { PortageFolderBadge, PortageFolderGlyph } from './portage/portageFolderArt';
import { portageFolderSvg, type PortageFolderVariant } from './portage/portageFolderSvg';
import { portageFileSvg, type PortageFileName } from './portage/portageFiles';
import type { FolderColour, FolderTone } from './portage/portagePalette';

// The folder colour ids (`liminal | gnome | … | rainbow`), defined with their palette.
export { FOLDER_COLOURS, type FolderColour } from './portage/portagePalette';

/**
 * The artwork for every icon group but `folder`, which is drawn from the folder family instead.
 * `Record` keys make a group Rust adds without art a compile error.
 */
export const PORTAGE_FILE_ART: Record<Exclude<IconGroup, 'folder'>, PortageFileName> = {
	image: 'image',
	audio: 'audio',
	video: 'video',
	archive: 'archive',
	code: 'code',
	document: 'document',
	pdf: 'pdf',
	app: 'app',
	text: 'text',
	markdown: 'markdown',
	spreadsheet: 'spreadsheet',
	presentation: 'presentation',
	font: 'font',
	diskImage: 'disk-image',
	database: 'database',
	config: 'config',
	shellScript: 'shell-script',
	executable: 'executable',
	certificate: 'certificate',
	ebook: 'ebook',
	torrent: 'torrent',
	calendar: 'calendar',
	contact: 'contact',
	log: 'log',
	model3d: 'model-3d',
	subtitles: 'subtitles',
	playlist: 'playlist',
	package: 'package',
	symlink: 'symlink',
	other: 'blank',
};

/**
 * Groups that Portage has no icon of their own for, and the art each one uses. `other` is any file
 * the tables could not classify: the prototype's closest is its "Unknown" page with a question mark,
 * which would put one on every unclassified file, so Waypoint draws a plain page of the same shape
 * (`files/blank.svg`: the page, its lower band, fold and highlight, with no label).
 */
export const PORTAGE_FALLBACK_ART: Partial<Record<IconGroup, PortageFileName>> = {
	other: 'blank',
};

/** The mark each standard folder wears on its front panel. */
export const PORTAGE_STANDARD_FOLDER_GLYPHS: Record<SpecialFolder, PortageFolderGlyph> = {
	home: 'home',
	desktop: 'desktop',
	documents: 'documents',
	downloads: 'downloads',
	pictures: 'pictures',
	music: 'music',
	videos: 'videos',
	templates: 'templates',
	public: 'public',
	projects: 'projects',
};

export interface PortageIconOptions {
	group: IconGroup;
	/** Which standard folder of the user's a `folder` entry is. Ignored for any other group. */
	special?: SpecialFolder | null;
	colour?: FolderColour;
	tone?: FolderTone;
	variant?: PortageFolderVariant;
	badge?: PortageFolderBadge;
	/** Draw the soft shadow under the icon. Off by default (see `portageFolderSvg`). */
	shadow?: boolean;
}

const SVG_CACHE = new Map<string, string>();
const MARKUP_CACHE = new Map<string, string>();

/** What decides the art: a file's group alone, a folder's standard folder, colour, tone, variant and badge. */
function cacheKey(options: PortageIconOptions): string {
	const { group, special, colour, tone, variant, badge, shadow } = options;
	const shade = shadow ? 's' : '';
	if (group !== 'folder') return `${group}|${shade}`;
	return [group, special, colour, tone, variant, badge, shade].map((part) => part ?? '').join('|');
}

/**
 * The standalone 64 by 64 SVG for an entry. Colour, tone, variant and badge only change folders.
 * Built once per distinct icon and the same string instance returned after, so a listing of
 * thousands of rows builds a handful of strings.
 */
export function portageIconSvg(options: PortageIconOptions): string {
	const key = cacheKey(options);
	let svg = SVG_CACHE.get(key);
	if (svg === undefined) {
		const { group, special, colour, tone, variant, badge, shadow = false } = options;
		svg =
			group !== 'folder'
				? portageFileSvg(PORTAGE_FILE_ART[group], shadow)
				: portageFolderSvg({
						colour,
						tone,
						variant,
						badge,
						shadow,
						glyph: special ? PORTAGE_STANDARD_FOLDER_GLYPHS[special] : undefined,
					});
		SVG_CACHE.set(key, svg);
	}
	return svg;
}

/** The inside of an `<svg>`: everything between its opening and closing tags. */
function innerMarkup(svg: string): string {
	return svg.slice(svg.indexOf('>') + 1, svg.lastIndexOf('</svg>'));
}

/** The art for an entry as the contents of an `<svg viewBox="0 0 64 64">`, memoised like `portageIconSvg`. */
export function portageIconMarkup(options: PortageIconOptions): string {
	const key = cacheKey(options);
	let markup = MARKUP_CACHE.get(key);
	if (markup === undefined) {
		markup = innerMarkup(portageIconSvg(options));
		MARKUP_CACHE.set(key, markup);
	}
	return markup;
}
