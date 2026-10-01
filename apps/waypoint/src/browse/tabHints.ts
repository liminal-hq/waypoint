// Scroll position and focused entry: reported to the session for a restore, applied once when a restored tab's listing opens
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabHints } from '@liminal-hq/waypoint-protocol/generated/TabHints';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { TabsApi } from '../services/tabsApi';
import type { ListingSession } from './useListingSession';
import type { ViewMode } from './viewStore';

/** The most often the active tab's hints go to the session while it is being used. */
export const HINT_INTERVAL_MS = 2000;

/** How many entries a restore looks through for the focused entry's name. */
export const FOCUS_SEARCH_LIMIT = 2048;

const SEARCH_PAGE = 256;

/** What the tab's view is showing now: the offset of the layout in use and the focused entry's name. */
export function readHints(session: ListingSession, mode: ViewMode): TabHints {
	const scrollTop = mode === 'grid' ? session.view.gridScrollTop : session.view.scrollTop;
	const focus = session.store.getState().focus;
	const focused = focus === null ? null : (session.model.entryAt(focus)?.name ?? null);
	return { scrollTop: Math.max(0, Math.round(scrollTop)), focused };
}

/** Whether two sets of hints say the same thing. */
export function sameHints(a: TabHints, b: TabHints): boolean {
	return a.scrollTop === b.scrollTop && a.focused === b.focused;
}

/**
 * Puts a restored tab's scroll offset where its view will read it when it mounts (the view applies
 * `session.view` itself), and focuses the entry with the saved name when it is among the first
 * `FOCUS_SEARCH_LIMIT` entries of the listing. Called once per tab, when its first listing opens.
 */
export function applyHints(session: ListingSession, hints: TabHints, mode: ViewMode): void {
	if (mode === 'grid') session.view.gridScrollTop = hints.scrollTop;
	else session.view.scrollTop = hints.scrollTop;
	if (hints.focused !== null) void focusByName(session, hints.focused);
}

async function focusByName(session: ListingSession, name: string): Promise<void> {
	const { model, store } = session;
	const end = Math.min(model.count, FOCUS_SEARCH_LIMIT);
	try {
		for (let from = 0; from < end; from += SEARCH_PAGE) {
			const page = await model.readRange(from, Math.min(end, from + SEARCH_PAGE));
			const at = page.findIndex((entry) => entry.name === name);
			// Something the person did meanwhile wins over a restore.
			if (store.getState().touched) return;
			if (at >= 0) {
				store.getState().moveTo(from + at, false);
				return;
			}
		}
	} catch (error) {
		// The listing closed under the search; there is nothing left to focus.
		console.debug('could not restore the focused entry', error);
	}
}

export interface ActiveView {
	tab: TabId;
	session: ListingSession;
	mode: ViewMode;
}

/**
 * Reports the active tab's hints to the session: every `HINT_INTERVAL_MS` while they change, and
 * at once when the window loses focus or is going away. Returns the function that stops it.
 */
export function followHints(
	api: Pick<TabsApi, 'setTabHints'>,
	active: () => ActiveView | null,
	intervalMs: number = HINT_INTERVAL_MS,
): () => void {
	const reported = new Map<TabId, TabHints>();
	const report = () => {
		const view = active();
		if (!view) return;
		const hints = readHints(view.session, view.mode);
		const last = reported.get(view.tab);
		if (last && sameHints(last, hints)) return;
		reported.set(view.tab, hints);
		api.setTabHints(view.tab, hints).catch((error: unknown) => {
			console.warn('could not save the tab hints', error);
		});
	};
	const timer = setInterval(report, intervalMs);
	window.addEventListener('blur', report);
	window.addEventListener('pagehide', report);
	return () => {
		clearInterval(timer);
		window.removeEventListener('blur', report);
		window.removeEventListener('pagehide', report);
	};
}
