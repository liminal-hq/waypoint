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
	'window.main.startFailed': 'Waypoint could not start the file browser.',
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

	'tabs.strip.label': 'Tabs',
	'tabs.panel.label': 'Files',
	'tabs.new': 'New tab',
	'tabs.close': 'Close {title}',
	'tabs.scrollLeft': 'Scroll tabs left',
	'tabs.scrollRight': 'Scroll tabs right',
	'tabs.moved': 'Moved {title} to position {position} of {count}',
	'tabs.count.one': '{count} tab',
	'tabs.count.other': '{count} tabs',
	'tabs.menu.label': 'Tab actions',
	'tabs.menu.moveToNewWindow': 'Move to New Window',
	'tabs.menu.moveToWindow': 'Move to Window',
	'tabs.menu.windowEntry': '{title} — {tabs}',
	'tabs.menu.untitledWindow': 'Window',
	'tabs.announce.movedNewWindow': 'Moved to a new window',
	'tabs.announce.movedToWindow': 'Moved {name} to {window}',
	'tabs.announce.movedHere': 'Moved {name} to this window',
	'tabs.announce.movedManyHere.one': 'Moved {count} tab to this window',
	'tabs.announce.movedManyHere.other': 'Moved {count} tabs to this window',

	'window.notice.many':
		'Many windows are open. Each one uses memory, so close the ones you are done with.',
	'window.notice.limit': 'Waypoint cannot open more than {limit} windows. Close one first.',
	'window.notice.openFailed': 'Could not open a new window.',
	'window.notice.moveFailed': 'Could not move it to another window.',
	'tabs.pinned': 'Pinned',
	'tabs.pinnedBadge': 'Pinned tab',
	'tabs.colourDescription': 'Colour: {colour}',
	'tabs.announce.pinned': 'Pinned {title}',
	'tabs.announce.unpinned': 'Unpinned {title}',
	'tabs.announce.colour': 'Colour of {title} set to {colour}',
	'tabs.announce.colourCleared': 'Colour of {title} removed',
	'tabs.announce.reopened': 'Reopened {title}',
	'tabs.announce.noneClosed': 'There are no closed tabs to reopen',
	'tabs.announce.duplicated': 'Duplicated {title}',
	'tabs.announce.closedOthers': 'Closed the other tabs',
	'tabs.announce.closedRight': 'Closed the tabs to the right',
	'tabs.menu.pin': 'Pin Tab',
	'tabs.menu.unpin': 'Unpin Tab',
	'tabs.menu.colour': 'Colour',
	'tabs.menu.duplicate': 'Duplicate Tab',
	'tabs.menu.close': 'Close Tab',
	'tabs.menu.closeOthers': 'Close Other Tabs',
	'tabs.menu.closeRight': 'Close Tabs to the Right',
	'tabs.menu.reopen': 'Reopen Closed Tab',
	'tabs.menu.recentlyClosed': 'Recently Closed',
	'tabs.plusMenu.label': 'New tab actions',
	'tabs.plusMenu.newTab': 'New Tab',
	'tabs.plusMenu.newTabHome': 'New Tab at Home',
	'tabs.colour.none': 'None',
	'tabs.colour.red': 'Red',
	'tabs.colour.orange': 'Orange',
	'tabs.colour.yellow': 'Yellow',
	'tabs.colour.green': 'Green',
	'tabs.colour.teal': 'Teal',
	'tabs.colour.blue': 'Blue',
	'tabs.colour.purple': 'Purple',
	'tabs.colour.pink': 'Pink',
	'tabs.colour.grey': 'Grey',
	'tabs.switcher.label': 'Switch tab',
	'tabs.switcher.candidate': '{title}, tab {position} of {count}',
	'tabs.switcher.cancelled': 'Tab switch cancelled',

	'status.bar.label': 'Status bar',
	'status.items.one': '{count} item',
	'status.items.other': '{count} items',
	'status.free': '{size} free',
	'status.openFailed': 'Could not open {name}.',
	'status.copyPathFailed': 'Could not copy the path of {name}.',

	'menu.entry.label': 'Item actions',
	'menu.open': 'Open',
	'menu.openInNewTab': 'Open in New Tab',
	'menu.openInNewWindow': 'Open in New Window',
	'menu.copyPath': 'Copy Path',

	'view.switcher.label': 'View',
	'view.list': 'List',
	'view.grid': 'Grid',
	'view.withShortcut': '{name} ({keys})',
	'view.gridSize': 'Icon size',
	'view.gridSize.value': '{size} pixels',

	'sidebar.label': 'Sidebar',
	'sidebar.toggle': 'Sidebar',
	'sidebar.section.places': 'Places',
	'sidebar.section.favourites': 'Favourites',
	'sidebar.view.label': 'Sidebar view',
	'sidebar.view.places': 'Places',
	'sidebar.view.folders': 'Folders',
	'sidebar.place.home': 'Home',
	'sidebar.place.desktop': 'Desktop',
	'sidebar.place.documents': 'Documents',
	'sidebar.place.downloads': 'Downloads',
	'sidebar.place.pictures': 'Pictures',
	'sidebar.place.music': 'Music',
	'sidebar.place.videos': 'Videos',
	'sidebar.favourites.empty': 'No favourites yet. Add a folder with Add to Favourites.',
	'sidebar.folders.tree': 'Folders',
	'sidebar.folders.more': 'Showing the first {shown} of {total} folders',
	'sidebar.rename.label': 'Favourite name',
	'sidebar.moved': 'Moved {name} to position {position} of {count}',
	'sidebar.favourites.failed': 'Could not change the favourites.',
	'sidebar.menu.place': 'Place actions',
	'sidebar.menu.favourite': 'Favourite actions',
	'sidebar.menu.folder': 'Folder actions',
	'menu.rename': 'Rename',
	'menu.removeFromFavourites': 'Remove from Favourites',
	'menu.addToFavourites': 'Add to Favourites',
	'menu.moveUp': 'Move Up',
	'menu.moveDown': 'Move Down',

	'menu.background.label': 'Folder actions',
	'menu.sortBy': 'Sort by',
	'menu.sort.name': 'Name',
	'menu.sort.size': 'Size',
	'menu.sort.modified': 'Modified',
	'menu.sort.kind': 'Kind',
	'menu.sort.descending': 'Descending',
	'menu.sort.foldersFirst': 'Folders first',
	'menu.showHidden': 'Show hidden files',

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
