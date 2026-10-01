// Scroll position and focused entry: reported to the session for a restore or hand-off, applied once when a restored or arriving tab's listing opens
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
	session.view.pendingScroll = hints.scrollTop > 0 ? hints.scrollTop : null;
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

type Flusher = (tab: TabId) => Promise<void>;

/** The reporters running in this window; `flushHints` asks each of them. */
const flushers = new Set<Flusher>();

/**
 * Sends the tab's current scroll offset and focused entry to the session now, whatever the timer
 * is doing, and resolves once the session has them (or could not take them: a failed report is
 * logged and does not stop the caller). A tab about to leave the window calls this first, so the
 * window that receives it can put it back where it was.
 */
export async function flushHints(tab: TabId): Promise<void> {
	await Promise.all([...flushers].map((flush) => flush(tab)));
}

/**
 * Reports the active tab's hints to the session: every `HINT_INTERVAL_MS` while they change, and
 * at once when the window loses focus, is hidden or is going away, and when `onFlush` calls back
 * (the shell asks for a flush when the window is about to close, before the session is saved,
 * which is earlier than `pagehide`). `viewOf` finds any tab's view, which `flushHints` needs for a
 * tab that is not the active one (it defaults to the active view alone). Returns the function
 * that stops it.
 */
export function followHints(
	api: Pick<TabsApi, 'setTabHints'>,
	active: () => ActiveView | null,
	intervalMs: number = HINT_INTERVAL_MS,
	viewOf: (tab: TabId) => ActiveView | null = (tab) => {
		const view = active();
		return view?.tab === tab ? view : null;
	},
	onFlush?: (flush: () => void) => () => void,
): () => void {
	const reported = new Map<TabId, TabHints>();
	const send = (view: ActiveView): Promise<void> => {
		const hints = readHints(view.session, view.mode);
		reported.set(view.tab, hints);
		return api.setTabHints(view.tab, hints).catch((error: unknown) => {
			console.warn('could not save the tab hints', error);
		});
	};
	const report = () => {
		const view = active();
		if (!view) return;
		const last = reported.get(view.tab);
		if (last && sameHints(last, readHints(view.session, view.mode))) return;
		void send(view);
	};
	const flush: Flusher = async (tab) => {
		const view = viewOf(tab);
		if (view) await send(view);
	};
	flushers.add(flush);
	const timer = setInterval(report, intervalMs);
	window.addEventListener('blur', report);
	window.addEventListener('pagehide', report);
	window.addEventListener('beforeunload', report);
	const reportWhenHidden = () => {
		if (document.visibilityState === 'hidden') report();
	};
	document.addEventListener('visibilitychange', reportWhenHidden);
	const unsubscribe = onFlush?.(report);
	return () => {
		flushers.delete(flush);
		clearInterval(timer);
		window.removeEventListener('blur', report);
		window.removeEventListener('pagehide', report);
		window.removeEventListener('beforeunload', report);
		document.removeEventListener('visibilitychange', reportWhenHidden);
		unsubscribe?.();
	};
}
