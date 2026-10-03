// The Inspector's empty state: nothing to look at, and what to do about it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { t } from '../i18n/messages';
import { EmptyIcon } from './InspectorIcons';
import styles from './EmptyState.module.css';

export function EmptyState() {
	return (
		<div className={styles.emptyState}>
			<EmptyIcon className={styles.icon} width={36} height={36} />
			<p className={styles.title}>{t('inspector.empty.title')}</p>
			<p className={styles.hint}>{t('inspector.empty.hint')}</p>
		</div>
	);
}
