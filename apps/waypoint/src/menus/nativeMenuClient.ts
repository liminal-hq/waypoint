// What the page needs from the `show_native_menu` command: show a menu as the system's own and say what was chosen
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

/** A picture for a menu item: straight (not premultiplied) RGBA, `width` by `height` pixels. */
export interface NativeMenuIcon {
	width: number;
	height: number;
	rgba: number[];
}

/**
 * One entry of a native menu, as `native_menu.rs` reads it. An item's `id` is the page's own; the
 * command answers with it. A submenu has no id because opening it chooses nothing.
 */
export type NativeMenuItem =
	| {
			kind: 'action';
			id: string;
			label: string;
			enabled?: boolean;
			shortcut?: string;
			icon?: NativeMenuIcon;
	  }
	| {
			kind: 'checkbox';
			id: string;
			label: string;
			checked: boolean;
			enabled?: boolean;
			shortcut?: string;
	  }
	| {
			kind: 'submenu';
			label: string;
			enabled?: boolean;
			icon?: NativeMenuIcon;
			items: NativeMenuItem[];
	  }
	| { kind: 'separator' };

/** Where the menu's top-left corner goes: logical pixels from the window's top-left corner. */
export interface NativeMenuPoint {
	x: number;
	y: number;
}

export interface NativeMenuClient {
	/**
	 * Shows `items` as the system's menu, and resolves with the chosen item's id once it closes, or
	 * `null` if it was dismissed. Rejects when no usable menu was shown, so the page can show its own.
	 */
	show(items: NativeMenuItem[], at: NativeMenuPoint | null): Promise<string | null>;
}
