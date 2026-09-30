// Window menu: the shared Liminal menu opened from empty title bar space
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useMemo, type ReactNode } from 'react';
import { ContextMenu } from '../ContextMenu/ContextMenu';
import type { MenuPosition, SelectableMenuItem } from '../ContextMenu/types';
import {
	CloseIcon,
	MaximiseIcon,
	MinimiseIcon,
	MoveIcon,
	PinIcon,
	RestoreIcon,
} from '../icons/icons';
import { defaultChromeLabels, type ChromeLabels } from '../labels';
import type { WindowControls } from '../TitleBar/windowControls';
import { buildWindowMenuModel, type WindowMenuActionId } from './windowMenuModel';

const iconFor: Record<WindowMenuActionId, ReactNode> = {
	restore: <RestoreIcon />,
	maximise: <MaximiseIcon />,
	minimise: <MinimiseIcon />,
	move: <MoveIcon />,
	'always-on-top': <PinIcon />,
	close: <CloseIcon />,
};

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
		() =>
			buildWindowMenuModel({ isMaximised, alwaysOnTop, showAlwaysOnTop, canMove, labels }).map(
				(item) => {
					const icon = item.id ? iconFor[item.id as WindowMenuActionId] : undefined;
					return icon && (item.type === 'action' || item.type === 'checkbox')
						? { ...item, icon }
						: item;
				},
			),
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
