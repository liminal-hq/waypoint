// The application menu's items and what choosing a row does, shared by the title bar button and the menu bar
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useCallback, useMemo } from 'react';
import { appMenuItems, HISTORY_MORE_ID, parseHistoryRow } from '../commands/appMenuModel';
import { useCommandBridge, useCommands } from '../commands/commandBridge';
import type { CommandId } from '../commands/registry';

/**
 * The menu's rows, the registry's commands resolved for the window now, and the handler that runs
 * the chosen row: a command, an Undo History entry, or More in Command Palette….
 */
export function useAppMenu() {
	const { commands, facts, run } = useCommands();
	const bridge = useCommandBridge();
	const items = useMemo(() => appMenuItems(commands, facts), [commands, facts]);
	const onSelect = useCallback(
		(item: { id: string }) => {
			if (item.id === HISTORY_MORE_ID) {
				bridge.store.getState().actions.openPalette('undo');
				return;
			}
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
	return { items, onSelect };
}
