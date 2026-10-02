// What files dragged in from another application or window are, as a file drag's source: pure rules over the plugin's URIs and paths
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import { normaliseUri } from '../ops/clipboardRules';
import type { Location } from '../services/opsClient';
import type { FileDragSource } from './fileDragModel';

/** The icons of a stack of files whose kinds are not known (no listing has them): plain pages, up to three. */
const STACK: IconGroup[] = ['document', 'document', 'document'];

/** The last part of a path, whichever separator it uses; the display strings the plugin sends follow the platform. */
export function leafOfPath(path: string): string {
	const trimmed = path.replace(/[\\/]+$/, '');
	return trimmed.slice(Math.max(trimmed.lastIndexOf('/'), trimmed.lastIndexOf('\\')) + 1);
}

/** A `file:` URI as a path to show, for a drop whose display paths did not come with it. */
function displayOf(uri: string): string {
	return normaliseUri(uri)
		.replace(/^file:\/\//, '')
		.replace(/^\/([A-Za-z]:)/, '$1');
}

/**
 * The files of a drag that the page can act on: the `file:` URIs (a link or text dragged in from a
 * browser carries other schemes and is not a file drag), each with the display path the plugin
 * gave when the two lists line up, and otherwise the URI's own text.
 */
export function droppedFiles(uris: readonly string[], paths: readonly string[]): Location[] {
	const aligned = paths.length === uris.length;
	return uris.flatMap((uri, index) =>
		uri.startsWith('file:')
			? [{ display: (aligned ? paths[index] : undefined) ?? displayOf(uri), uri }]
			: [],
	);
}

/**
 * The folder every file is in, in the lossless form of its URI, or `null` when they are not all in
 * one folder (or one is a root). Only the text of the URIs is read: Rust is asked about nothing,
 * since the answer is only used to see that a drop would put the files where they already are.
 */
export function commonFolder(files: readonly Location[]): Location | null {
	let folder: string | null = null;
	for (const file of files) {
		const flat = file.uri.replace(/\/+$/, '');
		const cut = flat.lastIndexOf('/');
		const scheme = flat.indexOf('://');
		if (cut < 0 || scheme < 0 || cut <= scheme + 2) return null;
		const parent = flat.slice(0, cut) || null;
		if (parent === null || (folder !== null && normaliseUri(parent) !== normaliseUri(folder))) {
			return null;
		}
		folder = parent;
	}
	if (folder === null) return null;
	const shown = normaliseUri(folder).replace(/^file:\/\//, '');
	// A root keeps its trailing slash: `file:///` and a drive, `file:///C:/`.
	const root = folder.endsWith('://') || /^file:\/\/\/[A-Za-z]:$/.test(folder);
	return { display: shown === '' ? '/' : shown, uri: root ? `${folder}/` : folder };
}

/** Whether two lists name the same files, in any order and in any spelling of the URI. */
export function sameUris(a: readonly string[], b: readonly string[]): boolean {
	if (a.length !== b.length) return false;
	const left = a.map(normaliseUri).sort();
	const right = b.map(normaliseUri).sort();
	return left.every((uri, index) => uri === right[index]);
}

/**
 * The source of a drag of files that come from outside this window's listings. The files are named
 * by location; the folder they are in is known when they share one, which is what refuses a move
 * onto the folder they are already in. Nothing is known about them beyond their names, so the
 * stack shows plain pages, and a drop never moves them out of a folder that cannot be written to
 * (the other side decides that).
 */
export function externalSource(files: readonly Location[], own = false): FileDragSource {
	const count = files.length;
	return {
		session: null,
		tab: null,
		handle: null,
		spec: null,
		external: { locations: [...files], own },
		count,
		name: count === 1 ? leafOfPath(files[0]!.display) : null,
		groups: STACK.slice(0, Math.min(count, 3)),
		folder: commonFolder(files),
		readOnly: false,
		rightButton: false,
	};
}
