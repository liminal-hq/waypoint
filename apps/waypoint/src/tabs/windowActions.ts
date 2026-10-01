// New windows and the hand-off of tabs between windows: the commands, their notices and announcements
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import { useEffect, useMemo } from 'react';
import { flushHints } from '../browse/tabHints';
import { t, tf, tn } from '../i18n/messages';
import { WARN_WINDOWS, windowLimitOf, type TabsApi } from '../services/tabsApi';
import { announce } from './announcer';
import { postNotice } from './notices';
import { useHomeLocation, useTabsApi } from './TabsContext';
import { locationLabel } from './tabTitle';

/** What a window can do about other windows. Every method settles; a failure becomes a notice. */
export interface WindowActions {
	/** Ctrl+Shift+N: a new window with one tab at Home. */
	newWindow(): Promise<void>;
	/** Ctrl+middle-click and Open in New Window: a new window with one tab at `location`. */
	openInNewWindow(location: Location): Promise<void>;
	/** Hands `tab` to a window of its own. */
	moveToNewWindow(tab: TabSnapshot): Promise<void>;
	/** Hands `tab` to the end of another window's strip. */
	moveToWindow(tab: TabSnapshot, target: WindowSummary): Promise<void>;
	/** The other windows of the session, for the Move to Window menu. */
	otherWindows(): Promise<WindowSummary[]>;
}

/** The text a menu shows for a window: its folder and how many tabs it holds. */
export function windowMenuLabel(window: WindowSummary): string {
	const title = window.title === '' ? t('tabs.menu.untitledWindow') : window.title;
	return tf('tabs.menu.windowEntry', {
		title,
		tabs: tn('tabs.count', window.tabCount),
	});
}

/** Says why a window could not be opened or a tab moved, in the status bar. */
function refuse(error: unknown, fallback: 'window.notice.openFailed' | 'window.notice.moveFailed') {
	const limit = windowLimitOf(error);
	if (limit !== null) {
		postNotice(tf('window.notice.limit', { limit }));
		return;
	}
	console.warn('window command failed', error);
	postNotice(t(fallback));
}

/**
 * The window commands over `api`. `flush` sends a tab's scroll and focus to the session before the
 * tab moves, so the window it arrives in can restore them; it defaults to the view's own reporter.
 */
export function createWindowActions(
	api: TabsApi,
	home: Location,
	flush: (tab: number) => Promise<void> = flushHints,
): WindowActions {
	// From `WARN_WINDOWS` windows on, say so: each one costs memory.
	const warnIfMany = async () => {
		try {
			if ((await api.listWindows()).length >= WARN_WINDOWS) postNotice(t('window.notice.many'));
		} catch (error) {
			console.debug('could not count the windows', error);
		}
	};
	const open = async (location: Location) => {
		try {
			await api.openWindow(location);
		} catch (error) {
			return refuse(error, 'window.notice.openFailed');
		}
		await warnIfMany();
	};
	return {
		newWindow: () => open(home),
		openInNewWindow: open,
		moveToNewWindow: async (tab) => {
			try {
				await flush(tab.id);
				await api.moveTabs(
					{ kind: 'tabs', value: [tab.id] },
					{ kind: 'newWindow', label: null, geometry: null },
				);
			} catch (error) {
				return refuse(error, 'window.notice.moveFailed');
			}
			announce(t('tabs.announce.movedNewWindow'));
			await warnIfMany();
		},
		moveToWindow: async (tab, target) => {
			try {
				await flush(tab.id);
				await api.moveTabs(
					{ kind: 'tabs', value: [tab.id] },
					{ kind: 'existingWindow', label: target.label, index: target.tabCount },
				);
			} catch (error) {
				return refuse(error, 'window.notice.moveFailed');
			}
			announce(
				tf('tabs.announce.movedToWindow', {
					name: locationLabel(tab.location),
					window: target.title === '' ? t('tabs.menu.untitledWindow') : target.title,
				}),
			);
		},
		otherWindows: async () => {
			try {
				return (await api.listWindows()).filter((window) => !window.active);
			} catch (error) {
				console.warn('could not list the windows', error);
				return [];
			}
		},
	};
}

export function useWindowActions(): WindowActions {
	const api = useTabsApi();
	const home = useHomeLocation();
	return useMemo(() => createWindowActions(api, home), [api, home]);
}

/**
 * The key that opens a new window (Ctrl+Shift+N). Returns whether it took the key.
 */
export function handleWindowKey(
	event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>,
	actions: Pick<WindowActions, 'newWindow'>,
): boolean {
	if (
		(event.ctrlKey || event.metaKey) &&
		event.shiftKey &&
		!event.altKey &&
		event.key.toLowerCase() === 'n'
	) {
		void actions.newWindow();
		return true;
	}
	return false;
}

/** Ctrl+Shift+N, and the announcement of tabs that other windows hand to this one. */
export function useWindowShortcuts(): void {
	const actions = useWindowActions();
	const api = useTabsApi();
	useEffect(() => {
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || event.isComposing) return;
			if (handleWindowKey(event, actions)) event.preventDefault();
		};
		window.addEventListener('keydown', onKeyDown);
		return () => window.removeEventListener('keydown', onKeyDown);
	}, [actions]);
	useEffect(
		() =>
			api.onHandoff(({ tabs }) => {
				void announceArrival(api, tabs);
			}),
		[api],
	);
}

/** Says which tabs just arrived from another window (the live region of this window's strip). */
export async function announceArrival(api: Pick<TabsApi, 'getSnapshot'>, tabs: number[]) {
	try {
		const snapshot = await api.getSnapshot();
		const arrived = snapshot.tabs.filter((tab) => tabs.includes(tab.id));
		const [only] = arrived;
		if (arrived.length === 1 && only) {
			announce(tf('tabs.announce.movedHere', { name: locationLabel(only.location) }));
		} else if (arrived.length > 1) {
			announce(tn('tabs.announce.movedManyHere', arrived.length));
		}
	} catch (error) {
		console.debug('could not announce the arriving tabs', error);
	}
}
