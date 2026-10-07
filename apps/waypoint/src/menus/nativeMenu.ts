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

/** What colours an item's icon: the page's menu draws a dangerous item's icon in the danger colour, and marks a checked swatch. */
export interface IconLook {
	danger?: boolean;
	checked?: boolean;
}

/** A picture of an icon. `marksCheck` says the picture itself shows whether its item is checked (a swatch's ring). */
export interface IconPicture extends NativeMenuIcon {
	marksCheck?: boolean;
}

/** An icon to rasterise before the conversion, and the look it is drawn in. */
export interface IconRequest extends IconLook {
	icon: ReactNode;
}

/** The picture for an icon node in a look, if it was rasterised ahead of the conversion. */
export type IconLookup = (icon: ReactNode, look: IconLook) => IconPicture | null | undefined;

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

/**
 * The icons of the actions and checkboxes in `items`, in order, with the look each is drawn in, for
 * the host to rasterise before it converts. A submenu's own icon is not asked for: it cannot carry one.
 */
export function collectIcons(items: readonly MenuItem[]): IconRequest[] {
	const found: IconRequest[] = [];
	for (const item of items) {
		if (item.type === 'submenu') found.push(...collectIcons(item.items));
		else if (item.type === 'action' && item.icon !== undefined && item.icon !== null) {
			found.push({ icon: item.icon, danger: item.danger === true });
		} else if (item.type === 'checkbox' && item.icon !== undefined && item.icon !== null) {
			found.push({ icon: item.icon, checked: item.checked });
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
 * becomes a disabled item (the system has no heading), an icon on a submenu is dropped (it cannot
 * carry one), an icon that was not rasterised is dropped, and `title` and `ariaLabel` have no native
 * counterpart. `danger` is shown through the icon alone: the page draws a dangerous item's icon in
 * the danger colour, and the label keeps the system's colour. A checkbox cannot carry an icon, so
 * its icon is dropped unless the picture marks the check itself (a colour swatch with a ring round
 * it when checked), and then the item is an icon action instead, with no check mark of the
 * system's. A radio-like group stays checkboxes, as it is in the model. `shortcut` is passed on, and
 * the command shows it only where the system's menu draws one.
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
	const take = (picture: IconPicture | null | undefined): NativeMenuIcon | undefined => {
		if (!picture || iconBytes + picture.rgba.length > MAX_NATIVE_ICON_BYTES) {
			droppedIcons += 1;
			return undefined;
		}
		iconBytes += picture.rgba.length;
		return { width: picture.width, height: picture.height, rgba: picture.rgba };
	};
	const iconOf = (icon: ReactNode, look: IconLook): NativeMenuIcon | undefined => {
		if (icon === undefined || icon === null || icon === false) return undefined;
		return take(iconFor(icon, look));
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
					const icon = iconOf(item.icon, { danger: item.danger === true });
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
					if (item.icon !== undefined && item.icon !== null && item.icon !== false) {
						const picture = iconFor(item.icon, { checked: item.checked });
						if (picture?.marksCheck) {
							const icon = take(picture);
							if (icon) {
								return {
									kind: 'action',
									id: item.id,
									label: clip(item.label),
									enabled: !item.disabled,
									...(item.shortcut ? { shortcut: item.shortcut } : {}),
									icon,
								};
							}
						} else {
							droppedIcons += 1;
						}
					}
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
