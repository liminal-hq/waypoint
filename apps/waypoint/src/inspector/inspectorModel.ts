// The pure rules of the Inspector: what kind of preview a file gets, and how its facts read
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { EntryKind } from '@liminal-hq/waypoint-protocol/generated/EntryKind';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { MessageId } from '../i18n/messages';

/** How long a selection holds still before the details are read: cheap, so a keypress shows them at once. */
export const DETAILS_DELAY_MS = 20;

/** How long it holds still before anything heavy starts: a folder total, a text head, a media element, the default app. */
export const HEAVY_DELAY_MS = 150;

/** The most of a text file the preview reads, and so the most it ever shows. */
export const TEXT_HEAD_BYTES = 16 * 1024;

/** A picture larger than this is shown as its thumbnail: the whole file is not worth loading for a preview. */
export const IMAGE_MAX_BYTES = 20 * 1000 * 1000;

export type PreviewKind = 'image' | 'text' | 'audio' | 'video' | 'none';

const TEXT_APPLICATION = new Set([
	'application/json',
	'application/xml',
	'application/javascript',
	'application/x-sh',
	'application/x-shellscript',
	'application/toml',
	'application/x-yaml',
	'application/yaml',
	'application/sql',
]);

/**
 * What the preview shows for a file. The icon group decides until the content type is known
 * (an image, a sound or a video is those at once), and the content type decides after: text is
 * only text once something says so, since reading a binary file to find out is what "never loads
 * more than needed" rules out.
 */
export function previewKind(group: IconGroup, mime: string | null): PreviewKind {
	if (mime) {
		if (mime.startsWith('image/')) return 'image';
		if (mime.startsWith('audio/')) return 'audio';
		if (mime.startsWith('video/')) return 'video';
		if (
			mime.startsWith('text/') ||
			TEXT_APPLICATION.has(mime) ||
			mime.endsWith('+json') ||
			mime.endsWith('+xml')
		) {
			return 'text';
		}
		return 'none';
	}
	switch (group) {
		case 'image':
		case 'audio':
		case 'video':
			return group;
		case 'code':
			return 'text';
		default:
			return 'none';
	}
}

/** Whether an entry is a folder, or a link that resolves to one. */
export function isFolderKind(kind: EntryKind, resolvesTo: EntryKind | null): boolean {
	return kind === 'directory' || (kind === 'symlink' && resolvesTo === 'directory');
}

const GROUP_KIND: Record<IconGroup, MessageId> = {
	folder: 'inspector.kind.folder',
	image: 'inspector.kind.image',
	audio: 'inspector.kind.audio',
	video: 'inspector.kind.video',
	archive: 'inspector.kind.archive',
	code: 'inspector.kind.code',
	document: 'inspector.kind.document',
	other: 'inspector.kind.file',
};

/** The message that says what an entry is: a folder, a link, or the kind its icon group stands for. */
export function kindMessage(kind: EntryKind, group: IconGroup): MessageId {
	if (kind === 'directory') return 'inspector.kind.folder';
	if (kind === 'symlink') return 'inspector.kind.link';
	if (kind === 'other') return 'inspector.kind.special';
	return GROUP_KIND[group];
}

/** `rwxr-xr-x` for the nine permission bits, with `s`, `S`, `t` and `T` where setuid, setgid and sticky are set. */
export function permissionString(mode: number): string {
	const bit = (mask: number, letter: string) => ((mode & mask) !== 0 ? letter : '-');
	const exec = (mask: number, special: number, set: string, unset: string) => {
		const run = (mode & mask) !== 0;
		if ((mode & special) === 0) return run ? 'x' : '-';
		return run ? set : unset;
	};
	return [
		bit(0o400, 'r'),
		bit(0o200, 'w'),
		exec(0o100, 0o4000, 's', 'S'),
		bit(0o040, 'r'),
		bit(0o020, 'w'),
		exec(0o010, 0o2000, 's', 'S'),
		bit(0o004, 'r'),
		bit(0o002, 'w'),
		exec(0o001, 0o1000, 't', 'T'),
	].join('');
}

/** The permissions as people read them: `rwxr-xr-x (0755)`. */
export function formatPermissions(mode: number): string {
	const octal = (mode & 0o7777).toString(8).padStart(4, '0');
	return `${permissionString(mode)} (${octal})`;
}

/** The last part of a path as shown, or all of it for a root. */
export function baseName(display: string): string {
	const trimmed = display.replace(/[\\/]+$/, '');
	const at = Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\'));
	return trimmed === '' ? display : trimmed.slice(at + 1) || trimmed;
}
