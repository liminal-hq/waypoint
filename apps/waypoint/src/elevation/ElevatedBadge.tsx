// The title bar's Administrator badge: a button that says the window is in Administrator Mode and leaves it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useId } from 'react';
import { useStore } from 'zustand';
import { useCommandBridge } from '../commands/commandBridge';
import { t } from '../i18n/messages';
import { ShieldIcon } from '../icons/AppIcons';
import styles from './ElevatedBadge.module.css';

/**
 * Shown in the Main window's title bar only while the active tab is elevated. It is a shield and
 * the word "Administrator", never colour alone; its name is the word and its description says what
 * pressing it does (Leave Administrator Mode), which it runs for the active tab.
 */
export function ElevatedBadge() {
	const bridge = useCommandBridge();
	const elevated = useStore(bridge.store, (env) => env.facts.elevated);
	const describedBy = useId();
	if (!elevated) return null;
	return (
		<>
			<button
				type="button"
				className={styles.badge}
				aria-describedby={describedBy}
				title={t('cmd.leaveAdministrator')}
				onClick={() => bridge.store.getState().actions.leaveAdministrator()}
			>
				<ShieldIcon className={styles.icon} width={14} height={14} />
				<span className={styles.word}>{t('elevation.administrator')}</span>
			</button>
			<span id={describedBy} hidden>
				{t('cmd.leaveAdministrator')}
			</span>
		</>
	);
}
