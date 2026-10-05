// One row's Storage class cell: the class in words, and for an archived object a plain note that it needs a restore
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { t } from '../i18n/messages';
import styles from './ListView.module.css';
import { storageClassLabel, storageClassNeedsRestore, storageClassOf } from './storageClass';

/** A folder has no class and shows a dash; an archived class says so in words (and to a screen reader), never by colour alone. */
export function StorageClassCell({ entry }: { entry: Entry }) {
	const name = storageClassOf(entry);
	if (name === null) {
		return (
			<span className={styles.cell} data-column="storageClass">
				{t('browse.value.none')}
			</span>
		);
	}
	const label = storageClassLabel(name);
	const archived = storageClassNeedsRestore(name);
	return (
		<span
			className={styles.cell}
			data-column="storageClass"
			data-archived={archived ? '' : undefined}
			title={archived ? t('browse.storageClass.restoreNote') : label}
		>
			{label}
			{archived && (
				<span className={styles.srOnly}>{`, ${t('browse.storageClass.restoreNote')}`}</span>
			)}
		</span>
	);
}
