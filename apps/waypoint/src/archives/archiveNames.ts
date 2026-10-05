// Which files are archives Waypoint can open as folders, and what a new archive is called
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ArchiveFormat } from '@liminal-hq/waypoint-protocol/generated/ArchiveFormat';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';

/**
 * The names of archives the provider reads from the name when the first bytes do not say (mirrors
 * `from_name` in `waypoint-provider-archive`). A file with one of these ends is offered Open as
 * Folder and Extract; whether it really is one is the provider's to find.
 */
const ENDS = [
	'.zip',
	'.jar',
	'.apk',
	'.epub',
	'.docx',
	'.xlsx',
	'.pptx',
	'.odt',
	'.ods',
	'.7z',
	'.tar',
	'.tar.gz',
	'.tgz',
	'.tar.bz2',
	'.tbz',
	'.tbz2',
	'.tar.xz',
	'.txz',
	'.tar.zst',
	'.tzst',
	'.tar.zstd',
];

/** Whether `name` ends as a zip, tar or 7z archive does. */
export function isArchiveName(name: string): boolean {
	const lower = name.toLowerCase();
	return ENDS.some((end) => lower.endsWith(end));
}

/** Whether `entry` is a file that is named like an archive (a folder named `a.zip` is a folder). */
export function isArchiveEntry(entry: Entry): boolean {
	const file = entry.kind === 'file' || entry.linkTarget === 'file';
	return file && isArchiveName(entry.name);
}

/** Whether `location` is a place inside an archive. */
export function isArchiveLocation(location: Location): boolean {
	return location.uri.toLowerCase().startsWith('archive:');
}

/** The location of the top of the archive that `file` is: `archive:` and the file's address, then `!/`. */
export function archiveTopUri(file: Location): string {
	return `archive:${file.uri}!/`;
}

/** The archive file a location inside an archive is in (the outermost level's `!/` ends the container). */
export function containerUri(location: Location): string | null {
	if (!isArchiveLocation(location)) return null;
	const body = location.uri.slice('archive:'.length);
	// The last `!/` (or a final `!`) separates the container from the path inside it.
	const at = [...body.matchAll(/!(?:\/|$)/g)].at(-1)?.index;
	return at === undefined ? null : body.slice(0, at);
}

/** The formats Compress offers, in the order the menu lists them, with the words for each. */
export const COMPRESS_FORMATS: readonly ArchiveFormat[] = [
	'zip',
	'tarGz',
	'tarXz',
	'tarBz2',
	'tar',
	'sevenZ',
];

const EXTENSIONS: Record<ArchiveFormat, string> = {
	zip: '.zip',
	tar: '.tar',
	tarGz: '.tar.gz',
	tarBz2: '.tar.bz2',
	tarXz: '.tar.xz',
	sevenZ: '.7z',
};

/** The extension of the files `format` makes, with its dot. */
export function extensionOf(format: ArchiveFormat): string {
	return EXTENSIONS[format];
}

/** `name` with `format`'s extension on the end unless it already ends so (ignoring case). */
export function withExtension(name: string, format: ArchiveFormat): string {
	const end = extensionOf(format);
	return name.toLowerCase().endsWith(end) ? name : `${name}${end}`;
}

/** `name` without the extension of any format it ends in, so changing the format changes the end of the name only. */
export function stripArchiveEnd(name: string): string {
	const lower = name.toLowerCase();
	const end = Object.values(EXTENSIONS)
		.filter((extension) => lower.endsWith(extension) && name.length > extension.length)
		.sort((a, b) => b.length - a.length)[0];
	return end ? name.slice(0, name.length - end.length) : name;
}

/** What a new archive of `names` is called before the person chooses: the one name, or "Archive". */
export function defaultArchiveName(names: readonly string[], fallback: string): string {
	const [only] = names;
	return names.length === 1 && only ? stripFileExtension(only) : fallback;
}

/** A file's name without its last extension; a folder name or a dotfile keeps its whole name. */
function stripFileExtension(name: string): string {
	const dot = name.lastIndexOf('.');
	return dot > 0 ? name.slice(0, dot) : name;
}
