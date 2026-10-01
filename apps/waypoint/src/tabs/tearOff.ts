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
} from '../services/tearoffClient';
import type { TabsApi } from '../services/tabsApi';
import { parseRegionId, type DropSlot } from './dropRegions';
import { groupTabs } from './groupLayout';
import { pairName } from './PairPill';
import type { TabDragSource, TearOffHook } from './tabDrag';
import type { TearCardStore } from './tearOffCard';
import { CARD_GRAB, CARD_SIZE, newWindowGeometry } from './tearOffPlacement';
import { locationLabel } from './tabTitle';
import { refuse, windowName } from './windowActions';

/** The least time between two asks of the plugin which region the cursor is over. */
export const HIT_POLL_MS = 80;

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

type Phase = 'idle' | 'starting' | 'following' | 'degraded';

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
 */
export function createTearOff(deps: TearOffDeps): TearOff {
	const now = deps.now ?? (() => Date.now());
	const pollMs = deps.hitPollMs ?? HIT_POLL_MS;
	let phase: Phase = 'idle';
	/** Bumped whenever a drag's plugin state is abandoned, so a call still in flight undoes itself. */
	let generation = 0;
	let source: TabDragSource | null = null;
	let mergeName: string | null = null;
	let windows: WindowSummary[] | null = null;
	let stale = false;
	let polling = false;
	let lastPoll = 0;
	let sent = '';

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
		generation++;
		phase = 'idle';
		source = null;
		mergeName = null;
		windows = null;
		stale = false;
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
				// A frozen cursor says nothing about where the pointer is.
				if (hit && !stale && parseRegionId(hit.region)) {
					windows ??= await deps.api.listWindows().catch(() => []);
					const target = windows.find((window) => window.label === hit.window);
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
				if (phase === 'following') showCard(null);
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

	const merge = async (
		unit: Unit,
		tabs: readonly TabId[],
		hit: NonNullable<DropReport['hit']>,
		slot: DropSlot,
	): Promise<boolean> => {
		const list = await deps.api.listWindows().catch(() => [] as WindowSummary[]);
		const target = list.find((window) => window.label === hit.window);
		// A window that has gone, or this one (a release over its own strip is a reorder, not a merge).
		if (!target || target.active) return false;
		const index = slot.kind === 'slot' ? Math.min(slot.index, target.tabCount) : target.tabCount;
		return move(
			unit,
			tabs,
			{ kind: 'existingWindow', label: target.label, index },
			tf('drag.announce.merged', { name: unit.name, window: windowName(target) }),
		);
	};

	return {
		update(point: Point, dragged: TabDragSource): DragPill {
			source = dragged;
			const features = deps.features();
			const view = deps.viewport();
			const inside = point.x >= 0 && point.y >= 0 && point.x < view.width && point.y < view.height;
			const unit = describeUnit(deps.snapshot(), dragged);
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
			const was = phase;
			reset();
			// The ghost is put away; the pointer came back to the strip, or the drag was cancelled.
			if (was === 'following') void deps.client.end('cancel').catch(() => {});
		},

		async drop(_point: Point, dragged: TabDragSource): Promise<boolean> {
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
			if (hit && slot) return merge(unit, dragged.unit, hit, slot);

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

		connect() {
			const stopTimeout = deps.client.onTimeout(() => {
				// The plugin has put the ghost away already.
				reset();
				deps.announce(t('drag.announce.timedOut'));
				deps.cancelDrag();
			});
			const stopStale = deps.client.onCursorStale((value) => {
				stale = value;
				if (value) {
					mergeName = null;
					pushGhost();
				}
			});
			return () => {
				stopTimeout();
				stopStale();
				const was = phase;
				reset();
				if (was === 'following') void deps.client.end('cancel').catch(() => {});
			};
		},
	};
}
