// The keyboard, click and menu behaviour every file view shares, so the list and the grid cannot drift apart
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { useRef, type KeyboardEvent, type MouseEvent } from 'react';
import { isSelected } from './selection';
import type { ListingSession } from './useListingSession';
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
	onOpen: OpenHandler | undefined;
	onMenu: ((request: MenuRequest) => void) | undefined;
}

export interface Interactions {
	onKeyDown: (event: KeyboardEvent<HTMLElement>) => void;
	onItemClick: (event: MouseEvent, position: number, entry: Entry | undefined) => void;
	onItemContextMenu: (event: MouseEvent, position: number, entry: Entry | undefined) => void;
	onBackgroundContextMenu: (event: MouseEvent) => void;
}

/**
 * Selection by click, Shift, Ctrl and keyboard, type-ahead, Enter to open, and the context menu
 * requests of a listing. A view supplies only how its navigation keys move (`move`) and how to
 * scroll a position into sight.
 */
export function useListInteractions(options: InteractionOptions): Interactions {
	const { session, itemId, shown, move, scrollTo, onOpen, onMenu } = options;
	const { model, store } = session;
	const typeAhead = useRef(new TypeAheadBuffer());
	const typeAheadEpoch = useRef(0);

	// The menu key asks for the focused entry's menu, placed at its item, or the empty-space menu
	// when nothing is focused.
	const openMenuFromKeyboard = (event: KeyboardEvent<HTMLElement>) => {
		event.preventDefault();
		const at = store.getState().focus;
		const entry = at === null ? undefined : model.entryAt(at);
		const item = at === null ? null : document.getElementById(itemId(at));
		const rect = (item ?? event.currentTarget).getBoundingClientRect();
		const position = { x: rect.left + 24, y: rect.bottom };
		if (entry) onMenu?.({ kind: 'entry', entry, handle: model.handle, position, keyboard: true });
		else onMenu?.({ kind: 'background', position, keyboard: true });
	};

	const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
		if (event.nativeEvent.isComposing) return;
		const state = store.getState();
		const modifier = event.ctrlKey || event.metaKey;
		const from = state.focus;
		// Rows past the scroll cap are never drawn, so the keyboard cannot reach them; Ctrl+A is a
		// whole-listing action and still takes every entry, as the capped banner says.
		const last = shown - 1;

		// Alt + arrow is history and up-a-folder, which belong to the window, not to the view.
		const target = event.altKey ? null : move(event.key, from, last);
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
				// Mid-prefix, a space is part of the name being typed; otherwise it does nothing, and
				// must not scroll the view.
				if (!typeAhead.current.active) {
					event.preventDefault();
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
					if (position === null) return;
					store.getState().moveTo(position, true);
					scrollTo(position);
				},
			);
		}
	};

	const onItemClick = (event: MouseEvent, position: number, entry: Entry | undefined) => {
		const state = store.getState();
		const modifier = event.ctrlKey || event.metaKey;
		typeAheadEpoch.current++;
		if (event.shiftKey) void state.extendTo(position, modifier);
		else if (!entry) return;
		else if (modifier) state.toggleAt(position, entry.id);
		else state.click(position, entry.id);
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
		onMenu?.({
			kind: 'entry',
			entry,
			handle: model.handle,
			position: { x: event.clientX, y: event.clientY },
			keyboard: false,
		});
	};

	const onBackgroundContextMenu = (event: MouseEvent) => {
		event.preventDefault();
		onMenu?.({
			kind: 'background',
			position: { x: event.clientX, y: event.clientY },
			keyboard: false,
		});
	};

	return { onKeyDown, onItemClick, onItemContextMenu, onBackgroundContextMenu };
}
