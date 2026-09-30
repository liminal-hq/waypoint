// Pure builder for the shared Liminal window menu model
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { defaultChromeLabels, type ChromeLabels } from '../labels';
import type { MenuItem } from '../ContextMenu/types';

export type WindowMenuActionId =
	'restore' | 'maximise' | 'minimise' | 'move' | 'always-on-top' | 'close';

export interface WindowMenuModelOptions {
	isMaximised: boolean;
	alwaysOnTop: boolean;
	/** Include the Always on Top checkbox. Defaults to true. */
	showAlwaysOnTop?: boolean;
	/** Include Move (needs a host that can start dragging). Defaults to true. */
	canMove?: boolean;
	labels?: Pick<
		ChromeLabels,
		'restore' | 'maximise' | 'minimise' | 'move' | 'alwaysOnTop' | 'close'
	>;
}

/** Restore/Maximise, Minimise, Move, Always on Top, Close — in that order. */
export function buildWindowMenuModel({
	isMaximised,
	alwaysOnTop,
	showAlwaysOnTop = true,
	canMove = true,
	labels = defaultChromeLabels,
}: WindowMenuModelOptions): MenuItem[] {
	const items: MenuItem[] = [
		isMaximised
			? { type: 'action', id: 'restore', label: labels.restore }
			: { type: 'action', id: 'maximise', label: labels.maximise },
		{ type: 'action', id: 'minimise', label: labels.minimise },
	];
	if (canMove) items.push({ type: 'action', id: 'move', label: labels.move });
	if (showAlwaysOnTop) {
		items.push({ type: 'separator', id: 'sep-top' });
		items.push({
			type: 'checkbox',
			id: 'always-on-top',
			label: labels.alwaysOnTop,
			checked: alwaysOnTop,
		});
	}
	items.push({ type: 'separator', id: 'sep-close' });
	items.push({ type: 'action', id: 'close', label: labels.close });
	return items;
}
