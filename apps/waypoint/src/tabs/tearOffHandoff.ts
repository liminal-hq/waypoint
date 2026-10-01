// What the page that holds the tabs does when the compositor's drag ends: merge into the window they were dropped on, or go back
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import { tf } from '../i18n/messages';
import type { TearoffClient, ToplevelDragEnded } from '../services/tearoffClient';
import type { TabsApi } from '../services/tabsApi';
import { parseRegionId } from './dropRegions';
import { rawSlotOf } from './landing';
import { parseTearPayload, type TearPayload } from './tearOffPayload';
import { refuse, windowName } from './windowActions';

export interface TearHandoffDeps {
	client: TearoffClient;
	api: Pick<TabsApi, 'moveTabs' | 'listWindows'>;
	/** Sends a tab's scroll and focus to the session, so the window it arrives in can restore them. */
	flush(tab: TabId): Promise<void>;
	/** The live region's feed. */
	announce(text: string): void;
}

/**
 * The page that holds the dragged tabs when a compositor-moved drag ends is the one that can move
 * them (a window's session is the only one it can change). That is the new window made for the drag
 * (`mode: 'tabs'`), or the window itself when the tabs were all it had (`mode: 'window'`):
 *
 * - `dropped-on-window`: the tabs go to the slot of that window's strip the pointer was over (`region`), or the end, and this window closes
 *   (it has none left).
 * - `cancelled`, `failed`: the tabs go back to where they came from, when that window still
 *   exists; otherwise they stay here. A window that was the whole drag stays as it is.
 * - `dropped-elsewhere`: the window stays where the compositor left it.
 *
 * The end reaches the page as an event, and is also kept by the plugin for a page that was still
 * loading when it came, so `connect` asks for it once; the plugin numbers each end so it is acted on
 * once. The window that took the payload says so aloud (the page that moves the tabs is closing).
 */
export function createTearHandoff(deps: TearHandoffDeps) {
	const handled = new Set<number>();

	const moveTo = async (payload: TearPayload, target: WindowSummary, index: number) => {
		try {
			for (const id of payload.tabs) await deps.flush(id);
			await deps.api.moveTabs(payload.what, {
				kind: 'existingWindow',
				label: target.label,
				index,
			});
		} catch (error) {
			refuse(error, 'window.notice.moveFailed');
		}
	};

	const resolve = async (ended: ToplevelDragEnded) => {
		if (handled.has(ended.seq)) return;
		handled.add(ended.seq);
		const payload = parseTearPayload(ended.payload);
		if (!payload) return;
		const windows = await deps.api.listWindows().catch(() => null);
		const own = windows?.find((window) => window.active);
		// Both windows of a drag hear its end; only the one that was dragged holds the tabs.
		if (!windows || !own || own.label !== ended.window) return;

		if (ended.outcome === 'dropped-on-window' && ended.target) {
			const target = windows.find((window) => window.label === ended.target);
			if (!target || target.active) return;
			// The slot of the region the pointer let go over (the one the target's line showed), or the end.
			const slot = ended.region ? parseRegionId(ended.region) : null;
			await moveTo(payload, target, slot ? rawSlotOf(slot, target.tabCount) : target.tabCount);
			return;
		}
		if ((ended.outcome === 'cancelled' || ended.outcome === 'failed') && payload.mode === 'tabs') {
			const source = windows.find((window) => window.label === payload.source.window);
			// A source that is gone leaves the tabs where they are: this window is theirs now.
			if (!source || source.active) return;
			await moveTo(payload, source, Math.min(payload.source.index, source.tabCount));
		}
	};

	return {
		resolve,

		/** Listens for the end of a drag that moved this window, and for a payload dropped on it. */
		connect(): () => void {
			let active = true;
			const stopEnded = deps.client.onToplevelEnded((ended) => void resolve(ended));
			const stopDropped = deps.client.onPayloadDropped((dropped) => {
				// The merge is done by the dragged window's page, which is about to close; this window is the one the person is looking at.
				const payload = parseTearPayload(dropped.payload);
				if (!payload) return;
				void deps.api
					.listWindows()
					.then((windows) => {
						const own = windows.find((window) => window.active);
						deps.announce(
							tf('drag.announce.merged', {
								name: payload.name,
								window: own ? windowName(own) : '',
							}),
						);
					})
					.catch(() => {});
			});
			// The end may have come before the page was listening.
			void deps.client
				.takeToplevelResult()
				.then((ended) => {
					if (active && ended) return resolve(ended);
				})
				.catch((error: unknown) => console.warn('could not read the window drag result', error));
			return () => {
				active = false;
				stopEnded();
				stopDropped();
			};
		},
	};
}
