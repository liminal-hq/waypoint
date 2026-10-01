// The Places section: Home and the user folders as the file system plugin reports them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Place } from '@liminal-hq/waypoint-protocol/generated/Place';
import type { PlaceKind } from '@liminal-hq/waypoint-protocol/generated/PlaceKind';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { TrashInfo } from '@liminal-hq/waypoint-protocol/generated/TrashInfo';
import { FileIcon } from '../browse/FileIcon';
import { t, tn, type MessageId } from '../i18n/messages';
import { HomeIcon } from '../icons/AppIcons';
import { TrashIcon } from '../icons/MenuIcons';
import { itemGestures, type ItemActions } from './itemGestures';
import { ITEM_ATTRIBUTE, moveFocusInList } from './itemList';
import styles from './Sidebar.module.css';

export const LABELS: Record<PlaceKind, MessageId> = {
	home: 'sidebar.place.home',
	desktop: 'sidebar.place.desktop',
	documents: 'sidebar.place.documents',
	downloads: 'sidebar.place.downloads',
	pictures: 'sidebar.place.pictures',
	music: 'sidebar.place.music',
	videos: 'sidebar.place.videos',
	trash: 'sidebar.place.trash',
};

// The bundled set has no glyph per place, so each borrows the closest icon group.
const GROUPS: Record<Exclude<PlaceKind, 'home' | 'trash'>, IconGroup> = {
	desktop: 'folder',
	documents: 'document',
	downloads: 'folder',
	pictures: 'image',
	music: 'audio',
	videos: 'video',
};

interface PlaceListProps {
	places: readonly Place[];
	currentUri: string | undefined;
	actions: ItemActions;
	/** The Trash's state, which the Trash place shows as a count; `null` before it is known and without a Trash service. */
	trash?: TrashInfo | null;
}

export function PlaceList({ places, currentUri, actions, trash = null }: PlaceListProps) {
	return (
		<ul className={styles.list} onKeyDown={moveFocusInList}>
			{places.map((place) => {
				const label = t(LABELS[place.kind]);
				const isTrash = place.kind === 'trash';
				const unavailable = isTrash && trash !== null && !trash.available;
				const count = isTrash && trash?.available ? trash.count : 0;
				return (
					<li key={place.kind}>
						<button
							type="button"
							{...{ [ITEM_ATTRIBUTE]: '' }}
							className={styles.item}
							aria-current={place.location.uri === currentUri ? 'page' : undefined}
							data-unavailable={unavailable ? '' : undefined}
							title={unavailable ? (trash?.reason ?? t('sidebar.trash.unavailable')) : undefined}
							{...itemGestures(actions, {
								kind: isTrash ? 'trash' : 'place',
								location: place.location,
								label,
							})}
						>
							{place.kind === 'home' ? (
								<HomeIcon className={styles.itemIcon} />
							) : place.kind === 'trash' ? (
								<TrashIcon className={styles.itemIcon} />
							) : (
								<FileIcon group={GROUPS[place.kind]} />
							)}
							<span className={styles.label}>{label}</span>
							{count > 0 && (
								<>
									<span className={styles.badge} aria-hidden="true">
										{new Intl.NumberFormat().format(count)}
									</span>
									<span className={styles.srOnly}>{tn('sidebar.trash.count', count)}</span>
								</>
							)}
						</button>
					</li>
				);
			})}
		</ul>
	);
}
