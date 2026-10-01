// The Operations window: the queue's job list popped out of the status bar, on its own store over the same plugin
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { WindowFrame } from '@liminal-hq/waypoint-chrome/WindowFrame';
import { useCallback, useState } from 'react';
import { t } from '../i18n/messages';
import { OpsProvider } from '../ops/OpsContext';
import { OpsPanel } from '../ops/OpsPanel';
import { startOpsAnnouncer } from '../ops/opsAnnouncer';
import type { OpsHandle } from '../ops/opsStore';
import { currentWindowLabel } from '../ops/windowLabel';
import type { OpsClient } from '../services/opsClient';
import { createTauriOpsClient } from '../services/tauriOpsClient';
import { announce, useAnnouncement } from '../tabs/announcer';
import { AppTitleBar } from './AppTitleBar';
import { NoticeToast } from './NoticeToast';
import styles from './OpsScreen.module.css';

interface OpsScreenProps {
	/** The plugin's client; the real one unless a test supplies its own. */
	client?: OpsClient;
}

/**
 * Lists every job of every window. It has its own store (each window follows the queue itself, and
 * progress is sent to the window that subscribed) and speaks for all jobs, since no tab strip here
 * has a live region of its own.
 */
export function OpsScreen({ client }: OpsScreenProps) {
	const [own] = useState(() => client ?? createTauriOpsClient());
	const announcement = useAnnouncement();
	const onHandle = useCallback((handle: OpsHandle) => startOpsAnnouncer(handle, { announce }), []);
	return (
		<WindowFrame className={styles.screen}>
			<AppTitleBar title={t('window.ops.title')} />
			<OpsProvider client={own} windowLabel={currentWindowLabel()} onHandle={onHandle}>
				<main className={styles.content}>
					<OpsPanel layout="window" />
				</main>
			</OpsProvider>
			<NoticeToast />
			<div className={styles.srOnly} role="status" aria-live="polite">
				{announcement}
			</div>
		</WindowFrame>
	);
}
