// The context menu of a workspace in the sidebar: open all its folders, rename, delete
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Workspace } from '@liminal-hq/waypoint-protocol/generated/Workspace';
import { t } from '../i18n/messages';
import { EditIcon, TabsIcon, TrashIcon } from '../icons/MenuIcons';
import type { WorkspaceMenuRequest } from './WorkspaceList';

export interface WorkspaceMenuActions {
	openAll(workspace: Workspace): void;
	startRename(workspace: Workspace): void;
	remove(workspace: Workspace): void;
}

interface WorkspaceMenuProps {
	request: WorkspaceMenuRequest;
	actions: WorkspaceMenuActions;
	onClose: () => void;
}

/** The workspace menu's items; Delete Workspace is last, and styled as the danger it is. */
export function workspaceMenuItems(workspace: Workspace): MenuItem[] {
	return [
		{
			type: 'action',
			id: 'openAll',
			label: t('menu.openAllInTabs'),
			disabled: workspace.locations.length === 0,
			icon: <TabsIcon />,
		},
		{ type: 'separator' },
		{ type: 'action', id: 'rename', label: t('menu.rename'), shortcut: 'F2', icon: <EditIcon /> },
		{
			type: 'action',
			id: 'delete',
			label: t('menu.deleteWorkspace'),
			shortcut: 'Del',
			danger: true,
			icon: <TrashIcon />,
		},
	];
}

export function WorkspaceMenu({ request, actions, onClose }: WorkspaceMenuProps) {
	const { workspace } = request;
	const items = workspaceMenuItems(workspace);
	return (
		<ContextMenu
			items={items}
			position={request.position}
			ariaLabel={t('sidebar.menu.workspace')}
			openedWithKeyboard={request.keyboard}
			returnFocusTo={request.returnFocus}
			onClose={onClose}
			onSelect={(item) => {
				onClose();
				switch (item.id) {
					case 'openAll':
						return actions.openAll(workspace);
					case 'rename':
						return actions.startRename(workspace);
					case 'delete':
						return actions.remove(workspace);
				}
			}}
		/>
	);
}
