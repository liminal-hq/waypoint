// The Main window: a tabbed file browser over the file system and session plugins
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getHome } from '@liminal-hq/waypoint-plugin-vfs';
import { WindowFrame } from '@liminal-hq/waypoint-chrome/WindowFrame';
import { lazy, Suspense, useEffect, useRef, useState } from 'react';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { t } from '../i18n/messages';
import { tabsApi } from '../services/tabsApi';
import { createTauriVfsClient } from '../services/tauriVfsClient';
import { TabsProvider } from '../tabs/TabsContext';
import { AppTitleBar } from './AppTitleBar';
import { startMainServices, type MainServices } from './mainServices';
import styles from './MainScreen.module.css';
import { Workspace } from './Workspace';

// Development-only controls that change the demo folder under the open listing. Importing them
// behind the `DEV` constant keeps them out of a production bundle.
const DevLiveControls = import.meta.env.DEV
	? lazy(() => import('../browse/DevLiveControls').then((m) => ({ default: m.DevLiveControls })))
	: null;

/** `?demo` in the URL of a dev build runs on in-memory folders instead of the real plugins. */
function wantsDemo(): boolean {
	return import.meta.env.DEV && new URLSearchParams(window.location.search).has('demo');
}

function start(): Promise<MainServices> {
	if (wantsDemo()) return import('./demoServices').then((m) => m.startDemoServices());
	return startMainServices({ getHome, tabsApi, createClient: createTauriVfsClient });
}

type Startup =
	| { state: 'starting' }
	| { state: 'ready'; services: MainServices }
	| { state: 'failed'; reason: string };

export function MainScreen() {
	const [startup, setStartup] = useState<Startup>({ state: 'starting' });
	// React runs effects twice in development; starting twice would open a second tab.
	const started = useRef(false);
	useEffect(() => {
		if (started.current) return;
		started.current = true;
		start().then(
			(services) => setStartup({ state: 'ready', services }),
			(error: unknown) =>
				setStartup({
					state: 'failed',
					reason: error instanceof Error ? error.message : String(error),
				}),
		);
	}, []);

	return (
		<WindowFrame className={styles.screen}>
			<AppTitleBar title={t('window.main.title')} />
			{startup.state === 'failed' ? (
				<main className={styles.content}>
					<p role="alert" className={styles.failure}>
						{t('window.main.startFailed')} <span data-selectable="">{startup.reason}</span>
					</p>
				</main>
			) : startup.state === 'ready' ? (
				<VfsClientProvider client={startup.services.client}>
					<TabsProvider api={startup.services.tabsApi} home={startup.services.home}>
						<main className={styles.content}>
							{DevLiveControls && startup.services.demo && (
								<Suspense fallback={null}>
									<DevLiveControls
										client={startup.services.demo.client}
										location={startup.services.home}
									/>
								</Suspense>
							)}
							<Workspace />
						</main>
					</TabsProvider>
				</VfsClientProvider>
			) : null}
		</WindowFrame>
	);
}
