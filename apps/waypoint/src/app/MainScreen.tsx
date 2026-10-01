// The Main window: a tabbed file browser over the file system and session plugins
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { getHome } from '@liminal-hq/waypoint-plugin-vfs';
import { invoke } from '@tauri-apps/api/core';
import { WindowFrame } from '@liminal-hq/waypoint-chrome/WindowFrame';
import { lazy, Suspense, useEffect, useRef, useState } from 'react';
import { TimeFormatProvider } from '../browse/TimeFormatContext';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { t } from '../i18n/messages';
import { MainOps } from '../ops/MainOps';
import { collectServiceStatuses } from '../services/serviceStatuses';
import { tabsApi } from '../services/tabsApi';
import { createTauriOpsClient } from '../services/tauriOpsClient';
import { createTauriOsClipboardClient } from '../services/tauriOsClipboardClient';
import { createTauriPlacesClient } from '../services/tauriPlacesClient';
import { createTauriTearoffClient } from '../services/tauriTearoffClient';
import { createTauriTimeFormatClient } from '../services/tauriTimeFormatClient';
import { PlacesClientProvider } from '../sidebar/PlacesClientContext';
import { createTauriVfsClient } from '../services/tauriVfsClient';
import { createTauriTrashClient } from '../trash/tauriTrashClient';
import { TrashClientProvider } from '../trash/TrashClientContext';
import { TabsProvider } from '../tabs/TabsContext';
import { CommandBridgeProvider, createCommandBridge } from '../commands/commandBridge';
import { AppMenu } from './AppMenu';
import { AppTitleBar } from './AppTitleBar';
import { startMainServices, type MainServices } from './mainServices';
import styles from './MainScreen.module.css';
import { WindowCommands } from './WindowCommands';
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

// The timing harness for re-measuring the list against the budgets: development builds, and any
// build made with `VITE_WAYPOINT_PERF=1` (a release build to measure has no MCP bridge to drive it).
if (import.meta.env.DEV || import.meta.env.VITE_WAYPOINT_PERF === '1') {
	void import('../dev/perfHarness').then((m) => m.installPerfHarness());
}

function start(): Promise<MainServices> {
	if (wantsDemo()) return import('./demoServices').then((m) => m.startDemoServices());
	// What works on this system, once, for the log; the Services panel reads the same sources.
	void collectServiceStatuses().then(
		(statuses) => console.info('service statuses', JSON.stringify(statuses)),
		(error: unknown) => console.warn('could not read the service statuses', error),
	);
	return startMainServices({
		getHome,
		tabsApi,
		getRestoreNotice: () => invoke<string | null>('take_restore_notice'),
		createClient: createTauriVfsClient,
		createPlacesClient: createTauriPlacesClient,
		createTearoffClient: createTauriTearoffClient,
		createTimeFormatClient: createTauriTimeFormatClient,
		createOpsClient: createTauriOpsClient,
		createOsClipboardClient: createTauriOsClipboardClient,
		createTrashClient: createTauriTrashClient,
	});
}

type Startup =
	| { state: 'starting' }
	| { state: 'ready'; services: MainServices }
	| { state: 'failed'; reason: string };

export function MainScreen() {
	// What the menu, the Action bar and the keys' commands read: the workspace and the window publish into it.
	const [bridge] = useState(createCommandBridge);
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
		<CommandBridgeProvider value={bridge}>
			<WindowFrame className={styles.screen}>
				<WindowCommands />
				<AppTitleBar title={t('window.main.title')} start={<AppMenu />} />
				{startup.state === 'failed' ? (
					<main className={styles.content}>
						<p role="alert" className={styles.failure}>
							{t('window.main.startFailed')} <span data-selectable="">{startup.reason}</span>
						</p>
					</main>
				) : startup.state === 'ready' ? (
					<VfsClientProvider client={startup.services.client}>
						<PlacesClientProvider client={startup.services.placesClient}>
							<TimeFormatProvider client={startup.services.timeFormat}>
								<TrashClientProvider client={startup.services.trash}>
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
											<MainOps
												client={startup.services.ops}
												osClipboard={startup.services.osClipboard}
											>
												<Workspace
													startup={{ view: startup.services.view, notice: startup.services.notice }}
													tearoff={startup.services.tearoff}
												/>
											</MainOps>
										</main>
									</TabsProvider>
								</TrashClientProvider>
							</TimeFormatProvider>
						</PlacesClientProvider>
					</VfsClientProvider>
				) : null}
			</WindowFrame>
		</CommandBridgeProvider>
	);
}
