// The Main window: the file list over a synthetic home folder, until the real client replaces it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { WindowFrame } from '@liminal-hq/waypoint-chrome/WindowFrame';
import { lazy, Suspense, useState } from 'react';
import { createDemoClient, DEMO_HOME } from '../browse/demoClient';
import { ListView } from '../browse/ListView';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { t } from '../i18n/messages';
import { AppTitleBar } from './AppTitleBar';
import styles from './MainScreen.module.css';

// Development-only controls that change the demo folder under the open listing. Importing them
// behind the `DEV` constant keeps them out of a production bundle.
const DevLiveControls = import.meta.env.DEV
	? lazy(() => import('../browse/DevLiveControls').then((m) => ({ default: m.DevLiveControls })))
	: null;

export function MainScreen() {
	const [client] = useState(() => createDemoClient());
	return (
		<WindowFrame className={styles.screen}>
			<AppTitleBar title={t('window.main.title')} />
			<VfsClientProvider client={client}>
				<main className={styles.content}>
					{DevLiveControls && (
						<Suspense fallback={null}>
							<DevLiveControls client={client} location={DEMO_HOME} />
						</Suspense>
					)}
					<ListView location={DEMO_HOME} />
				</main>
			</VfsClientProvider>
		</WindowFrame>
	);
}
