// The application menu in the title bar: File, Edit, View and Window over the window's commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { AppMenuButton } from '@liminal-hq/waypoint-chrome/TitleBar/AppMenuButton';
import { useCallback, useMemo } from 'react';
import { APP_MENU_MNEMONICS, appMenuItems, parseHistoryRow } from '../commands/appMenuModel';
import { useCommandBridge, useCommands } from '../commands/commandBridge';
import type { CommandId } from '../commands/registry';
import { t } from '../i18n/messages';
import { AppMarkIcon } from '../icons/AppIcons';

/**
 * The menu button for a Main window's title bar start slot. Its rows are the registry's commands
 * resolved for the window now, so a row is hidden, disabled with its reason as the tooltip, or
 * checked exactly as the Action bar and the keys see it, and choosing one runs the registry's
 * command. F10, a lone Alt and Alt+F, E, V and W open it (with the menu named by the letter open).
 */
export function AppMenu() {
	const { commands, facts, run } = useCommands();
	const bridge = useCommandBridge();
	const items = useMemo(() => appMenuItems(commands, facts), [commands, facts]);
	const onSelect = useCallback(
		(item: { id: string }) => {
			const history = parseHistoryRow(item.id);
			if (history) {
				const { actions } = bridge.store.getState();
				if (history.kind === 'undo') actions.undoEntry(history.entry);
				else actions.redoEntry(history.entry);
				return;
			}
			run(item.id as CommandId);
		},
		[bridge, run],
	);
	return (
		<AppMenuButton
			label={t('app.name')}
			mark={<AppMarkIcon />}
			items={items}
			onSelect={onSelect}
			mnemonics={APP_MENU_MNEMONICS}
		/>
	);
}
