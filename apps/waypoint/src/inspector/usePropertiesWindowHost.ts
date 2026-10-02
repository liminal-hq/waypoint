// Gives the window its "Properties in a window": the command's fact and action, and Alt+Enter
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { useCallback, useEffect } from 'react';
import { useVfsClient } from '../browse/VfsClientContext';
import type { ListingSession } from '../browse/useListingSession';
import { runCommand } from '../commands/registry';
import type { CommandBridge } from '../commands/commandBridge';
import { openPropertiesWindowFor, propertiesSubject } from './openPropertiesWindow';
import { usePropertiesWindowClient } from './PropertiesWindowContext';
import { usePropertiesWindowShortcut } from './usePropertiesWindowShortcut';

/**
 * Wires Properties windows into a main window: the `propertiesWindow` fact (the service exists),
 * the `openPropertiesWindow` action for the active pane, and Alt+Enter, which runs the command
 * only where the registry offers it. Returns the opener the item menu uses for the entry that
 * was right-clicked.
 */
export function usePropertiesWindowHost(
	bridge: CommandBridge,
	activeSession: () => ListingSession | null,
): (from: ListingSession | null, entry?: Entry) => void {
	const client = usePropertiesWindowClient();
	const vfs = useVfsClient();

	const open = useCallback(
		(from: ListingSession | null, entry?: Entry) => {
			const session = from ?? activeSession();
			if (!client || !session) return;
			propertiesSubject(vfs, session, entry).then(
				(location) => {
					if (location) return openPropertiesWindowFor(client, location);
				},
				(error: unknown) => console.warn('could not name the Properties subject', error),
			);
		},
		[client, vfs, activeSession],
	);

	useEffect(() => {
		bridge.patchFacts({ propertiesWindow: client !== null });
	}, [bridge, client]);
	useEffect(() => {
		bridge.patchActions({ openPropertiesWindow: () => open(null) });
	}, [bridge, open]);

	const onKey = useCallback(() => {
		const { actions, facts } = bridge.store.getState();
		return runCommand('propertiesInWindow', actions, facts);
	}, [bridge]);
	usePropertiesWindowShortcut(onKey);

	return open;
}
