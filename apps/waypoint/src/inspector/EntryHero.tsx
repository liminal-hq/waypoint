// The hero at the head of the Properties tab: the entry's picture, its name and its kind and size
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { FileIcon } from '../browse/FileIcon';
import { useEffect } from 'react';
import { formatSize } from '../browse/format';
import { t } from '../i18n/messages';
import { Thumbnail } from '../thumbnails/Thumbnail';
import { useEntryThumbnailLoader } from '../thumbnails/ThumbnailsContext';
import { entryThumbKey, wantsThumbnail } from '../thumbnails/thumbnailModel';
import { kindMessage } from './inspectorModel';
import styles from './EntryHero.module.css';

/**
 * A rounded picture block over a centred name, with the kind and size beneath. It asks for the
 * same large thumbnail the Preview tab does, so switching tabs costs nothing, and the picture is
 * decorative: the name under it is what is read out.
 */
export function EntryHero({
	handle,
	entry,
	size,
	folder,
}: {
	handle: ListingHandle;
	entry: Entry;
	/** The size to state; `null` where it is not known. */
	size: number | null;
	folder: boolean;
}) {
	const loader = useEntryThumbnailLoader(handle, 'large');
	const thumbKey = wantsThumbnail(entry) ? entryThumbKey(entry) : null;
	useEffect(() => {
		if (loader && thumbKey !== null) loader.want([{ key: thumbKey, id: entry.id }]);
	}, [loader, thumbKey, entry.id]);

	return (
		<div className={styles.hero} data-hero="">
			<div className={styles.stage}>
				<Thumbnail
					loader={loader}
					thumbKey={thumbKey}
					group={entry.group}
					className={styles.thumbnail}
					iconClassName={styles.icon}
				/>
			</div>
			<p className={styles.title} data-selectable="">
				{entry.name}
			</p>
			<p className={styles.subtitle}>
				{t(kindMessage(entry.kind, entry.group))}
				{!folder && size !== null && <> · {formatSize(size)}</>}
			</p>
		</div>
	);
}

/** The same block for the folder being shown, when nothing in it is selected. */
export function FolderHero({ name, facts }: { name: string; facts: string }) {
	return (
		<div className={styles.hero} data-hero="">
			<div className={styles.stage}>
				<FileIcon group="folder" className={styles.folderIcon} />
			</div>
			<p className={styles.title} data-selectable="">
				{name}
			</p>
			<p className={styles.subtitle}>{facts}</p>
		</div>
	);
}
