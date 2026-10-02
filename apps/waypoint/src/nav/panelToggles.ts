// The toolbar's panel toggles as data: which registry commands they stand for and how each looks now
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ComponentType } from 'react';
import { tooltipFor } from '../app/actionBarModel';
import type { CommandsApi } from '../commands/commandBridge';
import type { CommandId } from '../commands/registry';
import type { IconProps } from '../icons/AppIcons';

/**
 * The panels the toolbar can toggle, left to right. Each is a registry command with a `checked`
 * state, so adding the Inspector or the Terminal drawer when they exist is one entry here (the
 * command itself, with its facts, comes with the panel). The toolbar never shows a toggle whose
 * command is not in the registry or is hidden.
 */
export const PANEL_TOGGLES: readonly CommandId[] = ['sidebar', 'splitView'];

/** The toolbar narrower than this (in pixels) moves the toggles into a More menu. */
export const COLLAPSE_BELOW = 560;

/** Whether a toolbar `width` is too narrow for the toggles. A width of 0 means not laid out: show them. */
export function shouldCollapse(width: number): boolean {
	return width > 0 && width < COLLAPSE_BELOW;
}

export interface PanelToggle {
	id: CommandId;
	/** The command's label, the button's accessible name. */
	label: string;
	/** The name and the key, or why the toggle cannot be used now. */
	tooltip: string;
	icon: ComponentType<IconProps>;
	/** Whether the panel is showing: the command's own checked state. */
	pressed: boolean;
	enabled: boolean;
}

/** The toggles on show now, resolved against the window's facts. */
export function panelToggles(api: Pick<CommandsApi, 'get'>): PanelToggle[] {
	const toggles: PanelToggle[] = [];
	for (const id of PANEL_TOGGLES) {
		const view = api.get(id);
		if (!view.visible || !view.icon) continue;
		toggles.push({
			id,
			label: view.label,
			tooltip: tooltipFor(view.label, view.shortcut, view.reason),
			icon: view.icon,
			pressed: view.checked === true,
			enabled: view.enabled,
		});
	}
	return toggles;
}
