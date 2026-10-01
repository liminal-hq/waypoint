// The pure parts of renaming in place: where the selection starts, and whether the extension changed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/**
 * Splits a name into the part to edit and the extension to keep, as `split_name` in `waypoint-ops`
 * does: the extension is what follows the last dot (two parts for `.tar.gz` and its kin), and a
 * leading or a trailing dot starts none, so `.bashrc` and `end.` are all stem.
 */
export function splitName(name: string): { stem: string; extension: string } {
	const dot = name.lastIndexOf('.');
	if (dot <= 0 || dot + 1 === name.length) return { stem: name, extension: '' };
	const stem = name.slice(0, dot);
	const inner = stem.lastIndexOf('.');
	if (inner > 0 && stem.slice(inner).toLowerCase() === '.tar') {
		return { stem: name.slice(0, inner), extension: name.slice(inner) };
	}
	return { stem, extension: name.slice(dot) };
}

/**
 * The range of `name` that rename starts with selected: the stem of a file, so typing replaces the
 * name and keeps the extension, and the whole name of a folder (a dot in a folder's name is not an
 * extension).
 */
export function initialSelection(name: string, isFolder: boolean): [number, number] {
	return [0, isFolder ? name.length : splitName(name).stem.length];
}

/** What a rename does to a file's extension, for the guard. `null` when the extension is unchanged or there was none. */
export type ExtensionChange = { from: string; to: string };

/**
 * Whether renaming `from` to `to` changes the extension of a file that had one. A change of case
 * alone (`.JPG` to `.jpg`) is not asked about, and neither is giving an extensionless file one.
 * Folders have no extension to guard.
 */
export function extensionChange(
	from: string,
	to: string,
	isFolder: boolean,
): ExtensionChange | null {
	if (isFolder) return null;
	const before = splitName(from).extension;
	const after = splitName(to).extension;
	if (before === '' || before.toLowerCase() === after.toLowerCase()) return null;
	return { from: before, to: after };
}

/** `typed` with the original extension put back: what "Keep .jpg" renames to. */
export function withExtension(typed: string, extension: string): string {
	return splitName(typed).stem + extension;
}
