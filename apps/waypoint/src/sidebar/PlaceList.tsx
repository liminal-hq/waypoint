// The Places section: Home and the user folders as the file system plugin reports them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Place } from '@liminal-hq/waypoint-protocol/generated/Place';
import type { PlaceKind } from '@liminal-hq/waypoint-protocol/generated/PlaceKind';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import { FileIcon } from '../browse/FileIcon';
import { t, type MessageId } from '../i18n/messages';
import { HomeIcon } from '../icons/AppIcons';
import { itemGestures, type ItemActions } from './itemGestures';
import { ITEM_ATTRIBUTE, moveFocusInList } from './itemList';
import styles from './Sidebar.module.css';

const LABELS: Record<PlaceKind, MessageId> = {
	home: 'sidebar.place.home',
	desktop: 'sidebar.place.desktop',
	documents: 'sidebar.place.documents',
	downloads: 'sidebar.place.downloads',
	pictures: 'sidebar.place.pictures',
	music: 'sidebar.place.music',
	videos: 'sidebar.place.videos',
};

// The bundled set has no glyph per place, so each borrows the closest icon group.
const GROUPS: Record<Exclude<PlaceKind, 'home'>, IconGroup> = {
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
}

export function PlaceList({ places, currentUri, actions }: PlaceListProps) {
	return (
		<ul className={styles.list} onKeyDown={moveFocusInList}>
			{places.map((place) => {
				const label = t(LABELS[place.kind]);
				return (
					<li key={place.kind}>
						<button
							type="button"
							{...{ [ITEM_ATTRIBUTE]: '' }}
							className={styles.item}
							aria-current={place.location.uri === currentUri ? 'page' : undefined}
							{...itemGestures(actions, { kind: 'place', location: place.location, label })}
						>
							{place.kind === 'home' ? (
								<HomeIcon className={styles.itemIcon} />
							) : (
								<FileIcon group={GROUPS[place.kind]} />
							)}
							<span className={styles.label}>{label}</span>
						</button>
					</li>
				);
			})}
		</ul>
	);
}
