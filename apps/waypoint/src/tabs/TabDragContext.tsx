// Supplies the window's one tab drag session to the strip, the pane area and whatever else draws a drag
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { createContext, useContext, useEffect, useState, type ReactNode } from 'react';
import { useStore } from 'zustand';
import { createDragSession, type DragSession, type DragState } from '../dnd/dragSession';
import { announce } from './announcer';
import { DRAG_START_PX } from './dragTiming';
import type { SeparateSource, SeparateTarget } from './paneDrag';
import type { TabDragSource, TabDragTarget, TearOffHook } from './tabDrag';

export type TabDragSession = DragSession<TabDragSource, TabDragTarget>;
export type TabDragState = DragState<TabDragSource, TabDragTarget>;
export type SeparateSession = DragSession<SeparateSource, SeparateTarget>;

interface TabDragContextValue {
	session: TabDragSession;
	/** The drag of a pane's grip up to the strip, which separates a pair. */
	separate: SeparateSession;
	/** The new-window phase, when something provides one (the tear-off slice). */
	tearOff: TearOffHook | undefined;
}

const TabDragContext = createContext<TabDragContextValue | null>(null);

/** Makes the window's tear-off hook once its drag session exists; `cancel` ends the drag in progress. */
export type TearOffFactory = (control: { cancel(): void }) => TearOffHook;

interface TabDragProviderProps {
	/** Implements the new-window phase of a drag; without one a release outside the strip does nothing. */
	tearOff?: TearOffHook | TearOffFactory;
	children: ReactNode;
}

/** One session per window: only one drag runs at a time, and the pill and zones read the same one. */
export function TabDragProvider({ tearOff, children }: TabDragProviderProps) {
	const [session] = useState<TabDragSession>(() =>
		createDragSession<TabDragSource, TabDragTarget>({
			startThresholdPx: DRAG_START_PX,
			announce,
			sameTarget: (a, b) => JSON.stringify(a) === JSON.stringify(b),
		}),
	);
	const [separate] = useState<SeparateSession>(() =>
		createDragSession<SeparateSource, SeparateTarget>({
			startThresholdPx: DRAG_START_PX,
			announce,
			sameTarget: (a, b) => a.outcome === b.outcome,
		}),
	);
	const [hook] = useState<TearOffHook | undefined>(() =>
		typeof tearOff === 'function'
			? tearOff({
					cancel: () => {
						session.cancel();
						separate.cancel();
					},
				})
			: tearOff,
	);
	useEffect(() => hook?.connect?.(), [hook]);
	useEffect(
		() => () => {
			session.dispose();
			separate.dispose();
		},
		[session, separate],
	);
	return (
		<TabDragContext.Provider value={{ session, separate, tearOff: hook }}>
			{children}
		</TabDragContext.Provider>
	);
}

function useTabDragContext(): TabDragContextValue {
	const value = useContext(TabDragContext);
	if (!value) throw new Error('Tab drag hooks must be used inside a TabDragProvider');
	return value;
}

export function useTabDragSession(): TabDragSession {
	return useTabDragContext().session;
}

export function useTearOffHook(): TearOffHook | undefined {
	return useTabDragContext().tearOff;
}

/** The drag's phase, pill and target; re-renders only when one of them changes. */
export function useTabDragState(): TabDragState {
	return useStore(useTabDragSession().store);
}

export function useSeparateSession(): SeparateSession {
	return useTabDragContext().separate;
}
