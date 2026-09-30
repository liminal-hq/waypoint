// The Main window: a tabbed file browser over in-memory folders, until the real clients replace them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { WindowFrame } from '@liminal-hq/waypoint-chrome/WindowFrame';
import { lazy, Suspense, useState } from 'react';
import { createDemoClient, DEMO_HOME } from '../browse/demoClient';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { t } from '../i18n/messages';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { TabsProvider } from '../tabs/TabsContext';
import { AppTitleBar } from './AppTitleBar';
import styles from './MainScreen.module.css';
import { Workspace } from './Workspace';

// Development-only controls that change the demo folder under the open listing. Importing them
// behind the `DEV` constant keeps them out of a production bundle.
const DevLiveControls = import.meta.env.DEV
	? lazy(() => import('../browse/DevLiveControls').then((m) => ({ default: m.DevLiveControls })))
	: null;

export function MainScreen() {
	const [client] = useState(() => createDemoClient());
	// The session plugin is registered but the file system plugin is not built yet, so the demo
	// folders and the tabs over them both run in memory.
	const [tabsApi] = useState(() => {
		const api = new FakeTabsApi();
		void api.openTab(DEMO_HOME);
		return api;
	});
	return (
		<WindowFrame className={styles.screen}>
			<AppTitleBar title={t('window.main.title')} />
			<VfsClientProvider client={client}>
				<TabsProvider api={tabsApi}>
					<main className={styles.content}>
						{DevLiveControls && (
							<Suspense fallback={null}>
								<DevLiveControls client={client} location={DEMO_HOME} />
							</Suspense>
						)}
						<Workspace />
					</main>
				</TabsProvider>
			</VfsClientProvider>
		</WindowFrame>
	);
}
