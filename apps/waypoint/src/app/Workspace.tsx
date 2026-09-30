// The browsing area of the Main window: the toolbar over the active tab's file view
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useEffect, useState, useSyncExternalStore } from 'react';
import { ListingManager } from '../browse/listingManager';
import { ListingView } from '../browse/ListView';
import { useVfsClient } from '../browse/VfsClientContext';
import { NavigationBar } from '../nav/NavigationBar';
import { useNavigation } from '../nav/useNavigation';
import { useOpenEntry } from '../nav/useOpenEntry';
import { useTabsSnapshot } from '../tabs/TabsContext';
import styles from './Workspace.module.css';

export function Workspace() {
	const client = useVfsClient();
	const snapshot = useTabsSnapshot();
	const [manager] = useState(() => new ListingManager(client));
	const navigation = useNavigation();
	const open = useOpenEntry(navigation);

	// One listing per tab, kept in step with the session (A9, A20).
	useEffect(() => {
		if (snapshot) manager.sync(snapshot.tabs, snapshot.active);
	}, [manager, snapshot]);
	useEffect(() => () => manager.dispose(), [manager]);

	useSyncExternalStore(manager.subscribe, manager.getVersion);
	const tab = navigation.tab;
	const state = tab ? manager.stateFor(tab.id) : undefined;

	return (
		<div className={styles.workspace}>
			<NavigationBar />
			<div className={styles.files}>{state && <ListingView state={state} onOpen={open} />}</div>
		</div>
	);
}
