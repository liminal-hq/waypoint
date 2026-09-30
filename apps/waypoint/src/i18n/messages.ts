// The message catalogue: every user-visible string the app shows, keyed by a stable identifier
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/**
 * English, the only catalogue that ships for now. Screens and the chrome's labels read their copy
 * from here through `t()`, never from a literal in JSX, so a translated catalogue with the same
 * keys can replace this one without touching a component. Keys are `area.thing`, and a value is
 * plain text: pluralisation and interpolation arrive with the localisation library chosen for
 * the catalogue's next stage.
 */
export const enMessages = {
	'window.main.title': 'Waypoint — Main',
	'window.main.description': 'The tabbed file browser — coming soon.',
	'window.settings.title': 'Waypoint — Settings',
	'window.settings.description': 'Application settings — coming soon.',
	'window.properties.title': 'Waypoint — Properties',
	'window.properties.description': 'File properties — coming soon.',
	'window.ops.title': 'Waypoint — Operations',
	'window.ops.description': 'Operations and jobs — coming soon.',
	'window.tearGhost.title': 'Waypoint — Tab preview',
	'window.tearGhost.description': 'Tab tear-off preview — coming soon.',

	'browse.list.label': 'Files',
	'browse.column.name': 'Name',
	'browse.column.size': 'Size',
	'browse.column.modified': 'Modified',
	'browse.column.kind': 'Kind',
	'browse.columns.label': 'Sort the list',
	'browse.sort.ascending': 'sorted ascending',
	'browse.sort.descending': 'sorted descending',
	'browse.value.none': '—',
	'browse.row.loading': 'Loading',
	'browse.group.folder': 'Folder',
	'browse.group.image': 'Image',
	'browse.group.audio': 'Audio',
	'browse.group.video': 'Video',
	'browse.group.archive': 'Archive',
	'browse.group.code': 'Code',
	'browse.group.document': 'Document',
	'browse.group.other': 'File',
	'browse.opening': 'Opening folder…',
	'browse.scanning': 'Scanning… {count} items found so far',
	'browse.empty': 'This folder is empty.',
	'browse.capped': 'Showing the first {shown} of {total} items',
	'browse.error.notFound.title': 'Folder not found',
	'browse.error.notFound.detail': '{location} does not exist, or it was moved or deleted.',
	'browse.error.permissionDenied.title': 'Permission denied',
	'browse.error.permissionDenied.detail': 'You do not have permission to open {location}.',
	'browse.error.notADirectory.title': 'Not a folder',
	'browse.error.notADirectory.detail': '{location} is a file, not a folder.',
	'browse.error.other.title': 'This folder could not be shown',
	'browse.error.other.detail': 'Something went wrong while reading the folder.',
	'browse.selection.none': 'No items selected',
	'browse.selection.one': '{count} item selected',
	'browse.selection.other': '{count} items selected',

	'nav.toolbar.label': 'Navigation',
	'nav.back': 'Back',
	'nav.forward': 'Forward',
	'nav.up': 'Up',
	'nav.history.back': 'Folders behind',
	'nav.history.forward': 'Folders ahead',
	'nav.path.crumbs': 'Location',
	'nav.path.edit': 'Edit location',
	'nav.path.input': 'Type a location and press Enter',
	'nav.path.invalid': '“{input}” is not a location Waypoint can open.',
	'nav.path.unsupported': 'Waypoint cannot open {what} locations yet.',
	'nav.path.failed': 'That location could not be checked.',

	'dev.live.label': 'Live update controls (development only)',
	'dev.live.add': 'Add 5 files',
	'dev.live.addTop': 'Add 5 at the top',
	'dev.live.remove': 'Remove 5 files',
	'dev.live.touch': 'Touch 5 files',
	'dev.live.auto': 'Change continuously',

	'chrome.restore': 'Restore',
	'chrome.maximise': 'Maximise',
	'chrome.minimise': 'Minimise',
	'chrome.move': 'Move',
	'chrome.alwaysOnTop': 'Always on Top',
	'chrome.systemWindowMenu': 'More options…',
	'chrome.close': 'Close',
	'chrome.windowMenu': 'Window menu',
	'chrome.windowControls': 'Window controls',
} as const;

export type MessageId = keyof typeof enMessages;

/** The message for `id` in the active catalogue. */
export function t(id: MessageId): string {
	return enMessages[id];
}

/** The message for `id` with each `{name}` token replaced by its value. */
export function tf(id: MessageId, values: Record<string, string | number>): string {
	return enMessages[id].replace(/\{(\w+)\}/g, (token, name: string) =>
		name in values ? String(values[name]) : token,
	);
}

/** Message identifiers that have a `.one` and an `.other` form, named by their shared prefix. */
export type PluralId = MessageId extends infer K
	? K extends `${infer Base}.other`
		? Base
		: never
	: never;

/**
 * The message for `count` of something, choosing the plural form the locale's rules give and
 * replacing `{count}` with the number formatted for that locale.
 */
export function tn(id: PluralId, count: number, locale?: string): string {
	const form = new Intl.PluralRules(locale).select(count);
	const key = `${id}.${form === 'one' ? 'one' : 'other'}` as MessageId;
	return tf(key, { count: new Intl.NumberFormat(locale).format(count) });
}
