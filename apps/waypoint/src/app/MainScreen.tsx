// The Main window: the file list over a synthetic home folder, until the real client replaces it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { WindowFrame } from '@liminal-hq/waypoint-chrome/WindowFrame';
import { useState } from 'react';
import { createDemoClient, DEMO_HOME } from '../browse/demoClient';
import { ListView } from '../browse/ListView';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { t } from '../i18n/messages';
import { AppTitleBar } from './AppTitleBar';
import styles from './MainScreen.module.css';

export function MainScreen() {
	const [client] = useState(() => createDemoClient());
	return (
		<WindowFrame className={styles.screen}>
			<AppTitleBar title={t('window.main.title')} />
			<VfsClientProvider client={client}>
				<main className={styles.content}>
					<ListView location={DEMO_HOME} />
				</main>
			</VfsClientProvider>
		</WindowFrame>
	);
}
