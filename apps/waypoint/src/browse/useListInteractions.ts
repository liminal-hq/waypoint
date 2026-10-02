// The keyboard, click and menu behaviour every file view shares, so the list and the grid cannot drift apart
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { useRef, type KeyboardEvent, type MouseEvent, type PointerEvent } from 'react';
import { modifiersOf } from '../dnd/dropAction';
import { useFileDragApi } from '../dnd/FileDragContext';
import { useSettings } from '../settings/SettingsContext';
import { GroupLayout, groupId } from './groupLayout';
import { navigate } from './groupNav';
import { isSelected } from './selection';
import type { ListingSession } from './useListingSession';
import { quickLookStore } from '../quicklook/quickLookStore';
import { findByPrefix, TypeAheadBuffer } from './typeAhead';

export type OpenHandler = (entry: Entry, handle: ListingHandle) => void;

/** Opens an entry beside the current tab, or in a new window when `inNewWindow` (Ctrl held). */
export type OpenInNewHandler = (entry: Entry, handle: ListingHandle, inNewWindow?: boolean) => void;

/** A request for a context menu: on an entry, or (for the host to decide) on empty space. */
export type MenuRequest =
	| {
			kind: 'entry';
			entry: Entry;
			handle: ListingHandle;
			position: { x: number; y: number };
			/** Opened from the keyboard, so focus goes into the menu. */
			keyboard: boolean;
	  }
	| { kind: 'background'; position: { x: number; y: number }; keyboard: boolean };

/** The layout of a listing with no groups, for a view that supplies none. */
const UNGROUPED = new GroupLayout([], new Set(), 0);

/** The pane (tab id) an element sits in, when it sits in one. */
function paneOf(element: Element): number | null {
	const value = element.closest('[data-pane]')?.getAttribute('data-pane');
	return value === null || value === undefined || Number.isNaN(Number(value))
		? null
		: Number(value);
}

export interface InteractionOptions {
	session: ListingSession;
	/** The DOM id of the item at a position, for placing a menu opened from the keyboard. */
	itemId: (position: number) => string;
	/** How many items can be reached (the scroll cap can leave some out). */
	shown: number;
	/**
	 * Where a navigation key goes from position `from` (`null` before anything is focused), or
	 * `null` when the key is not one this view navigates with. The result is clamped by the caller.
	 */
	move: (key: string, from: number | null, last: number) => number | null;
	scrollTo: (position: number) => void;
	/** The rows of the listing: with groups, navigation goes through them (`move` serves a listing without). */
	layout?: GroupLayout;
	/** How many rows a Page key moves. */
	pageRows?: () => number;
	/** Brings a group's header into sight. */
	scrollToHeader?: (group: number) => void;
	onOpen: OpenHandler | undefined;
	onMenu: ((request: MenuRequest) => void) | undefined;
	/** The address of an entry's thumbnail when it has loaded, for the drag's stack. */
	thumbnailOf?: ((entry: Entry) => string | null) | undefined;
}

export interface Interactions {
	onKeyDown: (event: KeyboardEvent<HTMLElement>) => void;
	onItemClick: (event: MouseEvent, position: number, entry: Entry | undefined) => void;
	/** A press on an entry, which becomes a file drag once the pointer has moved. */
	onItemPointerDown: (event: PointerEvent, position: number, entry: Entry | undefined) => void;
	/** A double click on an entry: opens it, unless the settings open with a single click (which already did). */
	onItemDoubleClick: (entry: Entry | undefined) => void;
	onItemContextMenu: (event: MouseEvent, position: number, entry: Entry | undefined) => void;
	onBackgroundContextMenu: (event: MouseEvent) => void;
	/** A click on a group's header: puts the keyboard there and folds the group shut or opens it. */
	onHeaderClick: (group: number) => void;
}

/**
 * Selection by click, Shift, Ctrl and keyboard, type-ahead, Enter to open, and the context menu
 * requests of a listing. A view supplies only how its navigation keys move (`move`) and how to
 * scroll a position into sight.
 */
