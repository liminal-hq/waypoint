// Pure builder for the shared Liminal window menu model
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { defaultChromeLabels, type ChromeLabels } from '../labels';
import { createElement } from 'react';
import type { MenuItem } from '../ContextMenu/types';
import {
	CloseIcon,
	MaximiseIcon,
	MinimiseIcon,
	MoreIcon,
	MoveIcon,
	PinIcon,
	RestoreIcon,
} from '../icons/icons';

export type WindowMenuActionId =
	'restore' | 'maximise' | 'minimise' | 'move' | 'always-on-top' | 'system-menu' | 'close';

export interface WindowMenuModelOptions {
	isMaximised: boolean;
	alwaysOnTop: boolean;
	/** Include the Always on Top checkbox. Defaults to true. */
	showAlwaysOnTop?: boolean;
	/** Include Move (needs a host that can start dragging). Defaults to true. */
	canMove?: boolean;
	/** Include "More options…", which hands over to the compositor's own menu. Defaults to false. */
	canShowSystemMenu?: boolean;
	labels?: Pick<
		ChromeLabels,
		'restore' | 'maximise' | 'minimise' | 'move' | 'alwaysOnTop' | 'systemWindowMenu' | 'close'
	>;
}

/**
 * Restore/Maximise and Minimise, then Move, then Always on Top, then More options,
 * then Close — in that order, each in its own section.
 */
export function buildWindowMenuModel({
	isMaximised,
	alwaysOnTop,
	showAlwaysOnTop = true,
	canMove = true,
	canShowSystemMenu = false,
	labels = defaultChromeLabels,
}: WindowMenuModelOptions): MenuItem[] {
	const items: MenuItem[] = [
		isMaximised
			? { type: 'action', id: 'restore', label: labels.restore, icon: createElement(RestoreIcon) }
			: {
					type: 'action',
					id: 'maximise',
					label: labels.maximise,
					icon: createElement(MaximiseIcon),
				},
		{ type: 'action', id: 'minimise', label: labels.minimise, icon: createElement(MinimiseIcon) },
	];
	if (canMove) {
		items.push({ type: 'separator', id: 'sep-move' });
		items.push({ type: 'action', id: 'move', label: labels.move, icon: createElement(MoveIcon) });
	}
	if (showAlwaysOnTop) {
		items.push({ type: 'separator', id: 'sep-top' });
		items.push({
			type: 'checkbox',
			id: 'always-on-top',
			label: labels.alwaysOnTop,
			icon: createElement(PinIcon),
			checked: alwaysOnTop,
		});
	}
	if (canShowSystemMenu) {
		items.push({ type: 'separator', id: 'sep-system' });
		items.push({
			type: 'action',
			id: 'system-menu',
			label: labels.systemWindowMenu,
			icon: createElement(MoreIcon),
		});
	}
	items.push({ type: 'separator', id: 'sep-close' });
	items.push({ type: 'action', id: 'close', label: labels.close, icon: createElement(CloseIcon) });
	return items;
}
