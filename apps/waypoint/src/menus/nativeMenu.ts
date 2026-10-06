// Turns a context menu's items into a native menu description, or says why they cannot be
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem, SelectableMenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { ReactNode } from 'react';
import type { NativeMenuIcon, NativeMenuItem } from './nativeMenuClient';

/** The limits `native_menu.rs` enforces; a menu past them is shown by the page instead. */
export const MAX_NATIVE_ITEMS = 200;
export const MAX_NATIVE_DEPTH = 3;
export const MAX_NATIVE_TEXT = 256;
export const MAX_NATIVE_ICON_BYTES = 1 << 20;

/** The picture for an icon node, if it was rasterised ahead of the conversion. */
export type IconLookup = (icon: ReactNode) => NativeMenuIcon | null | undefined;

export type NativeMenuConversion =
	| {
			ok: true;
			items: NativeMenuItem[];
			/** The items a choice can name, by id, so the page runs the same `onSelect` the in-page menu would. */
			selectable: ReadonlyMap<string, SelectableMenuItem>;
			/** How many icons the native menu goes without: no picture could be made, or the item cannot carry one. */
			droppedIcons: number;
	  }
	| { ok: false; reason: string };

/** The icon nodes of the actions in `items`, in order, for the host to rasterise before it converts. */
export function collectIcons(items: readonly MenuItem[]): ReactNode[] {
	const found: ReactNode[] = [];
	for (const item of items) {
		if (item.type === 'submenu') found.push(...collectIcons(item.items));
		else if (item.type === 'action' && item.icon !== undefined && item.icon !== null) {
			found.push(item.icon);
		}
	}
	return found;
}

/** Keeps a label within what the native menu takes, ending in an ellipsis where it is cut. */
function clip(text: string): string {
	const characters = Array.from(text);
	return characters.length <= MAX_NATIVE_TEXT
		? text
		: `${characters.slice(0, MAX_NATIVE_TEXT - 1).join('')}…`;
}

class Unconvertible extends Error {}

/**
 * Converts `items` for the system's menu. It returns a reason, and the page keeps its own menu, when
 * the native menu could not show the same choices: nothing in it can be chosen, or it is larger or
 * nested deeper than the command accepts.
 *
 * What the system's menu cannot draw is left out and the menu is still used: a section heading
 * becomes a disabled item (the system has no heading), an icon on a checkbox or a submenu is
 * dropped (neither can carry one), an icon that was not rasterised is dropped, and `title`,
 * `ariaLabel` and `danger` have no native counterpart. A radio-like group stays checkboxes, as it is
 * in the model. `shortcut` is passed on, and the command shows it only where the system's menu
 * draws one.
 */
export function toNativeMenu(
	items: readonly MenuItem[],
	iconFor: IconLookup,
): NativeMenuConversion {
	const selectable = new Map<string, SelectableMenuItem>();
	let count = 0;
	let iconBytes = 0;
	let droppedIcons = 0;

	const counted = () => {
		count += 1;
		if (count > MAX_NATIVE_ITEMS) throw new Unconvertible(`more than ${MAX_NATIVE_ITEMS} items`);
	};
	const checkId = (id: string) => {
		if (Array.from(id).length > MAX_NATIVE_TEXT) throw new Unconvertible('an id is too long');
	};
	const iconOf = (icon: ReactNode): NativeMenuIcon | undefined => {
		if (icon === undefined || icon === null || icon === false) return undefined;
		const picture = iconFor(icon);
		if (!picture || iconBytes + picture.rgba.length > MAX_NATIVE_ICON_BYTES) {
			droppedIcons += 1;
			return undefined;
		}
		iconBytes += picture.rgba.length;
		return picture;
	};

	const convert = (level: readonly MenuItem[], depth: number): NativeMenuItem[] => {
		if (depth > MAX_NATIVE_DEPTH)
			throw new Unconvertible(`submenus nested more than ${MAX_NATIVE_DEPTH} deep`);
		return level.map((item): NativeMenuItem => {
			switch (item.type) {
				case 'separator':
					return { kind: 'separator' };
				case 'section':
					counted();
					return {
						kind: 'action',
						id: `section:${count}`,
						label: clip(item.label),
						enabled: false,
					};
				case 'action': {
					counted();
					checkId(item.id);
					selectable.set(item.id, item);
					const icon = iconOf(item.icon);
					return {
						kind: 'action',
						id: item.id,
						label: clip(item.label),
						enabled: !item.disabled,
						...(item.shortcut ? { shortcut: item.shortcut } : {}),
						...(icon ? { icon } : {}),
					};
				}
				case 'checkbox': {
					counted();
					checkId(item.id);
					selectable.set(item.id, item);
					if (item.icon !== undefined && item.icon !== null) droppedIcons += 1;
					return {
						kind: 'checkbox',
						id: item.id,
						label: clip(item.label),
						checked: item.checked,
						enabled: !item.disabled,
						...(item.shortcut ? { shortcut: item.shortcut } : {}),
					};
				}
				case 'submenu':
					counted();
					if (item.icon !== undefined && item.icon !== null) droppedIcons += 1;
					return {
						kind: 'submenu',
						label: clip(item.label),
						enabled: !item.disabled,
						items: convert(item.items, depth + 1),
					};
				default:
					throw new Unconvertible('an item of a kind the native menu does not have');
			}
		});
	};

	try {
		const converted = convert(items, 0);
		if (selectable.size === 0) return { ok: false, reason: 'nothing in it can be chosen' };
		return { ok: true, items: converted, selectable, droppedIcons };
	} catch (error) {
		if (error instanceof Unconvertible) return { ok: false, reason: error.message };
		throw error;
	}
}
