// The non-blocking "waiting for the system's prompt" state, with a Cancel that stops the wait
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useStore } from 'zustand';
import { t } from '../i18n/messages';
import { ShieldIcon } from '../icons/AppIcons';
import { elevationStore } from './elevationStore';
import styles from './ElevationPrompt.module.css';

/**
 * Shown while the system's administrator prompt is up (UAC, polkit) and the page is waiting on
 * it. It does not block the window: the person can keep working, and the prompt is the system's.
 * The live region is always mounted so the message is read out when it appears; Cancel is a real
 * button that stops the waiting call, which then ends as cancelled with nothing left running.
 */
export function ElevationPrompt() {
	const pending = useStore(elevationStore, (state) => state.pending);
	return (
		<div className={styles.region} role="status" aria-live="polite">
			{pending ? (
				<div className={styles.card} data-elevation-waiting="">
					<ShieldIcon className={styles.icon} width={16} height={16} />
					<span className={styles.text}>{t('elevation.waiting')}</span>
					<button type="button" className={styles.cancel} onClick={() => pending.cancel()}>
						{t('elevation.waiting.cancel')}
					</button>
				</div>
			) : null}
		</div>
	);
}
