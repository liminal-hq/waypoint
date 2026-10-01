// The new-window phase of a tab drag: the in-page card, the plugin's ghost, and what a release does
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MoveWhat } from '@liminal-hq/waypoint-protocol/generated/MoveWhat';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import type { DragPill, Point } from '../dnd/dragSession';
import { t, tf } from '../i18n/messages';
import type {
	DropReport,
	GhostPayload,
	Size,
	TearoffClient,
	TearoffFeatures,
	ToplevelDragEnded,
} from '../services/tearoffClient';
import type { TabsApi } from '../services/tabsApi';
import { parseRegionId, type DropSlot } from './dropRegions';
import { groupTabs } from './groupLayout';
import { pairName } from './PairPill';
import type { TabDragSource, TearOffHook } from './tabDrag';
import { postNotice } from './notices';
import { parseTearPayload, type TearPayload } from './tearOffPayload';
import type { TearCardStore } from './tearOffCardModel';
import { CARD_GRAB, CARD_SIZE, newWindowGeometry } from './tearOffPlacement';
import { locationLabel } from './tabTitle';
import { refuse, windowName } from './windowActions';

/** The least time between two asks of the plugin which region the cursor is over. */
export const HIT_POLL_MS = 80;

/** How long the window list read for the merge label is trusted: windows open and close mid-drag. */
export const WINDOWS_TTL_MS = 1000;

export interface TearOffDeps {
	client: TearoffClient;
	/** What the plugin reported at start-up; every feature off until it has. */
	features(): TearoffFeatures;
	api: Pick<TabsApi, 'moveTabs' | 'listWindows'>;
	snapshot(): SessionSnapshot | null;
	/** Sends a tab's scroll and focus to the session, so the window it arrives in can restore them. */
	flush(tab: TabId): Promise<void>;
	/** The live region's feed. */
	announce(text: string): void;
	/** Ends the drag in progress (the engine cancels it and says so). */
	cancelDrag(): void;
	card: TearCardStore;
	/** This window's content size in logical pixels: where the pointer is still over the page. */
	viewport(): Size;
	/** The transparent margin the frame draws around the window, in logical pixels. */
	frameMargin(): number;
	hitPollMs?: number;
	now?(): number;
}

/** The hook the engine calls, with the one thing the provider connects once the window is live. */
export type TearOff = Required<TearOffHook>;

/** What is dragged, as the card, the announcements and `moveTabs` name it. */
interface Unit {
	title: string;
	count: number | undefined;
	name: string;
	what: MoveWhat;
}

function describeUnit(snapshot: SessionSnapshot | null, source: TabDragSource): Unit {
	const label = (id: TabId) => {
		const tab = snapshot?.tabs.find((candidate) => candidate.id === id);
		return tab ? locationLabel(tab.location) : '';
	};
	const count = source.unit.length > 1 ? source.unit.length : undefined;
	const title = label(source.unit[0] ?? source.lead);
	if (source.kind === 'group' && source.group !== null) {
		const group = snapshot?.groups.find((candidate) => candidate.id === source.group);
		const members = snapshot ? groupTabs(snapshot.tabs, source.group).map((tab) => tab.id) : [];
		return {
			title,
			count: count ?? members.length,
			name: group?.name ?? title,
			what: { kind: 'group', value: source.group },
		};
	}
	const pair =
		source.unit.length > 1
			? snapshot?.pairs.find((candidate) => candidate.panes.includes(source.lead))
			: undefined;
	if (pair) {
		return {
			title,
			count,
			name: pairName(pair, snapshot),
			what: { kind: 'pair', value: pair.id },
		};
	}
	return {
		title,
		count,
		name: label(source.lead),
		what: { kind: 'tabs', value: [...source.unit] },
	};
}

function lowerFirst(text: string): string {
	return text.charAt(0).toLowerCase() + text.slice(1);
}

function pill(kind: string, text: string): DragPill {
	return { kind, text, announce: tf('drag.announce.pill', { text: lowerFirst(text) }) };
}

type Phase =
	| 'idle'
	| 'starting'
	| 'following'
	| 'degraded'
	// The compositor-moved window: getting the tabs into a window and the drag started, then the drag itself.
	| 'tearing'
	| 'toplevel'
	// The compositor-moved window could not start; nothing more is tried until the pointer is back in the strip.
	| 'blocked';

