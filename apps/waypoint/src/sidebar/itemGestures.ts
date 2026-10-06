// The pointer and keyboard gestures every sidebar item shares: click, middle-click and the context menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuPosition } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { KeyboardEvent, MouseEvent, PointerEvent } from 'react';
import { modifiersOf } from '../dnd/dropAction';
import type { FileDragApi } from '../dnd/FileDragContext';
import { dropAttributes } from '../dnd/dropTargets';

export type ItemKind = 'place' | 'trash' | 'favourite' | 'folder';

/** What a context menu needs to know about the item it was opened on. */
export interface ItemMenuRequest {
	kind: ItemKind;
	location: Location;
	label: string;
	position: MenuPosition;
	/** Opened from the keyboard, so focus goes into the menu. */
	keyboard: boolean;
	/** Where focus goes back to when the menu closes. */
	returnFocus: HTMLElement | null;
}

export interface ItemActions {
	open(location: Location): void;
	/** Middle-click: the folder opens in a tab beside this one, in the background, or in a new window with Ctrl. */
	openInNewTab(location: Location, inNewWindow?: boolean): void;
	openMenu(request: ItemMenuRequest): void;
}

export interface ItemGestureProps {
	/** Makes the item a file drop target: the Trash moves to the Trash, the rest are folders. */
	[dropAttribute: `data-drop${string}`]: string | undefined;
	onClick(event: MouseEvent<HTMLElement>): void;
	onMouseDown(event: MouseEvent<HTMLElement>): void;
	onPointerDown(event: PointerEvent<HTMLElement>): void;
	onAuxClick(event: MouseEvent<HTMLElement>): void;
	onContextMenu(event: MouseEvent<HTMLElement>): void;
	onKeyDown(event: KeyboardEvent<HTMLElement>): void;
}

/** Handlers to spread on an item's element. */
export function itemGestures(
	actions: ItemActions,
	item: { kind: ItemKind; location: Location; label: string },
	{
		droppable = true,
		drag = null,
	}: {
		droppable?: boolean;
		/**
		 * The window's file drag, which makes the item draggable: dropped on a split zone of the file
		 * area it opens in a new pane. Only a place or a tree folder is (the Trash is not a folder to
		 * open, and a favourite already uses the platform's drag to reorder).
		 */
		drag?: FileDragApi | null;
	} = {},
): ItemGestureProps {
	const draggable = drag !== null && (item.kind === 'place' || item.kind === 'folder');
	const menu = (element: HTMLElement, position: MenuPosition, keyboard: boolean) =>
		actions.openMenu({ ...item, position, keyboard, returnFocus: element });
	return {
		...(droppable
			? dropAttributes(item.kind === 'trash' ? 'trash' : 'place', item.location.uri, item.label)
			: {}),
		onClick: (event) => {
			// The click that ends a drag is not a click.
			if (drag?.consumeClick()) return;
			// A click with the middle button is reported by `auxclick`, never here; Ctrl-click has no meaning yet.
			if (event.button === 0) actions.open(item.location);
		},
		onPointerDown: (event) => {
			if (!draggable || event.button !== 0) return;
			drag.pressLocations({
				pointerId: event.pointerId,
				clientX: event.clientX,
				clientY: event.clientY,
				button: 0,
				element: event.currentTarget,
				locations: [item.location],
				name: item.label,
				groups: ['folder'],
				folder: null,
				modifiers: modifiersOf(event.nativeEvent),
				place: true,
			});
		},
		// Stops the middle button starting autoscroll on Linux before `auxclick` arrives.
		onMouseDown: (event) => {
			if (event.button === 1) event.preventDefault();
		},
		onAuxClick: (event) => {
			if (event.button !== 1) return;
			event.preventDefault();
			actions.openInNewTab(item.location, event.ctrlKey);
		},
		onContextMenu: (event) => {
			event.preventDefault();
			menu(event.currentTarget, { x: event.clientX, y: event.clientY }, false);
		},
		onKeyDown: (event) => {
			if (event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey)) {
				event.preventDefault();
				const rect = event.currentTarget.getBoundingClientRect();
				menu(event.currentTarget, { x: rect.left, y: rect.bottom }, true);
			}
		},
	};
}
