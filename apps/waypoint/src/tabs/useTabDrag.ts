// Connects the tab strip to the drag engine: starts drags, and says how each slot should look during one
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { GroupId } from '@liminal-hq/waypoint-protocol/generated/GroupId';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import { useEffect, useMemo, useRef, type PointerEvent, type RefObject } from 'react';
import { announce } from './announcer';
import { measureStrip } from './dragLayout';
import { requestRename } from './groupActions';
import {
	createTabDragHandlers,
	describeTabDrag,
	sessionSignature,
	type TabDragPreview,
	type TabDragTarget,
} from './tabDrag';
import { useTabDragSession, useTabDragState, useTearOffHook } from './TabDragContext';
import { useTabsApi, useTabsSnapshot } from './TabsContext';

/** How one tab's slot should be drawn while a drag runs. */
export interface SlotDrag {
	/** Part of what is being dragged. */
	dragging: boolean;
	/** The CSS length for `--wp-tab-shift`, or undefined when the slot is not moved by the drag. */
	shift: string | undefined;
	/** The ring around the tab being held over for a split. */
	ring: 'drawing' | 'armed' | undefined;
	/** Which end of the bridged pill this tab is, once the hold has completed. */
	bridge: 'start' | 'end' | undefined;
}

export interface ChipDrag {
	dragging: boolean;
	shift: string;
}

const IDLE_SLOT: SlotDrag = {
	dragging: false,
	shift: undefined,
	ring: undefined,
	bridge: undefined,
};

function previewOf(target: TabDragTarget | null): TabDragPreview | null {
	return target && 'preview' in target ? target.preview : null;
}

export interface TabDragView {
	/** `active` while the pointer moves it, `settling` while a release or cancel plays out. */
	phase: 'active' | 'settling' | null;
	marker: TabDragPreview['marker'];
	slot(tab: TabId, groupFirst?: TabId): SlotDrag;
	chip(group: GroupId, firstTab: TabId): ChipDrag;
}

export interface TabDrag {
	view: TabDragView;
	/** A press on a tab, or on the joint of a pair, may become a drag. */
	beginTab(event: PointerEvent<HTMLElement>, tab: TabId): void;
	/** A press on a group's chip may become a drag of the whole group. */
	beginGroup(event: PointerEvent<HTMLElement>, group: GroupId): void;
	/** True once after a drag ends: the click that follows it is not a click. */
	consumeClick(): boolean;
}

/**
 * The strip's side of a drag. The engine (`tabDrag.ts`) decides what each move means; this turns
 * the store's state into what each slot and chip should look like, and cancels a drag the session
 * changes under (a tab closed in another window, a group renamed from a menu).
 */
export function useTabDrag(scroller: RefObject<HTMLElement | null>): TabDrag {
	const session = useTabDragSession();
	const tearOff = useTearOffHook();
	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const state = useTabDragState();
	const latest = useRef(snapshot);
	latest.current = snapshot;

	const signature = sessionSignature(snapshot);
	const sourceSignature = state.source?.signature;
	useEffect(() => {
		const phase = session.store.getState().phase;
		if ((phase === 'pending' || phase === 'dragging') && sourceSignature !== signature) {
			session.cancel();
		}
	}, [session, signature, sourceSignature]);

	const begin = (event: PointerEvent<HTMLElement>, press: { tab: TabId } | { group: GroupId }) => {
		const current = latest.current;
		const strip = scroller.current;
		if (!current || !strip) return;
		const source = describeTabDrag(current, measureStrip(strip, current.tabs), press);
		if (!source) return;
		const handlers = createTabDragHandlers({
			api,
			snapshot: () => latest.current,
			announce,
			requestRename,
			tearOff,
		});
		const began = session.begin(
			{
				pointerId: event.pointerId,
				clientX: event.clientX,
				clientY: event.clientY,
				element: event.currentTarget.closest('[data-slot],[data-chip]') ?? event.currentTarget,
				source,
			},
			handlers,
		);
		if (!began) return;
		// Where the pointer is captured it keeps reporting coordinates outside the window, which the
		// engine reads; this is the other signal, for a document that sees the pointer leave.
		const root = document.documentElement;
		const onLeave = () => handlers.leftWindow();
		root.addEventListener('pointerleave', onLeave);
		const stop = session.store.subscribe((state) => {
			if (state.phase === 'pending' || state.phase === 'dragging') return;
			root.removeEventListener('pointerleave', onLeave);
			stop();
		});
	};

	const view = useMemo<TabDragView>(() => {
		const { phase, source, target } = state;
		const none: TabDragView = {
			phase: null,
			marker: null,
			slot: () => IDLE_SLOT,
			chip: () => ({ dragging: false, shift: '0px' }),
		};
		if (!source || (phase !== 'dragging' && phase !== 'dropped' && phase !== 'cancelled')) {
			return none;
		}
		// Once the new order has arrived the slots are where they belong: nothing slides any more.
		if (phase !== 'dragging' && source.signature !== signature) return none;
		const preview = previewOf(target);
		const shifts = new Map(preview?.shifts ?? []);
		const unit = new Set(source.unit);
		const cancelled = phase === 'cancelled';
		// Where the dragged slot rests: with the pointer, bridged to a held tab, or in its slot.
		const dragShift = (): string => {
			if (cancelled) return '0px';
			if (target?.outcome === 'holdSplit' && target.armed) return `${target.attach}px`;
			if (phase === 'dropped') {
				if (preview) return `${preview.slot + source.measure.tablistLeft - source.extent.left}px`;
				return '0px';
			}
			return 'var(--wp-drag-dx, 0px)';
		};
		const shiftOf = (id: TabId) => (cancelled ? 0 : (shifts.get(id) ?? 0));
		return {
			phase: phase === 'dragging' ? 'active' : 'settling',
			marker: phase === 'dragging' ? (preview?.marker ?? null) : null,
			slot: (tab) => {
				if (unit.has(tab)) {
					return {
						dragging: true,
						shift: dragShift(),
						ring: undefined,
						bridge:
							target?.outcome === 'holdSplit' && target.armed && tab === source.lead
								? target.before
									? 'start'
									: 'end'
								: undefined,
					};
				}
				const held = target?.outcome === 'holdSplit' && target.other === tab;
				return {
					dragging: false,
					shift: `${shiftOf(tab)}px`,
					ring: held ? (target.armed ? 'armed' : 'drawing') : undefined,
					bridge: held && target.armed ? (target.before ? 'end' : 'start') : undefined,
				};
			},
			chip: (group, firstTab) =>
				source.kind === 'group' && source.group === group
					? { dragging: true, shift: dragShift() }
					: { dragging: false, shift: `${shiftOf(firstTab)}px` },
		};
	}, [state, signature]);

	return {
		view,
		beginTab: (event, tab) => begin(event, { tab }),
		beginGroup: (event, group) => begin(event, { group }),
		consumeClick: () => session.consumeClick(),
	};
}
