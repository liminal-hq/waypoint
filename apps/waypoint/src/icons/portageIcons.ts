// Maps icon groups and standard folders to Portage artwork, and draws the SVG for either
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import type { PortageFolderBadge, PortageFolderGlyph } from './portage/portageFolderArt';
import { portageFolderSvg, type PortageFolderVariant } from './portage/portageFolderSvg';
import { PORTAGE_FILE_SVGS, type PortageFileName } from './portage/portageFiles';
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
}

/** The standalone 64 by 64 SVG for an entry. Colour, tone, variant and badge only change folders. */
export function portageIconSvg(options: PortageIconOptions): string {
	const { group, special, colour, tone, variant, badge } = options;
	if (group !== 'folder') return PORTAGE_FILE_SVGS[PORTAGE_FILE_ART[group]];
	return portageFolderSvg({
		colour,
		tone,
		variant,
		badge,
		glyph: special ? PORTAGE_STANDARD_FOLDER_GLYPHS[special] : undefined,
	});
}
