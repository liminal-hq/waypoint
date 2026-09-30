// Window menu: the shared Liminal menu opened from empty title bar space
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useMemo } from 'react';
import { ContextMenu } from '../ContextMenu/ContextMenu';
import type { MenuPosition, SelectableMenuItem } from '../ContextMenu/types';
import { defaultChromeLabels, type ChromeLabels } from '../labels';
import type { WindowControls } from '../TitleBar/windowControls';
import { buildWindowMenuModel, type WindowMenuActionId } from './windowMenuModel';

export interface WindowMenuProps {
	controls: WindowControls;
	position: MenuPosition;
	isMaximised: boolean;
	alwaysOnTop: boolean;
	showAlwaysOnTop?: boolean;
	onAlwaysOnTopChange: (value: boolean) => void;
	onClose: () => void;
	labels?: ChromeLabels;
}

export function WindowMenu({
	controls,
	position,
	isMaximised,
	alwaysOnTop,
	showAlwaysOnTop = true,
	onAlwaysOnTopChange,
	onClose,
	labels = defaultChromeLabels,
}: WindowMenuProps) {
	const canMove = controls.startDragging !== undefined;
	const items = useMemo(
		() => buildWindowMenuModel({ isMaximised, alwaysOnTop, showAlwaysOnTop, canMove, labels }),
		[isMaximised, alwaysOnTop, showAlwaysOnTop, canMove, labels],
	);

	const handleSelect = (item: SelectableMenuItem) => {
		switch (item.id as WindowMenuActionId) {
			case 'restore':
			case 'maximise':
				void controls.toggleMaximize();
				break;
			case 'minimise':
				void controls.minimize();
				break;
			case 'move':
				void controls.startDragging?.();
				break;
			case 'always-on-top':
				onAlwaysOnTopChange(!alwaysOnTop);
				break;
			case 'close':
				void controls.close();
				break;
		}
	};

	return (
		<ContextMenu
			items={items}
			position={position}
			onSelect={handleSelect}
			onClose={onClose}
			ariaLabel={labels.windowMenu}
		/>
	);
}
