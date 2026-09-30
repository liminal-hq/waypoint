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
