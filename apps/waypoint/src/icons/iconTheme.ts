// Which icon set the window draws entries in, read live from the attributes the theme engine writes on the root
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useSyncExternalStore } from 'react';
import type { IconTheme } from '@liminal-hq/waypoint-protocol/generated/IconTheme';
import { FOLDER_COLOURS, type FolderColour, type FolderTone } from './portage/portagePalette';

/**
 * The sets the views can draw. `system` draws the operating system's own icons where it can supply them
 * and the Waypoint glyph for any icon it cannot (and for all of them where it supplies none).
 */
export type ResolvedIconTheme = 'waypoint' | 'portage' | 'system';

/** The set a setting value draws as. */
export function resolveIconTheme(choice: IconTheme): ResolvedIconTheme {
	return choice === 'portage' || choice === 'system' ? choice : 'waypoint';
}

/** Everything an entry's icon needs to pick its art. */
export interface IconLook {
	theme: ResolvedIconTheme;
	colour: FolderColour;
	tone: FolderTone;
}

/** The look for a theme, a colour and the window's resolved scheme (`null` follows the OS). */
export function iconLook(
	theme: string | undefined,
	colour: string | undefined,
	scheme: string | undefined,
	osDark: boolean,
): IconLook {
	return {
		theme: theme === 'portage' || theme === 'system' ? theme : 'waypoint',
		colour: (FOLDER_COLOURS as readonly string[]).includes(colour ?? '')
			? (colour as FolderColour)
			: 'liminal',
		tone: scheme === 'dark' || (scheme !== 'light' && osDark) ? 'dark' : 'light',
	};
}

const DARK_QUERY = '(prefers-color-scheme: dark)';

function osPrefersDark(): boolean {
	return typeof globalThis.matchMedia === 'function' && globalThis.matchMedia(DARK_QUERY).matches;
}

/** The look in force on `root`: `data-icon-theme`, `data-folder-colour` and the resolved `data-theme`. */
export function readIconLook(root: HTMLElement = document.documentElement): IconLook {
	const { iconTheme, folderColour, theme } = root.dataset;
	return iconLook(iconTheme, folderColour, theme, theme ? false : osPrefersDark());
}

const listeners = new Set<() => void>();
let observer: MutationObserver | null = null;
let media: MediaQueryList | null = null;
let snapshot: IconLook | null = null;

function notify(): void {
	for (const listener of listeners) listener();
}

/** The current look, the same object until something in it changes so a row does not re-render for nothing. */
function getSnapshot(): IconLook {
	const next = readIconLook();
	if (
		snapshot &&
		snapshot.theme === next.theme &&
		snapshot.colour === next.colour &&
		snapshot.tone === next.tone
	) {
		return snapshot;
	}
	snapshot = next;
	return next;
}

/** One observer and one media query serve every icon on the page, however many rows are showing. */
function subscribe(listener: () => void): () => void {
	listeners.add(listener);
	if (listeners.size === 1) {
		observer = new MutationObserver(notify);
		observer.observe(document.documentElement, {
			attributes: true,
			attributeFilter: ['data-icon-theme', 'data-folder-colour', 'data-theme'],
		});
		if (typeof globalThis.matchMedia === 'function') {
			media = globalThis.matchMedia(DARK_QUERY);
			media.addEventListener?.('change', notify);
		}
	}
	return () => {
		listeners.delete(listener);
		if (listeners.size === 0) {
			observer?.disconnect();
			observer = null;
			media?.removeEventListener?.('change', notify);
			media = null;
		}
	};
}

/**
 * The icon set, folder colour and tone for this window, live. Every icon reads it, so a change in
 * Settings repaints the rows already on screen without remounting the list.
 */
export function useIconLook(): IconLook {
	return useSyncExternalStore(subscribe, getSnapshot, getSnapshot);
}
