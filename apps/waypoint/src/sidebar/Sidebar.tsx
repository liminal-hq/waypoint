// The sidebar: a Places / Folders switch over Places and Favourites, or the Folders tree on its own, in one navigation landmark
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useCallback, useId, useMemo, useState, type KeyboardEvent, type ReactNode } from 'react';
import { t, tf, type MessageId } from '../i18n/messages';
import { ChevronRightSmallIcon } from '../icons/AppIcons';
import { useNavigation } from '../nav/useNavigation';
import { useTabActions } from '../tabs/tabActions';
import { useWindowActions } from '../tabs/windowActions';
import { FavouriteList } from './FavouriteList';
import { FolderTree } from './FolderTree';
import type { ItemActions, ItemMenuRequest } from './itemGestures';
import { PlaceList } from './PlaceList';
import { usePlacesClient } from './PlacesClientContext';
import { SidebarMenu } from './SidebarMenu';
import {
	useSidebarState,
	useSidebarStore,
	type SidebarSection,
	type SidebarView,
} from './sidebarStore';
import { usePlaces } from './usePlaces';
import styles from './Sidebar.module.css';

const TITLES: Record<SidebarSection, MessageId> = {
	places: 'sidebar.section.places',
	favourites: 'sidebar.section.favourites',
};

interface SectionProps {
	section: SidebarSection;
	children: ReactNode;
}

/** A labelled group whose heading is a button that collapses it; a collapsed body is not in the DOM. */
function Section({ section, children }: SectionProps) {
	const store = useSidebarStore();
	const collapsed = useSidebarState((state) => state.collapsed[section]);
	const id = useId();
	return (
		<div role="group" aria-labelledby={`${id}-title`} className={styles.section}>
			<h2 id={`${id}-title`} className={styles.heading}>
				<button
					type="button"
					className={styles.headingButton}
					aria-expanded={!collapsed}
					aria-controls={`${id}-body`}
					onClick={() => store.getState().toggleSection(section)}
				>
					<ChevronRightSmallIcon className={styles.headingChevron} />
					{t(TITLES[section])}
				</button>
			</h2>
			{!collapsed && <div id={`${id}-body`}>{children}</div>}
		</div>
	);
}

const VIEWS: readonly SidebarView[] = ['places', 'folders'];
const VIEW_TITLES: Record<SidebarView, MessageId> = {
	places: 'sidebar.view.places',
	folders: 'sidebar.view.folders',
};

/** The Places / Folders switch: two tabs with roving focus, Left and Right moving between them. */
function ViewSwitch({ panelId }: { panelId: string }) {
	const store = useSidebarStore();
	const view = useSidebarState((state) => state.view);
	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
		event.preventDefault();
		const next: SidebarView = view === 'places' ? 'folders' : 'places';
		store.getState().setView(next);
		event.currentTarget.querySelector<HTMLElement>(`[data-view="${next}"]`)?.focus();
	};
	return (
		<div
			role="tablist"
			aria-label={t('sidebar.view.label')}
			className={styles.viewSwitch}
			onKeyDown={onKeyDown}
		>
			{VIEWS.map((id) => (
				<button
					key={id}
					type="button"
					role="tab"
					data-view={id}
					aria-selected={view === id}
					aria-controls={panelId}
					tabIndex={view === id ? 0 : -1}
					className={styles.viewTab}
					onClick={() => store.getState().setView(id)}
				>
					{t(VIEW_TITLES[id])}
				</button>
			))}
		</div>
	);
}

interface SidebarProps {
	/** Whether the Folders tree lists hidden folders (the window's hidden-files choice). */
	showHidden: boolean;
	/** Reports something that could not be done, for the status bar. */
	onNotice(message: string): void;
}

export function Sidebar({ showHidden, onNotice }: SidebarProps) {
	const placesClient = usePlacesClient();
	const places = usePlaces(placesClient);
	const navigation = useNavigation();
	const { openInBackground } = useTabActions();
	const { openInNewWindow } = useWindowActions();
	const openInNewTab = useCallback(
		(location: Location, inNewWindow = false) =>
			void (inNewWindow ? openInNewWindow(location) : openInBackground(location)),
		[openInNewWindow, openInBackground],
	);
	const { goTo } = navigation;
	const currentLocation = navigation.tab?.location;
	const currentUri = currentLocation?.uri;

	const view = useSidebarState((state) => state.view);
	const panelId = useId();
	const [menu, setMenu] = useState<ItemMenuRequest | null>(null);
	const [renaming, setRenaming] = useState<string | null>(null);
	const [announcement, setAnnouncement] = useState('');

	const actions: ItemActions = useMemo(
		() => ({ open: goTo, openInNewTab, openMenu: setMenu }),
		[goTo, openInNewTab],
	);

	const favourites = useMemo(() => places?.favourites ?? [], [places]);
	const fail = useCallback(
		(error: unknown) => {
			console.warn('could not change the favourites', error);
			onNotice(t('sidebar.favourites.failed'));
		},
		[onNotice],
	);
	const move = useCallback(
		(location: Location, to: number) => {
			const name = favourites.find((favourite) => favourite.location.uri === location.uri)?.label;
			placesClient.moveFavourite(location, to).then(
				(next) =>
					setAnnouncement(
						tf('sidebar.moved', {
							name: name ?? location.display,
							position: to + 1,
							count: next.favourites.length,
						}),
					),
				fail,
			);
		},
		[placesClient, favourites, fail],
	);

	const menuActions = useMemo(
		() => ({
			open: goTo,
			openInNewTab,
			startRename: (location: Location) => setRenaming(location.uri),
			remove: (location: Location) => void placesClient.removeFavourite(location).catch(fail),
			move: (location: Location, by: number) => {
				const index = favourites.findIndex((favourite) => favourite.location.uri === location.uri);
				if (index >= 0) move(location, index + by);
			},
			add: (location: Location) => void placesClient.addFavourite(location).catch(fail),
		}),
		[goTo, openInNewTab, placesClient, favourites, move, fail],
	);

	const menuIndex = menu
		? favourites.findIndex((favourite) => favourite.location.uri === menu.location.uri)
		: -1;

	return (
		<nav className={styles.sidebar} aria-label={t('sidebar.label')}>
			<ViewSwitch panelId={panelId} />
			<div id={panelId} role="tabpanel" aria-label={t(VIEW_TITLES[view])} className={styles.panel}>
				{view === 'places' ? (
					<>
						<Section section="places">
							<PlaceList places={places?.places ?? []} currentUri={currentUri} actions={actions} />
						</Section>
						<Section section="favourites">
							{places && (
								<FavouriteList
									favourites={favourites}
									currentUri={currentUri}
									actions={actions}
									renaming={renaming}
									onRenameStart={(location) => setRenaming(location.uri)}
									onRenameCommit={(location, label) => {
										setRenaming(null);
										placesClient.renameFavourite(location, label).catch(fail);
									}}
									onRenameCancel={() => setRenaming(null)}
									onMove={move}
								/>
							)}
						</Section>
					</>
				) : (
					<FolderTree location={currentLocation} showHidden={showHidden} actions={actions} />
				)}
			</div>
			<div role="status" className={styles.srOnly}>
				{announcement}
			</div>
			{menu && (
				<SidebarMenu
					request={menu}
					favouritePosition={menuIndex >= 0 ? { index: menuIndex, count: favourites.length } : null}
					pinned={menuIndex >= 0}
					actions={menuActions}
					onClose={() => setMenu(null)}
				/>
			)}
		</nav>
	);
}