export function useListInteractions(options: InteractionOptions): Interactions {
	const { session, itemId, shown, move, scrollTo, onOpen, onMenu, thumbnailOf } = options;
	const layout = options.layout ?? UNGROUPED;
	const pageRows = options.pageRows ?? (() => 1);
	const scrollToHeader = options.scrollToHeader ?? (() => {});
	const { model, store } = session;
	const clickMode = useSettings((settings) => settings.general.clickMode);
	const fileDrag = useFileDragApi();
	const typeAhead = useRef(new TypeAheadBuffer());
	const typeAheadEpoch = useRef(0);

	// The menu key asks for the focused entry's menu, placed at its item, or the empty-space menu
	// when nothing is focused. With Ctrl held it always asks for the empty-space menu, the way to
	// it in a folder whose rows fill the window and leave no empty space to click.
	const openMenuFromKeyboard = (event: KeyboardEvent<HTMLElement>) => {
		event.preventDefault();
		const at = store.getState().focus;
		const entry = at === null || event.ctrlKey ? undefined : model.entryAt(at);
		const item = at === null ? null : document.getElementById(itemId(at));
		const rect = (item ?? event.currentTarget).getBoundingClientRect();
		const position = { x: rect.left + 24, y: rect.bottom };
		if (entry) onMenu?.({ kind: 'entry', entry, handle: model.handle, position, keyboard: true });
		else onMenu?.({ kind: 'background', position, keyboard: true });
	};

	const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
		if (event.nativeEvent.isComposing) return;
		// A key typed in a field inside the list (the rename field) is the field's, not the list's.
		if (event.target !== event.currentTarget) return;
		const state = store.getState();
		const modifier = event.ctrlKey || event.metaKey;
		const from = state.focus;
		// Rows past the scroll cap are never drawn, so the keyboard cannot reach them; Ctrl+A is a
		// whole-listing action and still takes every entry, as the capped banner says.
		const last = shown - 1;

		if (layout.grouped && state.focusHeader !== null) {
			const group = layout.groups.findIndex((run) => groupId(run.key) === state.focusHeader);
			const open = group >= 0 && !layout.isCollapsed(group);
			// On a header, Left folds the group shut and Right opens it; Enter and Space do either.
			const fold: boolean | null =
				event.altKey || modifier
					? null
					: event.key === 'ArrowLeft'
						? true
						: event.key === 'ArrowRight'
							? false
							: event.key === 'Enter' || event.key === ' '
								? open
								: null;
			if (fold !== null) {
				event.preventDefault();
				typeAheadEpoch.current++;
				if (group >= 0) state.setGroupCollapsed(state.focusHeader, fold);
				return;
			}
		}

		// Alt + arrow is history and up-a-folder, which belong to the window, not to the view.
		if (layout.grouped) {
			const cursor =
				state.focusHeader !== null
					? { header: layout.groups.findIndex((run) => groupId(run.key) === state.focusHeader) }
					: from === null
						? null
						: { position: from };
			const stop = event.altKey ? null : navigate(layout, event.key, cursor, pageRows());
			if (stop !== null) {
				event.preventDefault();
				typeAheadEpoch.current++;
				if ('header' in stop) {
					state.focusGroup(groupId(layout.groups[stop.header]!.key));
					scrollToHeader(stop.header);
				} else {
					if (event.shiftKey) void state.extendTo(stop.position, modifier);
					else state.moveTo(stop.position, !modifier);
					scrollTo(stop.position);
				}
				return;
			}
		}
		const target = event.altKey || layout.grouped ? null : move(event.key, from, last);
		if (target !== null) {
			event.preventDefault();
			typeAheadEpoch.current++;
			const clamped = Math.max(0, Math.min(last, target));
			if (event.shiftKey) void state.extendTo(clamped, modifier);
			else state.moveTo(clamped, !modifier);
			scrollTo(clamped);
			return;
		}

		switch (event.key) {
			case 'Enter': {
				// Alt+Enter is Properties in a window, which belongs to the window, not to the view.
				if (event.altKey) return;
				const entry = from === null ? undefined : model.entryAt(from);
				if (entry && onOpen) {
					event.preventDefault();
					onOpen(entry, model.handle);
				}
				return;
			}
			case 'Escape':
				event.preventDefault();
				typeAheadEpoch.current++;
				return state.deselectAll();
			case 'ContextMenu':
				return openMenuFromKeyboard(event);
			case ' ':
				if (modifier) {
					event.preventDefault();
					typeAheadEpoch.current++;
					state.toggleFocused();
					return;
				}
				// Mid-prefix, a space is part of the name being typed; otherwise it opens Quick Look on
				// the focused entry (when the window has one), and must not scroll the view.
				if (!typeAhead.current.active) {
					event.preventDefault();
					// The Trash's items are not on disk to preview, and a held key opens it once.
					if (from !== null && !event.repeat && model.layout !== 'trash') {
						quickLookStore.getState().open({ session, move, onOpen });
					}
					return;
				}
				break;
		}

		if (event.key === 'F10' && event.shiftKey) return openMenuFromKeyboard(event);

		if (modifier && !event.shiftKey && !event.altKey) {
			const key = event.key.toLowerCase();
			typeAheadEpoch.current++;
			if (key === 'a') {
				event.preventDefault();
				state.selectAll();
			} else if (key === 'i') {
				event.preventDefault();
				state.invertSelection();
			}
			return;
		}

		if (event.key.length === 1 && !modifier && !event.altKey) {
			event.preventDefault();
			const prefix = typeAhead.current.push(event.key);
			const epoch = ++typeAheadEpoch.current;
			// A fresh first letter looks past the focused entry; a growing prefix may keep it.
			const start = from === null ? 0 : prefix.length === 1 ? from + 1 : from;
			void findByPrefix(model, prefix, start, () => epoch !== typeAheadEpoch.current).then(
				(position) => {
					// Entries of a folded group are not there to be found.
					if (position === null || layout.rowOfEntry(position) === null) return;
					store.getState().moveTo(position, true);
					scrollTo(position);
				},
			);
		}
	};

	const onItemPointerDown = (event: PointerEvent, position: number, entry: Entry | undefined) => {
		if (!fileDrag || !entry || event.pointerType === 'touch') return;
		if (event.button !== 0 && event.button !== 2) return;
		// A press in the rename field is the field's.
		if (event.target instanceof Element && event.target.closest('input, textarea')) return;
		const row = event.currentTarget as Element;
		fileDrag.press({
			pointerId: event.pointerId,
			clientX: event.clientX,
			clientY: event.clientY,
			button: event.button,
			// The list outlives its rows, which the virtualiser recycles as it scrolls.
			element: row.closest('[role="listbox"]') ?? row,
			session,
			position,
			entry,
			thumbnail: thumbnailOf?.(entry) ?? null,
			tab: paneOf(row),
			modifiers: modifiersOf(event),
		});
	};

	const onItemClick = (event: MouseEvent, position: number, entry: Entry | undefined) => {
		// The click that ends a drag is not a click on the row it ended over.
		if (fileDrag?.consumeClick()) return;
		const state = store.getState();
		const modifier = event.ctrlKey || event.metaKey;
		typeAheadEpoch.current++;
		if (event.shiftKey) void state.extendTo(position, modifier);
		else if (!entry) return;
		else if (modifier) state.toggleAt(position, entry.id);
		else {
			state.click(position, entry.id);
			// A plain click opens when the settings say so, except in a field inside the row (a
			// rename in progress), which a click only places the caret in.
			const inField = event.target instanceof Element && event.target.closest('input, textarea');
			if (clickMode === 'single' && !inField) onOpen?.(entry, model.handle);
		}
	};

	const onItemDoubleClick = (entry: Entry | undefined) => {
		if (clickMode === 'double' && entry) onOpen?.(entry, model.handle);
	};

	// Right-click selects the entry first unless it is already part of the selection, as file
	// managers do, so the menu never acts on something other than what is highlighted.
	const onItemContextMenu = (event: MouseEvent, position: number, entry: Entry | undefined) => {
		event.preventDefault();
		event.stopPropagation();
		if (!entry) return;
		const state = store.getState();
		if (!isSelected(state.selection, entry.id)) state.click(position, entry.id);
		else state.moveTo(position, false);
		const open = () =>
			onMenu?.({
				kind: 'entry',
				entry,
				handle: model.handle,
				position: { x: event.clientX, y: event.clientY },
				keyboard: false,
			});
		// Where the menu opens on the press, a right-button drag must be able to start first: the
		// menu waits for the release, and a drag drops it.
		if (!fileDrag?.deferMenu(open)) open();
	};

	const onBackgroundContextMenu = (event: MouseEvent) => {
		event.preventDefault();
		onMenu?.({
			kind: 'background',
			position: { x: event.clientX, y: event.clientY },
			keyboard: false,
		});
	};

	const onHeaderClick = (group: number) => {
		const run = layout.groups[group];
		if (!run) return;
		const state = store.getState();
		typeAheadEpoch.current++;
		const id = groupId(run.key);
		state.focusGroup(id);
		state.setGroupCollapsed(id, !layout.isCollapsed(group));
	};

	return {
		onHeaderClick,
		onKeyDown,
		onItemClick,
		onItemPointerDown,
		onItemDoubleClick,
		onItemContextMenu,
		onBackgroundContextMenu,
	};
}
