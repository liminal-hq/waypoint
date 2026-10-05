// While an archive that has to be read from its start is opened, the view says why it takes a while
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useEffect, useState, type ReactNode } from 'react';
import styles from '../connections/RemoteState.module.css';
import { tf } from '../i18n/messages';
import { announce } from '../tabs/announcer';
import { useArchiveClient } from './ArchiveContext';
import { containerUri } from './archiveNames';
import { lockedName } from './lockModel';

/**
 * A compressed tar has no index, so listing one reads the whole file (D154). The provider says so
 * as it starts; the view that is opening that archive replaces its plain opening message with the
 * reason, in a status that is read out, so a long wait is never silent.
 */
export function ArchiveOpening({
	location,
	fallback,
}: {
	location: Location;
	fallback: ReactNode;
}) {
	const archives = useArchiveClient();
	const container = containerUri(location);
	const [slow, setSlow] = useState<{ name: string; format: string } | null>(null);
	useEffect(() => {
		if (!archives || container === null) return;
		return archives.onSlowListing((listing) => {
			if (listing.container.uri !== container) return;
			const notice = { name: lockedName(listing.container), format: listing.format };
			setSlow(notice);
			announce(tf('archive.slow', notice));
		});
	}, [archives, container]);
	if (!slow) return <>{fallback}</>;
	return (
		<div className={styles.state} role="status" data-state="slow-archive">
			<span className={styles.spinner} aria-hidden="true" />
			<p className={styles.detail}>{tf('archive.slow', slow)}</p>
		</div>
	);
}
