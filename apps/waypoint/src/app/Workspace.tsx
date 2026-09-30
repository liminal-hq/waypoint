// The browsing area of the Main window: the toolbar over the active tab's file view
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useState, useSyncExternalStore } from 'react';
import { ListingManager } from '../browse/listingManager';
import type { SessionState } from '../browse/useListingSession';
import { ListingView } from '../browse/ListView';
import { useVfsClient } from '../browse/VfsClientContext';
import { TabStrip } from '../tabs/TabStrip';
import { tabDomId, TAB_PANEL_ID } from '../tabs/tabIds';
import { useTabShortcuts } from '../tabs/useTabShortcuts';
import { t } from '../i18n/messages';
import { NavigationBar } from '../nav/NavigationBar';
import { useNavigation } from '../nav/useNavigation';
import { useOpenEntry } from '../nav/useOpenEntry';
import { useTabsSnapshot } from '../tabs/TabsContext';
import styles from './Workspace.module.css';

const OPENING: SessionState = { status: 'opening' };

export function Workspace() {
	const client = useVfsClient();
	const snapshot = useTabsSnapshot();
	const [manager] = useState(() => new ListingManager(client));
	const navigation = useNavigation();
	const { open, openInNewTab } = useOpenEntry(navigation);
	useTabShortcuts();

	// One listing per tab, kept in step with the session (A9, A20).
	useEffect(() => {
		if (snapshot) manager.sync(snapshot.tabs, snapshot.active);
	}, [manager, snapshot]);
	useEffect(() => () => manager.dispose(), [manager]);

	useSyncExternalStore(manager.subscribe, manager.getVersion);
	const tab = navigation.tab;
	// A tab that has only just become active has no listing until the effect above opens one.
	const state = tab ? (manager.stateFor(tab.id) ?? OPENING) : undefined;

	return (
		<div className={styles.workspace}>
			<TabStrip />
			<NavigationBar />
			<div
				className={styles.files}
				role="tabpanel"
				id={TAB_PANEL_ID}
				aria-label={tab ? undefined : t('tabs.panel.label')}
				aria-labelledby={tab ? tabDomId(tab.id) : undefined}
			>
				{state && <ListingView state={state} onOpen={open} onOpenInNewTab={openInNewTab} />}
			</div>
		</div>
	);
}
