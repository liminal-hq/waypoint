// The clipboard's rules as pure functions: which rows a cut dims, what a paste refuses, the request a paste makes and which clipboard wins
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import type { Clipboard, JobRequest, Location } from '../services/opsClient';
import type { OsFiles } from '../services/osClipboardClient';

// --- Locations as text -----------------------------------------------------------------------
//
// Rust owns paths, so none of this decides anything about a file system. It only compares the
// `uri`s Rust handed out: the clipboard's items against a listing's folder and a paste's
// destination. A `uri` is percent-encoded, so two spellings of one name compare equal once decoded.

function decode(text: string): string {
	try {
		return decodeURIComponent(text);
	} catch {
		return text;
	}
}

/** A `uri` with its trailing slashes dropped and its escapes decoded, so equal places compare equal. */
export function normaliseUri(uri: string): string {
	const trimmed = uri.replace(/\/+$/, '');
	return decode(trimmed === '' || trimmed.endsWith(':') ? uri : trimmed);
}

/** The folder a `uri` sits in, in the same normalised form; `null` for a root, which has none. */
export function parentUri(uri: string): string | null {
	const flat = normaliseUri(uri);
	const cut = flat.lastIndexOf('/');
	const scheme = flat.indexOf('://');
	if (cut < 0 || (scheme >= 0 && flat.length <= scheme + 4)) return null;
	const parent = flat.slice(0, cut);
	return scheme >= 0 && parent.length === scheme + 3 ? `${parent}/` : parent || null;
}

/** The name at the end of a `uri` (decoded). */
export function leafName(uri: string): string {
	const flat = normaliseUri(uri);
	return flat.slice(flat.lastIndexOf('/') + 1);
}

/** Whether `inner` is `outer` or lies inside it. */
export function isWithin(inner: string, outer: string): boolean {
	const a = normaliseUri(inner);
	const b = normaliseUri(outer);
	return a === b || a.startsWith(b.endsWith('/') ? b : `${b}/`);
}

// --- Dimming ---------------------------------------------------------------------------------

/**
 * The names a cut dims in the folder `folderUri`: the clipboard's items that sit directly in it.
 * Empty unless the clipboard holds a cut, so a copy dims nothing.
 */
export function cutNames(clipboard: Clipboard, folderUri: string): ReadonlySet<string> {
	if (clipboard.mode !== 'cut' || clipboard.items.length === 0) return NONE;
	const folder = normaliseUri(folderUri);
	const names = new Set<string>();
	for (const item of clipboard.items) {
		if (parentUri(item.uri) === folder) names.add(leafName(item.uri));
	}
	return names.size === 0 ? NONE : names;
}

const NONE: ReadonlySet<string> = new Set();

// --- What a paste refuses and what it asks for -----------------------------------------------

/**
 * Why `items` cannot be pasted into `destination`, as the typed refusal the planner would give, or
 * `null` when nothing is wrong. The planner is the authority and checks again; this finds the plain
 * cases without queueing a job that would only fail:
 * - a folder cannot be put inside itself, or into a folder within it (`intoItself`);
 * - a cut whose every item is already in the destination has nothing to move (`sameFolder`).
 */
export function pasteRefusal(
	mode: Clipboard['mode'],
	items: readonly Location[],
	destination: Location,
): OpsError | null {
	if (items.some((item) => isWithin(destination.uri, item.uri))) return { kind: 'intoItself' };
	if (mode === 'cut' && items.length > 0 && items.every((item) => isIn(item, destination))) {
		return { kind: 'sameFolder' };
	}
	return null;
}

/** Whether `item` sits directly in `destination`. */
export function isIn(item: Location, destination: Location): boolean {
	return parentUri(item.uri) === normaliseUri(destination.uri);
}

/**
 * The job a paste submits. A cut moves. A copy copies, except when every item is already in the
 * destination: that is a copy beside the original, so it is a duplicate (`name (2)`), as it is in
 * other file managers. Conflicts are left to the planner and the person: nothing is overwritten
 * without a decision.
 */
export function pasteRequest(
	clipboard: Pick<Clipboard, 'mode' | 'items'>,
	destination: Location,
	windowLabel: string,
): JobRequest {
	const inPlace =
		clipboard.mode === 'copy' &&
		clipboard.items.length > 0 &&
		clipboard.items.every((item) => isIn(item, destination));
	return {
		kind: { kind: inPlace ? 'duplicate' : clipboard.mode === 'cut' ? 'move' : 'copy' },
		sources: { kind: 'locations', locations: [...clipboard.items] },
		destination: inPlace ? null : destination,
		name: null,
		options: { conflict: null, verify: null },
		originWindow: windowLabel,
	};
}

// --- Which clipboard wins --------------------------------------------------------------------

/** The clipboard's `file:` items as the system clipboard holds them, or `null` when it has none to share. */
export function clipboardAsFiles(clipboard: Clipboard): OsFiles | null {
	const uris = clipboard.items.map((item) => item.uri).filter((uri) => uri.startsWith('file:'));
	return uris.length === 0 ? null : { uris, cut: clipboard.mode === 'cut' };
}

/** The `file:` URIs the clipboard's items have once pasted (copied or moved) into `destination`, each keeping its last path segment. */
export function pastedUris(items: readonly Location[], destination: Location): string[] {
	const base = destination.uri.replace(/\/+$/, '');
	return items
		.filter((item) => item.uri.startsWith('file:') && destination.uri.startsWith('file:'))
		.map((item) => `${base}/${item.uri.replace(/\/+$/, '').split('/').pop() ?? ''}`);
}

/** Whether two file lists are the same files with the same intent, whatever the order or the spelling of the `uri`s. */
export function sameFiles(a: OsFiles | null, b: OsFiles | null): boolean {
	if (a === null || b === null) return a === b;
	if (a.cut !== b.cut || a.uris.length !== b.uris.length) return false;
	const left = a.uris.map(normaliseUri).sort();
	const right = b.uris.map(normaliseUri).sort();
	return left.every((uri, index) => uri === right[index]);
}

/** Which clipboard a paste uses. */
export type ClipboardChoice = 'app' | 'os';

export interface ClipboardEvidence {
	/** The system file clipboard works here. */
	available: boolean;
	/** Waypoint's clipboard (Rust's). */
	app: Clipboard;
	/** What the system clipboard holds now: `null` for nothing, or for content that is not files. */
	os: OsFiles | null;
	/** What the system clipboard held the last time this window looked, or put there itself. */
	lastSeen: OsFiles | null;
}

/**
 * The rule that decides which clipboard a paste (or a window gaining focus) uses:
 * 1. Where the system clipboard is unavailable, Waypoint's is the only one.
 * 2. Nothing, or something that is not files, on the system clipboard is ignored: Waypoint's stands.
 * 3. The same files as Waypoint's: nothing differs, Waypoint's stands.
 * 4. The same files as the last time this window looked: they were adopted or passed over already,
 *    and Waypoint's has been set since (a system clipboard that refused the write), so it stands.
 * 5. Otherwise another application copied files since: the system clipboard wins, and is adopted
 *    (copy or cut as it says) into Waypoint's with `source: 'os'`.
 */
export function chooseClipboard(evidence: ClipboardEvidence): ClipboardChoice {
	const { available, app, os, lastSeen } = evidence;
	if (!available || os === null || os.uris.length === 0) return 'app';
	if (sameFiles(os, clipboardAsFiles(app))) return 'app';
	if (sameFiles(os, lastSeen)) return 'app';
	return 'os';
}
