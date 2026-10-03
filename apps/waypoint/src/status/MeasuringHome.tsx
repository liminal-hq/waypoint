// The status bar's "Measuring Home" item: shown while the directory-size scan runs, with how far it is
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useSyncExternalStore } from 'react';
import { t, tf } from '../i18n/messages';
import { useHomeScanStore } from '../overview/HomeScanContext';
import { OVERVIEW_LOCATION } from '../overview/overviewLocation';
import { useNavigation } from '../nav/useNavigation';
import styles from './StatusBar.module.css';

const NO_SUBSCRIBE = () => () => {};
const NOT_RUNNING = () => false;

/**
 * Nothing at all unless the window is measuring Home. While it is, a button shows the folders done
 * and takes the active tab to Overview, and a second one stops the scan.
 */
export function MeasuringHome() {
	const store = useHomeScanStore();
	const running = useSyncExternalStore(
		store?.subscribe ?? NO_SUBSCRIBE,
		store ? () => store.getSnapshot().running : NOT_RUNNING,
	);
	if (!store || !running) return null;
	return <MeasuringHomeItem />;
}

function MeasuringHomeItem() {
	const store = useHomeScanStore();
	const { goTo } = useNavigation();
	const scan = useSyncExternalStore(store!.subscribe, store!.getSnapshot);
	const total = scan.shown?.foldersTotal ?? 0;
	const text =
		total > 0
			? tf('status.measuringHome.progress', { done: scan.shown?.foldersScanned ?? 0, total })
			: t('status.measuringHome');
	return (
		<span className={styles.measuring}>
			<button
				type="button"
				className={styles.measuringOpen}
				title={t('status.measuringHome.open')}
				onClick={() => void goTo(OVERVIEW_LOCATION)}
			>
				{text}
			</button>
			<button
				type="button"
				className={styles.measuringCancel}
				aria-label={t('status.measuringHome.cancel')}
				title={t('status.measuringHome.cancel')}
				onClick={() => store?.cancel()}
			>
				×
			</button>
		</span>
	);
}
