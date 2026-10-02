// The strip above the Trash's list: Restore and Delete Permanently for the selection, and Empty Trash
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useSyncExternalStore } from 'react';
import { useStore } from 'zustand';
import { selectedCount } from '../browse/selection';
import type { ListingSession } from '../browse/useListingSession';
import { t } from '../i18n/messages';
import { RestoreIcon, TrashIcon } from '../icons/MenuIcons';
import styles from './TrashBar.module.css';
import { useTrashActions } from './trashJobs';

/**
 * Restore and Delete Permanently act on what is selected and are off until something is; Empty
 * Trash acts on everything and is off while the Trash is empty. It renders nothing where the window
 * has no Trash service, since none of the buttons could do anything.
 */
export function TrashBar({ session }: { session: ListingSession }) {
	const actions = useTrashActions();
	const { model, store } = session;
	const selection = useStore(store, (state) => state.selection);
	const count = useSyncExternalStore(model.subscribe, () => model.count);
	if (!actions) return null;
	const selected = selectedCount(selection, count);
	return (
		<div className={styles.bar} role="toolbar" aria-label={t('trash.bar.label')}>
			<button
				type="button"
				className={styles.button}
				disabled={selected === 0}
				onClick={() => actions.restore(session)}
			>
				<RestoreIcon />
				{t('trash.restore')}
			</button>
			<button
				type="button"
				className={`${styles.button} ${styles.danger}`}
				disabled={selected === 0}
				onClick={() => actions.deletePermanently(session)}
			>
				<TrashIcon />
				{t('trash.delete')}
			</button>
			<span className={styles.spacer} />
			<button
				type="button"
				className={`${styles.button} ${styles.danger}`}
				disabled={count === 0}
				onClick={() => actions.emptyTrash(count)}
			>
				{t('trash.empty')}
			</button>
		</div>
	);
}
