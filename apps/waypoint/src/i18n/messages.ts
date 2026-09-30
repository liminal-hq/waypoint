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
