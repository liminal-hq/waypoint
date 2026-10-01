// The context menu of a workspace in the sidebar: open all its folders, rename, delete
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Workspace } from '@liminal-hq/waypoint-protocol/generated/Workspace';
import { t } from '../i18n/messages';
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

export function WorkspaceMenu({ request, actions, onClose }: WorkspaceMenuProps) {
	const { workspace } = request;
	const items: MenuItem[] = [
		{
			type: 'action',
			id: 'openAll',
			label: t('menu.openAllInTabs'),
			disabled: workspace.locations.length === 0,
		},
		{ type: 'separator' },
		{ type: 'action', id: 'rename', label: t('menu.rename'), shortcut: 'F2' },
		{
			type: 'action',
			id: 'delete',
			label: t('menu.deleteWorkspace'),
			shortcut: 'Del',
			danger: true,
		},
	];
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