/**
 * The tear-off hook for one window. While the pointer is over the window it shows the in-page
 * card. Where the plugin reports a ghost that follows the cursor, the ghost takes over when the
 * pointer leaves the window, and says "Release to merge into …" while the cursor is over another
 * window's strip; where it does not (Wayland, D94) the card and the pill are all there is, and a
 * release outside the strip opens a window the compositor places.
 *
 * A release asks the plugin where the cursor was and which drop region it was over. A region of
 * another window merges the tabs into that window, at the slot the region names; no region opens
 * a new window under the cursor; a cursor the plugin cannot vouch for (stale, or none) opens one
 * without geometry, which cascades from this window. Every move flushes the tabs' hints first.
 *
 * Where the plugin reports `toplevelDrag` (Wayland with `xdg-toplevel-drag`, D94's replacement) none
 * of that applies: as soon as the pointer is out of the strip the tabs move to a new window made
 * hidden (or, when they are all the window has, the window itself is taken), the compositor
 * attaches that real window to the pointer, and this page hears of the end of the drag (the page
 * gets no pointer events meanwhile, so the drag engine is stopped at once and reset again at the
 * end). What the drag ends as is acted on by the page that holds the tabs (`tearOffHandoff.ts`).
 *
 * The plugin's `cursor-stale` event is not acted on: a pointer held still for a second looks the
 * same as a frozen cursor, and the merge label and a merge by region should survive a pause. The
 * report's `cursorStale` is what keeps a window from being placed at a position that may be old.
 */
