// The Places section: Home and the user folders as the file system plugin reports them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Place } from '@liminal-hq/waypoint-protocol/generated/Place';
import type { PlaceKind } from '@liminal-hq/waypoint-protocol/generated/PlaceKind';
import type { SpecialFolder } from '@liminal-hq/waypoint-protocol/generated/SpecialFolder';
import type { TrashInfo } from '@liminal-hq/waypoint-protocol/generated/TrashInfo';
import { FileIcon } from '../browse/FileIcon';
import { useFileDragApi } from '../dnd/FileDragContext';
import type { GitBadge } from '../services/gitClient';
import { t, tn, type MessageId } from '../i18n/messages';
import { HomeIcon } from '../icons/AppIcons';
import { OverviewIcon } from '../overview/OverviewIcons';
import { TrashIcon } from '../icons/MenuIcons';
import { itemGestures, type ItemActions } from './itemGestures';
import { SidebarGitMark } from './SidebarGitMark';
import { ITEM_ATTRIBUTE, moveFocusInList } from './itemList';
import styles from './Sidebar.module.css';
import { formatLocale } from '../i18n/active';

export const LABELS: Record<PlaceKind, MessageId> = {
	overview: 'sidebar.place.overview',
	home: 'sidebar.place.home',
	desktop: 'sidebar.place.desktop',
	documents: 'sidebar.place.documents',
	downloads: 'sidebar.place.downloads',
	pictures: 'sidebar.place.pictures',
	music: 'sidebar.place.music',
	videos: 'sidebar.place.videos',
	trash: 'sidebar.place.trash',
};

// Each user folder is drawn as the standard folder it is (a folder with its own mark).
const SPECIAL: Record<Exclude<PlaceKind, 'overview' | 'home' | 'trash'>, SpecialFolder> = {
	desktop: 'desktop',
	documents: 'documents',
	downloads: 'downloads',
	pictures: 'pictures',
	music: 'music',
	videos: 'videos',
};

interface PlaceListProps {
	places: readonly Place[];
	currentUri: string | undefined;
	actions: ItemActions;
	/** The Trash's state, which the Trash place shows as a count; `null` before it is known and without a Trash service. */
	trash?: TrashInfo | null;
	/** What changed inside each place that is in a Git working tree, by `uri`; a place with nothing changed has none. */
	gitBadges?: ReadonlyMap<string, GitBadge>;
}

export function PlaceList({
	places,
	currentUri,
	actions,
	trash = null,
	gitBadges,
}: PlaceListProps) {
	const drag = useFileDragApi();
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
							data-drop-unavailable={unavailable ? '' : undefined}
							title={unavailable ? (trash?.reason ?? t('sidebar.trash.unavailable')) : undefined}
							{...itemGestures(
								actions,
								{
									kind: isTrash ? 'trash' : 'place',
									location: place.location,
									label,
								},
								// Overview is a page, so nothing is dropped on it, and neither it nor the Trash is dragged to a pane.
								{
									droppable: place.kind !== 'overview',
									drag: place.kind === 'overview' ? null : drag,
								},
							)}
						>
							{place.kind === 'overview' ? (
								<OverviewIcon className={styles.itemIcon} />
							) : place.kind === 'home' ? (
								<HomeIcon className={styles.itemIcon} />
							) : place.kind === 'trash' ? (
								<TrashIcon className={styles.itemIcon} />
							) : (
								<FileIcon group="folder" special={SPECIAL[place.kind]} />
							)}
							<span className={styles.label}>{label}</span>
							<SidebarGitMark badge={gitBadges?.get(place.location.uri)} />
							{count > 0 && (
								<>
									<span className={styles.badge} aria-hidden="true">
										{new Intl.NumberFormat(formatLocale()).format(count)}
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
