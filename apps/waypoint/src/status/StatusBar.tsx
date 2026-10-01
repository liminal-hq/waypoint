// The status bar: what is selected and how big, how many items, free space, and a slot for the view switcher
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useSyncExternalStore, type ReactNode } from 'react';
import { formatSize } from '../browse/format';
import { useVfsClient } from '../browse/VfsClientContext';
import type { ListingSession } from '../browse/useListingSession';
import { t, tf, tn } from '../i18n/messages';
import styles from './StatusBar.module.css';
import { useFreeSpace } from './useFreeSpace';
import { useSelectionSummary } from './useSelectionSummary';

interface StatusBarProps {
	/** The active tab's listing once it is open. */
	session: ListingSession | null;
	location: Location | undefined;
	/** A failure to report, such as a file that would not open. */
	notice: string | null;
	/** The view switcher. */
	children?: ReactNode;
}

const NO_SUBSCRIBE = () => () => {};

export function StatusBar({ session, location, notice, children }: StatusBarProps) {
	const client = useVfsClient();
	const summary = useSelectionSummary(client, session);
	const revision = useSyncExternalStore(
		session?.model.subscribe ?? NO_SUBSCRIBE,
		() => session?.model.revision ?? 0,
	);
	const space = useFreeSpace(client, location, revision);

	const selecting = summary.count > 0;
	const items = tn('status.items', summary.total);
	const selection = tn('browse.selection', summary.count);
	const announcement = !summary.touched
		? ''
		: summary.count === 0
			? t('browse.selection.none')
			: selection;

	return (
		<div className={styles.bar} role="group" aria-label={t('status.bar.label')}>
			<span className={styles.items}>{session ? items : ''}</span>
			{selecting && (
				<span className={styles.selection} data-pending={summary.pending ? '' : undefined}>
					{selection}
					{summary.size !== null && ` · ${formatSize(summary.size)}`}
				</span>
			)}
			{notice && (
				<span className={styles.notice} role="alert">
					{notice}
				</span>
			)}
			<span className={styles.spacer} />
			{space && (
				<span className={styles.space}>
					{tf('status.free', { size: formatSize(space.freeBytes) })}
				</span>
			)}
			{children && <div className={styles.switcher}>{children}</div>}
			<div className={styles.srOnly} role="status" aria-live="polite">
				{announcement}
			</div>
		</div>
	);
}
