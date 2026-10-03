// The Preview tab for several selected entries: a stacked-folders block, how many, and their size
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { formatSize } from '../browse/format';
import { FileIcon } from '../browse/FileIcon';
import type { ListingSession } from '../browse/useListingSession';
import { useOptionalVfsClient } from '../browse/VfsClientContext';
import { tn } from '../i18n/messages';
import type { VfsClient } from '../services/vfsClient';
import { useSelectionSummary } from '../status/useSelectionSummary';
import styles from './InspectorPanel.module.css';

export function ManySummaryBlock({
	count,
	session,
}: {
	count: number;
	session: ListingSession | null;
}) {
	const vfs = useOptionalVfsClient();
	return (
		<div className={styles.summary}>
			<div className={styles.heroStage}>
				<FileIcon group="folder" size={64} className={styles.bigIcon} />
			</div>
			<p className={styles.summaryTitle}>{tn('browse.selection', count)}</p>
			{vfs && session && <ManySize client={vfs} session={session} />}
		</div>
	);
}

function ManySize({ client, session }: { client: VfsClient; session: ListingSession }) {
	const summary = useSelectionSummary(client, session);
	return summary.size === null ? null : (
		<p className={styles.summaryFacts} data-pending={summary.pending ? '' : undefined}>
			{formatSize(summary.size)}
		</p>
	);
}
