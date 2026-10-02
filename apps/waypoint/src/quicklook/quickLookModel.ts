// The pure rules of Quick Look: what an entry is previewed as, which entries can be stepped to, and where an arrow key goes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { MessageId } from '../i18n/messages';

/** How the overlay shows an entry. */
export type PreviewKind =
	| 'folder'
	| 'image'
	| 'text'
	| 'audio'
	| 'video'
	| 'pdf'
	| 'font'
	| 'archive'
	| 'document'
	| 'file';

const TEXT_EXTENSIONS: ReadonlySet<string> = new Set([
	'txt',
	'md',
	'markdown',
	'log',
	'json',
	'jsonc',
	'csv',
	'tsv',
	'xml',
	'yml',
	'yaml',
	'toml',
	'ini',
	'conf',
	'cfg',
	'rst',
	'tex',
	'html',
	'htm',
	'css',
	'svg',
	'sh',
	'env',
	'lock',
	'gitignore',
]);

const FONT_EXTENSIONS: ReadonlySet<string> = new Set(['ttf', 'otf', 'woff', 'woff2', 'ttc']);

/** The lower-case extension of a name, without the dot; empty for a name with none (and for a dotfile like `.bashrc`). */
export function extensionOf(name: string): string {
	const dot = name.lastIndexOf('.');
	return dot <= 0 ? '' : name.slice(dot + 1).toLowerCase();
}

/** The entry a link or file resolves to for previewing: a link to a folder is a folder. */
function isFolder(entry: Entry): boolean {
	return (
		entry.kind === 'directory' || (entry.kind === 'symlink' && entry.linkTarget === 'directory')
	);
}

/**
 * What to show for an entry, decided from what the listing already knows so the overlay opens
 * with no round trip. A file of an unknown kind with no extension is tried as text; if it is
 * binary the overlay falls back to the generic view.
 */
export function previewKindOf(entry: Entry): PreviewKind {
	if (isFolder(entry)) return 'folder';
	const extension = extensionOf(entry.name);
	if (extension === 'pdf') return 'pdf';
	if (FONT_EXTENSIONS.has(extension)) return 'font';
	switch (entry.group) {
		case 'image':
			// An SVG is text to read, but it also draws; the picture is the more useful preview.
			return 'image';
		case 'audio':
			return 'audio';
		case 'video':
			return 'video';
		case 'archive':
			return 'archive';
		case 'code':
			return 'text';
		default:
			break;
	}
	if (TEXT_EXTENSIONS.has(extension)) return 'text';
	if (entry.group === 'document') return 'document';
	return extension === '' ? 'text' : 'file';
}

/** The catalogue message for a preview kind's name. */
export function kindMessage(kind: PreviewKind, entry: Pick<Entry, 'kind'>): MessageId {
	if (kind === 'file' && entry.kind === 'symlink') return 'quickLook.kind.link';
	return `quickLook.kind.${kind}`;
}

/** Whether Quick Look can step to the entry: everything but sockets, pipes and devices. */
export function isPreviewable(entry: Pick<Entry, 'kind'>): boolean {
	return entry.kind !== 'other';
}

/** The entries whose pictures are asked for ahead of a step: the previewable ones either side. */
export function neighbourPositions(position: number, count: number): number[] {
	return [position + 1, position - 1].filter((at) => at >= 0 && at < count);
}

/**
 * Where an arrow key steps from `from`, or `null` when it does not step. The view's own
 * navigation decides first (so the grid's Up and Down go a row), and Left and Right step by one
 * in either view. Keys the view uses for paging are not steps.
 */
export function stepTarget(
	key: string,
	from: number,
	last: number,
	move: (key: string, from: number | null, last: number) => number | null,
): number | null {
	if (key !== 'ArrowLeft' && key !== 'ArrowRight' && key !== 'ArrowUp' && key !== 'ArrowDown') {
		return null;
	}
	const target = move(key, from, last);
	if (target !== null) return target === from ? null : Math.max(0, Math.min(last, target));
	if (key === 'ArrowLeft') return from > 0 ? from - 1 : null;
	if (key === 'ArrowRight') return from < last ? from + 1 : null;
	return null;
}
