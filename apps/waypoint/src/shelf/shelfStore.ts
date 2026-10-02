// The window's copy of the Shelf and what the panel remembers: revision-gated items, selection, collapsed groups, height and what is known of each file
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ShelfItem } from '@liminal-hq/waypoint-protocol/generated/ShelfItem';
import type { ShelfItemId } from '@liminal-hq/waypoint-protocol/generated/ShelfItemId';
import { createContext, useContext } from 'react';
import { useStore } from 'zustand';
import { createStore, type StoreApi } from 'zustand/vanilla';
import type { ShelfWindow } from '@liminal-hq/waypoint-protocol/generated/ShelfWindow';
import type { ItemState } from './shelfModel';

/** The dock's height in pixels: where it starts, and the least and most it may take. */
export const DEFAULT_HEIGHT = 132;
export const MIN_HEIGHT = 88;
export const MAX_HEIGHT = 360;

export interface ShelfState {
	/** In the order added (oldest first), as Rust keeps them. */
	items: readonly ShelfItem[];
	/** The revision `items` is as of; an older one never replaces a newer. */
	revision: number;
	open: boolean;
	height: number;
	selected: ReadonlySet<ShelfItemId>;
	/** Where a Shift selection starts from. */
	anchor: ShelfItemId | null;
	/** The row the keyboard is on: an `item:` or a `group:` key (`shelfModel.ts`). */
	focus: string | null;
	/** The `uri` of every origin folder whose group is collapsed. */
	collapsed: ReadonlySet<string>;
	/** What was last found out about each item's file, by the item's location `uri`. */
	status: ReadonlyMap<string, ItemState>;
	/** Counts up each time something asks for the panel's focus (the Focus Shelf command). */
	focusRequests: number;
	/** The Shelf is its own window: every main window keeps no dock, and the toggles raise or hide that window. The session's, as of the snapshot. */
	undocked: boolean;
	/** The Shelf window stays above other windows (the session's choice). */
	onTop: boolean;
	/** The Shelf window is on screen; only meaningful while `undocked`. A window starts shown. */
	windowShown: boolean;
}

export interface ShelfActions {
	/** Takes the session's Shelf as of `revision`; an older revision than the one held is ignored. */
	sync(items: readonly ShelfItem[], revision: number): void;
	setOpen(open: boolean): void;
	toggleOpen(): void;
	setHeight(height: number): void;
	/**
	 * A click or a key on an item. `only` selects just it, `toggle` flips it (Ctrl), and `range`
	 * selects from the anchor to it through `order` (Shift).
	 */
	select(id: ShelfItemId, how: 'only' | 'toggle' | 'range', order: readonly ShelfItemId[]): void;
	selectAll(order: readonly ShelfItemId[]): void;
	clearSelection(): void;
	setFocus(key: string | null): void;
	toggleGroup(uri: string, collapsed?: boolean): void;
	/** Records what was found out about files; each entry replaces the one held. */
	setStatus(entries: ReadonlyMap<string, ItemState>): void;
	requestFocus(): void;
	/** Takes where the Shelf lives from the session: docked, or in its own window and whether that stays on top. */
	setWindow(window: Pick<ShelfWindow, 'undocked' | 'onTop'>): void;
	setWindowShown(shown: boolean): void;
}

export type ShelfStore = StoreApi<ShelfState & ShelfActions>;

export function clampHeight(height: number): number {
	return Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, Math.round(height)));
}

/**
 * Memory for the window only: the Shelf's items come from the session (A56), and whether the panel
 * is open, its height and its collapsed groups are this window's own and last as long as it does.
 */
export function createShelfStore(initial: Partial<ShelfState> = {}): ShelfStore {
	return createStore<ShelfState & ShelfActions>()((set, get) => ({
		items: [],
		revision: -1,
		open: false,
		height: DEFAULT_HEIGHT,
		selected: new Set<ShelfItemId>(),
		anchor: null,
		focus: null,
		collapsed: new Set<string>(),
		status: new Map<string, ItemState>(),
		focusRequests: 0,
		undocked: false,
		onTop: false,
		windowShown: true,
		...initial,

		sync: (items, revision) => {
			const state = get();
			if (revision < state.revision) return;
			const ids = new Set(items.map((item) => item.id));
			const uris = new Set(items.map((item) => item.location.uri));
			const selected = new Set([...state.selected].filter((id) => ids.has(id)));
			const status = new Map([...state.status].filter(([uri]) => uris.has(uri)));
			set({
				items,
				revision,
				selected: selected.size === state.selected.size ? state.selected : selected,
				anchor: state.anchor !== null && ids.has(state.anchor) ? state.anchor : null,
				focus: stillThere(state.focus, items) ? state.focus : null,
				status: status.size === state.status.size ? state.status : status,
			});
		},
		setOpen: (open) => set({ open }),
		toggleOpen: () => set((state) => ({ open: !state.open })),
		setHeight: (height) => set({ height: clampHeight(height) }),
		select: (id, how, order) => {
			const state = get();
			if (how === 'only') {
				set({ selected: new Set([id]), anchor: id, focus: `item:${id}` });
			} else if (how === 'toggle') {
				const next = new Set(state.selected);
				if (!next.delete(id)) next.add(id);
				set({ selected: next, anchor: id, focus: `item:${id}` });
			} else {
				const from = order.indexOf(state.anchor ?? id);
				const to = order.indexOf(id);
				if (from < 0 || to < 0) {
					set({ selected: new Set([id]), anchor: id, focus: `item:${id}` });
					return;
				}
				const [lo, hi] = from <= to ? [from, to] : [to, from];
				set({ selected: new Set(order.slice(lo, hi + 1)), focus: `item:${id}` });
			}
		},
		selectAll: (order) => set({ selected: new Set(order) }),
		clearSelection: () => {
			if (get().selected.size > 0) set({ selected: new Set(), anchor: null });
		},
		setFocus: (focus) => set({ focus }),
		toggleGroup: (uri, collapsed) => {
			const next = new Set(get().collapsed);
			const shouldCollapse = collapsed ?? !next.has(uri);
			if (shouldCollapse) next.add(uri);
			else next.delete(uri);
			set({ collapsed: next });
		},
		setStatus: (entries) => {
			const next = new Map(get().status);
			let changed = false;
			for (const [uri, value] of entries) {
				if (next.get(uri) !== value) {
					next.set(uri, value);
					changed = true;
				}
			}
			if (changed) set({ status: next });
		},
		requestFocus: () => set((state) => ({ focusRequests: state.focusRequests + 1 })),
		setWindow: ({ undocked, onTop }) => {
			const state = get();
			if (state.undocked !== undocked || state.onTop !== onTop) set({ undocked, onTop });
		},
		setWindowShown: (windowShown) => {
			if (get().windowShown !== windowShown) set({ windowShown });
		},
	}));
}

/** Whether the row a key names is still on the Shelf (a group is, while any item has its origin). */
function stillThere(key: string | null, items: readonly ShelfItem[]): boolean {
	if (key === null) return true;
	if (key.startsWith('item:')) return items.some((item) => `item:${item.id}` === key);
	return items.some((item) => `group:${item.origin.uri}` === key);
}

export const ShelfStoreContext = createContext<ShelfStore | null>(null);

export function useShelfStore(): ShelfStore {
	const store = useContext(ShelfStoreContext);
	if (!store) throw new Error('useShelfStore must be used inside a ShelfProvider');
	return store;
}

export function useShelfState<T>(select: (state: ShelfState & ShelfActions) => T): T {
	return useStore(useShelfStore(), select);
}