export function createTearOff(deps: TearOffDeps): TearOff {
	const now = deps.now ?? (() => Date.now());
	const pollMs = deps.hitPollMs ?? HIT_POLL_MS;
	let phase: Phase = 'idle';
	/** Bumped whenever a drag's plugin state is abandoned, so a call still in flight undoes itself. */
	let generation = 0;
	let source: TabDragSource | null = null;
	let mergeName: string | null = null;
	let windows: { list: WindowSummary[]; at: number } | null = null;
	let polling = false;
	let lastPoll = 0;
	let sent = '';
	/** Asks which region the cursor is over while the ghost follows: the page gets no pointer events outside its window. */
	let watching: ReturnType<typeof setInterval> | undefined;
	const stopWatching = () => {
		if (watching !== undefined) clearInterval(watching);
		watching = undefined;
	};

	const payload = (unit: Unit, withLabel: boolean): GhostPayload => ({
		title: unit.title,
		...(unit.count === undefined ? {} : { count: unit.count }),
		...(withLabel
			? {
					label: mergeName
						? tf('drag.pill.mergeInto', { name: mergeName })
						: t('drag.pill.newWindow'),
				}
			: {}),
	});

	const showCard = (next: GhostPayload | null) => {
		const current = deps.card.getState().payload;
		if (current === next) return;
		if (
			current &&
			next &&
			current.title === next.title &&
			current.count === next.count &&
			current.label === next.label
		) {
			return;
		}
		deps.card.setState({ payload: next });
	};

	const reset = () => {
		stopWatching();
		generation++;
		phase = 'idle';
		source = null;
		mergeName = null;
		windows = null;
		sent = '';
		showCard(null);
	};

	/** Gives the ghost the current label, when it has changed. */
	const pushGhost = () => {
		if (phase !== 'following' || !source) return;
		const next = payload(describeUnit(deps.snapshot(), source), true);
		const key = JSON.stringify(next);
		if (key === sent) return;
		sent = key;
		deps.client.update(next).catch((error: unknown) => console.warn('ghost update failed', error));
	};

	const pollHit = () => {
		if (phase !== 'following' || polling || !deps.features().hitTest) return;
		const at = now();
		if (at - lastPoll < pollMs) return;
		lastPoll = at;
		polling = true;
		const mine = generation;
		void (async () => {
			try {
				const hit = await deps.client.hitTest();
				let name: string | null = null;
				if (hit && parseRegionId(hit.region)) {
					// A failed read is not remembered, and a good one only for a moment.
					if (!windows || now() - windows.at > WINDOWS_TTL_MS) {
						const list = await deps.api.listWindows().catch(() => null);
						if (list) windows = { list, at: now() };
					}
					const target = windows?.list.find((window) => window.label === hit.window);
					if (target && !target.active) name = windowName(target);
				}
				if (mine !== generation) return;
				mergeName = name;
				pushGhost();
			} catch (error) {
				console.debug('could not hit-test the drop regions', error);
			} finally {
				polling = false;
			}
		})();
	};

	const startFollowing = (unit: Unit) => {
		phase = 'starting';
		const mine = generation;
		const first = payload(unit, true);
		sent = JSON.stringify(first);
		deps.client.begin(first, CARD_GRAB, CARD_SIZE).then(
			(result) => {
				if (mine !== generation) {
					// The drag moved on while the ghost was starting: put it away again.
					if (result === 'following') void deps.client.end('cancel').catch(() => {});
					return;
				}
				phase = result === 'following' ? 'following' : 'degraded';
				if (phase === 'following') {
					showCard(null);
					watching = setInterval(pollHit, pollMs);
				}
			},
			(error: unknown) => {
				console.warn('could not start the tear-off ghost', error);
				if (mine === generation) phase = 'degraded';
			},
		);
	};

	/** Where a release put the tabs, and what to say. */
	const move = async (
		unit: Unit,
		tabs: readonly TabId[],
		to: Parameters<TabsApi['moveTabs']>[1],
		say: string,
	): Promise<boolean> => {
		try {
			for (const id of tabs) await deps.flush(id);
			await deps.api.moveTabs(unit.what, to);
		} catch (error) {
			refuse(error, 'window.notice.moveFailed');
			return false;
		}
		deps.announce(say);
		return true;
	};

	/** Merges into the window the region belongs to; null when that window has gone, so the caller can fall back. */
	const merge = async (
		unit: Unit,
		tabs: readonly TabId[],
		hit: NonNullable<DropReport['hit']>,
		slot: DropSlot,
	): Promise<boolean | null> => {
		const list = await deps.api.listWindows().catch(() => [] as WindowSummary[]);
		const target = list.find((window) => window.label === hit.window);
		if (!target) return null;
		// This window (a release over its own strip is a reorder, not a merge): nothing moves, and it says so.
		if (target.active) {
			deps.announce(t('drag.announce.cancelled'));
			return false;
		}
		const index = slot.kind === 'slot' ? Math.min(slot.index, target.tabCount) : target.tabCount;
		return move(
			unit,
			tabs,
			{ kind: 'existingWindow', label: target.label, index },
			tf('drag.announce.merged', { name: unit.name, window: windowName(target) }),
		);
	};

	/** The compositor has the drag: stop the engine, which will get no more pointer events. */
	const enterToplevel = () => {
		if (phase !== 'tearing') return;
		phase = 'toplevel';
		deps.cancelDrag();
	};

	/** Moves the tabs to a window made hidden, or takes this window when they are all it has, and starts the compositor's drag of it. */
	const startToplevel = (unit: Unit, dragged: TabDragSource, point: Point) => {
		phase = 'tearing';
		const mine = generation;
		void (async () => {
			let target: string | null = null;
			/** The window made hidden for this drag, which nothing shows unless the drag starts. */
			let hidden: string | null = null;
			try {
				const windows = await deps.api.listWindows();
				const own = windows.find((window) => window.active)?.label;
				if (!own) throw new Error('this window is not in the session');
				const snapshot = deps.snapshot();
				const whole = !!snapshot && snapshot.tabs.every((tab) => dragged.unit.includes(tab.id));
				const index = snapshot
					? Math.max(
							0,
							snapshot.tabs.findIndex((tab) => tab.id === dragged.unit[0]),
						)
					: 0;
				const payload: TearPayload = {
					v: 1,
					mode: whole ? 'window' : 'tabs',
					what: unit.what,
					tabs: [...dragged.unit],
					name: unit.name,
					source: { window: own, index },
				};
				target = own;
				if (!whole) {
					for (const id of dragged.unit) await deps.flush(id);
					// The new window must not be mapped before the drag has attached it.
					await deps.client.holdNextWindow(true);
					try {
						target = await deps.api.moveTabs(unit.what, {
							kind: 'newWindow',
							label: null,
							geometry: null,
						});
					} finally {
						await deps.client.holdNextWindow(false).catch(() => {});
					}
					hidden = target;
				}
				// The pointer holds the window where it holds this one: the new window opens over the old.
				const view = deps.viewport();
				const grab = {
					x: Math.min(Math.max(point.x, 0), view.width),
					y: Math.min(Math.max(point.y, 0), view.height),
				};
				const report = await deps.client.beginToplevelDrag(payload, target, grab);
				if (report.state === 'started') enterToplevel();
				else if (phase === 'tearing' && mine === generation) phase = 'blocked';
			} catch (error) {
				console.warn('could not start the window drag', error);
				// The plugin tells the page of the window it was to drag when a drag it answered does not
				// start, and that page puts the tabs back. A call that failed outright never reached it, so
				// the tabs would stay in a window nothing shows: show it, where they can be used.
				if (hidden) {
					await deps.client.showWindow(hidden).catch(() => {});
				}
				if (phase === 'tearing' && mine === generation) {
					phase = 'blocked';
					refuse(error, 'window.notice.moveFailed');
				}
			}
		})();
	};

	/** What a toplevel drag's end means to the page that began it. The page holding the tabs acts on it (`tearOffHandoff.ts`); this one stops the engine and says what happened. */
	const onToplevelEnded = (ended: ToplevelDragEnded) => {
		if (phase === 'idle' || (phase !== 'tearing' && phase !== 'toplevel' && phase !== 'blocked'))
			return;
		reset();
		// The drag's own pointer events never reached the page, so nothing else has told the engine it is over.
		deps.cancelDrag();
		const name = parseTearPayload(ended.payload)?.name ?? '';
		const moved = parseTearPayload(ended.payload)?.mode === 'tabs';
		switch (ended.outcome) {
			case 'dropped-elsewhere':
				if (moved) deps.announce(tf('drag.announce.movedNewWindow', { name }));
				break;
			case 'cancelled':
				deps.announce(t('drag.announce.cancelled'));
				break;
			case 'failed':
				postNotice(t('window.notice.moveFailed'));
				deps.announce(t('window.notice.moveFailed'));
				break;
			case 'dropped-on-window':
				// The window that took it says so.
				break;
		}
	};

	return {
		update(point: Point, dragged: TabDragSource): DragPill {
			source = dragged;
			const features = deps.features();
			const view = deps.viewport();
			const inside = point.x >= 0 && point.y >= 0 && point.x < view.width && point.y < view.height;
			const unit = describeUnit(deps.snapshot(), dragged);
			if (features.toplevelDrag) {
				// The real window is the preview: no card here, no ghost.
				showCard(null);
				if (phase === 'idle') startToplevel(unit, dragged, point);
				return pill('window', t('drag.pill.newWindow'));
			}
			if (features.ghost && features.cursorFollow && phase === 'idle' && !inside) {
				startFollowing(unit);
			}
			if (phase === 'following') {
				showCard(null);
				pollHit();
				pushGhost();
			} else {
				showCard(inside ? payload(unit, false) : null);
			}
			return mergeName && phase === 'following'
				? pill('merge', tf('drag.pill.mergeInto', { name: mergeName }))
				: pill('window', t('drag.pill.newWindow'));
		},

		leave() {
			// The drag has been handed to the compositor (or is being); the pointer coming back to the strip changes nothing.
			if (phase === 'tearing' || phase === 'toplevel') return;
			const was = phase;
			reset();
			// The ghost is put away; the pointer came back to the strip, or the drag was cancelled.
			if (was === 'following') void deps.client.end('cancel').catch(() => {});
		},

		async drop(_point: Point, dragged: TabDragSource): Promise<boolean> {
			// Released before the compositor took the drag: it carries on, and if the button was already up it ends as failed and the tabs go back.
			if (phase === 'tearing' || phase === 'toplevel') return true;
			if (phase === 'blocked') {
				reset();
				return false;
			}
			const features = deps.features();
			const was = phase;
			const unit = describeUnit(deps.snapshot(), dragged);
			reset();
			let report: DropReport | null = null;
			// Where the cursor was is known wherever it is live, even for a drag that never left this window.
			if (features.cursorFollow || was !== 'idle') {
				try {
					report = await deps.client.end('drop');
				} catch (error) {
					console.warn('could not read where the drag ended', error);
				}
			}
			const hit = report?.hit ?? null;
			const slot = hit ? parseRegionId(hit.region) : null;
			if (hit && slot) {
				const merged = await merge(unit, dragged.unit, hit, slot);
				// The window under the cursor has gone since the regions were registered: the tabs
				// open a window of their own rather than the drag being lost.
				if (merged !== null) return merged;
			}

			const inner = deps.viewport();
			const geometry =
				report?.cursor && !report.cursorStale && features.windowPosition
					? newWindowGeometry({
							cursor: report.cursor,
							scale: report.scaleFactor,
							grab: CARD_GRAB,
							margin: deps.frameMargin(),
							inner,
						})
					: null;
			return move(
				unit,
				dragged.unit,
				{ kind: 'newWindow', label: null, geometry },
				tf('drag.announce.movedNewWindow', { name: unit.name }),
			);
		},

		handsOff: () => phase === 'tearing' || phase === 'toplevel',

		connect() {
			const stopStarted = deps.client.onToplevelStarted(() => enterToplevel());
			const stopEnded = deps.client.onToplevelEnded(onToplevelEnded);
			const stopTimeout = deps.client.onTimeout(() => {
				// The plugin has put the ghost away already.
				reset();
				deps.announce(t('drag.announce.timedOut'));
				deps.cancelDrag();
			});
			return () => {
				stopStarted();
				stopEnded();
				stopTimeout();
				const was = phase;
				reset();
				if (was === 'following') void deps.client.end('cancel').catch(() => {});
			};
		},
	};
}
