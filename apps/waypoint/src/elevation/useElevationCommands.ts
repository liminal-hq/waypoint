// Publishes Administrator Mode to the command bridge: whether it is offered and whether this tab is in it, and the two actions
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useEffect, useRef } from 'react';
import { showNotice } from '../app/notices';
import { selectedCount } from '../browse/selection';
import type { ListingSession } from '../browse/useListingSession';
import { useVfsClient } from '../browse/VfsClientContext';
import { useCommandBridge } from '../commands/commandBridge';
import { useConnections } from '../connections/ConnectionsContext';
import { selectedEntries } from '../dnd/openFolders';
import { t } from '../i18n/messages';
import { isFolder } from '../nav/useOpenEntry';
import { announce } from '../tabs/announcer';
import { useTabsApi, useTabsSnapshot } from '../tabs/TabsContext';
import { isElevatedLocation, isFileUri } from './elevatedLocation';
import { useElevationAvailable } from './ElevationContext';
import { leaveAdministrator, openAsAdministrator } from './elevationFlow';
import { elevationStore } from './elevationStore';

/**
 * Called once by the workspace. `elevation` (offered here) needs the setting on, the plugin
 * reporting that a helper can start and a connections client to ask through; `elevated` is read
 * from the active tab's location alone, so nothing about it is stored. Open as Administrator acts
 * on the folder it is given (the permission-denied state and the context menus give one), or else
 * on the one selected folder, or the current folder when nothing is selected.
 */
export function useElevationCommands(activeSession: () => ListingSession | null): void {
	const bridge = useCommandBridge();
	const connections = useConnections();
	const available = useElevationAvailable();
	const tabs = useTabsApi();
	const snapshot = useTabsSnapshot();
	const vfs = useVfsClient();
	const active = snapshot?.tabs.find((tab) => tab.id === snapshot.active);
	const elevated = isElevatedLocation(active?.location);
	const offered = available && connections !== null;

	const latest = useRef({ connections, offered, tabs, active, vfs, activeSession });
	latest.current = { connections, offered, tabs, active, vfs, activeSession };

	useEffect(() => {
		bridge.patchFacts({ elevation: offered, elevated });
	}, [bridge, offered, elevated]);

	useEffect(() => {
		/** The folder to elevate: `given`, else the one selected folder, else the current one; `null` after saying why not. */
		const targetOf = async (given: Location | undefined): Promise<Location | null> => {
			if (given) return given;
			const session = latest.current.activeSession();
			if (!session) return null;
			const { model, store } = session;
			const { selection } = store.getState();
			if (selectedCount(selection, model.count) === 0) return model.location;
			const chosen = await selectedEntries(model, selection, 2);
			const [only] = chosen;
			if (chosen.length === 1 && only && isFolder(only)) {
				return latest.current.vfs.entryLocation(model.handle, only.id);
			}
			showNotice(t('cmd.reason.elevateFolder'));
			return null;
		};
		bridge.patchActions({
			openAsAdministrator: (location) => {
				void (async () => {
					const { connections: found, offered: on, tabs: api, active: tab } = latest.current;
					// One prompt at a time: asking again while the system's prompt is up does nothing.
					if (!found || !on || elevationStore.getState().pending) return;
					const target = await targetOf(location);
					// Only an ordinary local folder can be opened as an administrator.
					if (!target || !isFileUri(target.uri)) return;
					await openAsAdministrator(
						{ client: found.client, tabs: api, active: tab, say: showNotice, announce },
						target,
					);
				})().catch((error: unknown) => {
					console.warn('could not open the folder as an administrator', error);
				});
			},
			leaveAdministrator: () => {
				const { tabs: api, active: tab } = latest.current;
				leaveAdministrator({ tabs: api, active: tab, announce }).catch((error: unknown) => {
					console.warn('could not leave Administrator Mode', error);
				});
			},
		});
	}, [bridge]);
}
