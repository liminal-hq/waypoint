// The window-level tab keys from docs/interactions.md section 2 that need no groups or pairs
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t, tf } from '../i18n/messages';
import { useEffect, useRef } from 'react';
import { announce } from './announcer';
import { endSwitcher, stepSwitcher, switcherWalk } from './mruSwitcher';
import { useTabsSnapshot } from './TabsContext';
import { useTabActions, type TabActions } from './tabActions';
import { useTabExtras } from './tabExtras';
import { locationLabel } from './tabTitle';

/** What the tab keys do; `useTabShortcuts` supplies the real ones and tests supply spies. */
export interface TabKeyHandlers extends Pick<TabActions, 'newTab' | 'close' | 'goTo'> {
	reopenClosed(): void;
	/** One step of the Ctrl+Tab walk (the switcher commits when Ctrl is released). */
	switchStep(delta: 1 | -1): void;
}

/**
 * Ctrl+T and Ctrl+W open and close, Ctrl+Shift+T reopens the last closed tab, Ctrl+Tab and
 * Ctrl+Shift+Tab walk the tabs in most-recently-used order (committed when Ctrl is released, see
 * `useTabShortcuts`), and Alt+1 to Alt+9 go to that tab.
 */
export function handleTabKey(
	event: Pick<KeyboardEvent, 'key' | 'ctrlKey' | 'metaKey' | 'altKey' | 'shiftKey'>,
	handlers: TabKeyHandlers,
	activeId: number | undefined,
): boolean {
	const key = event.key.toLowerCase();
	const modifier = event.ctrlKey || event.metaKey;
	if (modifier && !event.altKey && key === 't' && event.shiftKey) {
		handlers.reopenClosed();
		return true;
	}
	if (modifier && !event.altKey && !event.shiftKey) {
		if (key === 't') {
			handlers.newTab();
			return true;
		}
		if (key === 'w') {
			if (activeId !== undefined) handlers.close(activeId);
			return true;
		}
	}
	if (event.ctrlKey && !event.altKey && key === 'tab') {
		handlers.switchStep(event.shiftKey ? -1 : 1);
		return true;
	}
	if (event.altKey && !modifier && !event.shiftKey && /^[1-9]$/.test(event.key)) {
		handlers.goTo(Number(event.key));
		return true;
	}
	return false;
}

export function useTabShortcuts(): void {
	const actions = useTabActions();
	const extras = useTabExtras();
	const snapshot = useTabsSnapshot();
	// The listeners below are added once; they read the latest state from here.
	const latest = useRef({ actions, extras, snapshot });
	latest.current = { actions, extras, snapshot };

	useEffect(() => {
		const handlers: TabKeyHandlers = {
			newTab: () => latest.current.actions.newTab(),
			close: (tab) => latest.current.actions.close(tab),
			goTo: (position) => latest.current.actions.goTo(position),
			reopenClosed: () => latest.current.extras.reopen(),
			switchStep: (delta) => {
				const { snapshot: current } = latest.current;
				if (!current) return;
				const walk = stepSwitcher(current, delta);
				const tab =
					walk && current.tabs.find((candidate) => candidate.id === walk.order[walk.index]);
				if (walk && tab) {
					announce(
						tf('tabs.switcher.candidate', {
							title: locationLabel(tab.location),
							position: current.tabs.indexOf(tab) + 1,
							count: current.tabs.length,
						}),
					);
				}
			},
		};
		const onKeyDown = (event: KeyboardEvent) => {
			if (event.defaultPrevented || event.isComposing) return;
			if (handleTabKey(event, handlers, latest.current.snapshot?.active ?? undefined)) {
				event.preventDefault();
			}
		};
		// Escape and Ctrl's release are listened for first (capture), so nothing below can swallow them.
		const onKeyDownFirst = (event: KeyboardEvent) => {
			if (event.key === 'Escape' && switcherWalk()) {
				event.preventDefault();
				event.stopPropagation();
				endSwitcher();
				announce(t('tabs.switcher.cancelled'));
			}
		};
		const commit = () => {
			const target = endSwitcher();
			if (target !== null && target !== latest.current.snapshot?.active) {
				// One activation, so the tabs passed on the way never enter the MRU list.
				latest.current.actions.activate(target);
			}
		};
		const onKeyUp = (event: KeyboardEvent) => {
			if (event.key === 'Control' && switcherWalk()) commit();
		};
		const cancel = () => {
			if (switcherWalk()) endSwitcher();
		};
		window.addEventListener('keydown', onKeyDownFirst, true);
		window.addEventListener('keydown', onKeyDown);
		window.addEventListener('keyup', onKeyUp);
		window.addEventListener('blur', cancel);
		return () => {
			window.removeEventListener('keydown', onKeyDownFirst, true);
			window.removeEventListener('keydown', onKeyDown);
			window.removeEventListener('keyup', onKeyUp);
			window.removeEventListener('blur', cancel);
			cancel();
		};
	}, []);
}
