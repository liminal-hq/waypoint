// Which system icon an entry asks for: its extension, or a stand-in type for its group, or a kind of folder, and the size bucket to ask at
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { FolderKind, TypeIconTarget } from '@liminal-hq/plugin-mime-apps';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';

/**
 * The type the system is asked about for a file with no extension to go by, one per icon group. The
 * generic ones (`image/x-generic`) are the types themes name their per-kind icons after; the others
 * are the commonest type of the group, so the group's icon is what the system draws for it.
 */
export const GROUP_TYPES: Record<Exclude<IconGroup, 'folder'>, string> = {
	image: 'image/x-generic',
	audio: 'audio/x-generic',
	video: 'video/x-generic',
	archive: 'application/zip',
	code: 'text/x-csrc',
	document: 'application/vnd.oasis.opendocument.text',
	other: 'application/octet-stream',
	pdf: 'application/pdf',
	app: 'application/x-executable',
	text: 'text/plain',
	markdown: 'text/markdown',
	spreadsheet: 'application/vnd.oasis.opendocument.spreadsheet',
	presentation: 'application/vnd.oasis.opendocument.presentation',
	font: 'font/ttf',
	diskImage: 'application/x-iso9660-image',
	database: 'application/vnd.sqlite3',
	config: 'text/plain',
	shellScript: 'application/x-shellscript',
	executable: 'application/x-executable',
	certificate: 'application/x-x509-ca-cert',
	ebook: 'application/epub+zip',
	torrent: 'application/x-bittorrent',
	calendar: 'text/calendar',
	contact: 'text/vcard',
	log: 'text/x-log',
	model3d: 'model/stl',
	subtitles: 'application/x-subrip',
	playlist: 'audio/x-mpegurl',
	package: 'application/vnd.debian.binary-package',
	symlink: 'inode/symlink',
};

/** The standard folders the system draws its own icon for; any other mark (Projects) is a plain folder. */
const STANDARD_FOLDERS: ReadonlySet<string> = new Set([
	'home',
	'desktop',
	'documents',
	'downloads',
	'pictures',
	'music',
	'videos',
	'templates',
	'public',
]);

/** What an extension may be to be asked for: the characters of a file extension, and no longer than a real one. */
const EXTENSION = /^[a-z0-9_+~-]{1,24}$/;

/** The extension of a name in lower case, or `null` when it has none a type could go by (a leading dot alone is part of the name). */
export function extensionOf(name: string | undefined): string | null {
	if (!name) return null;
	const dot = name.lastIndexOf('.');
	if (dot <= 0 || dot === name.length - 1) return null;
	const extension = name.slice(dot + 1).toLowerCase();
	return EXTENSION.test(extension) ? extension : null;
}

/**
 * What to ask the system for. A folder is asked for by its kind. A file goes by its extension, so every
 * `.pdf` in a listing is one request; one with no extension, or one the name tables know better (a
 * name alone), goes by a stand-in type for its group. Never by name: the answer is per type.
 */
export function systemIconTarget(
	group: IconGroup,
	name?: string,
	special?: SpecialFolder | null,
): TypeIconTarget {
	if (group === 'folder') {
		return { folder: special && STANDARD_FOLDERS.has(special) ? (special as FolderKind) : 'plain' };
	}
	const extension = extensionOf(name);
	return extension ? { extension } : { mime: GROUP_TYPES[group] };
}

/** The sizes a system icon is asked at, in CSS pixels, so a few requests per type serve every view. */
export const ICON_SIZES = [16, 24, 32, 48, 64, 96, 128, 256] as const;

/** The smallest size in `ICON_SIZES` that holds `cssPixels`, so a picture is only ever scaled down. */
export function iconSizeFor(cssPixels: number): number {
	const needed = Number.isFinite(cssPixels) ? cssPixels : 16;
	return ICON_SIZES.find((size) => needed <= size) ?? 256;
}

/** The device pixel ratio to ask at, a whole number from 1 to 3 (the plugin draws whole pixels). */
export function iconScale(pixelRatio: number): number {
	return Math.min(3, Math.max(1, Math.ceil(Number.isFinite(pixelRatio) ? pixelRatio : 1)));
}
