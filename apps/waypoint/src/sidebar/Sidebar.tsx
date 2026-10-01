// The sidebar: a Places / Folders switch over Places and Favourites, or the Folders tree on its own, in one navigation landmark
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { Workspace } from '@liminal-hq/waypoint-protocol/generated/Workspace';
import type { WorkspaceId } from '@liminal-hq/waypoint-protocol/generated/WorkspaceId';
import { useCallback, useId, useMemo, useState, type KeyboardEvent, type ReactNode } from 'react';
import { t, tf, type MessageId } from '../i18n/messages';
import { ChevronRightSmallIcon } from '../icons/AppIcons';
import { useNavigation } from '../nav/useNavigation';
import { isWorkspaceNameTaken } from '../services/tabsApi';
import { useTabActions } from '../tabs/tabActions';
import { useWindowActions } from '../tabs/windowActions';
import { useTabsApi, useTabsSnapshot } from '../tabs/TabsContext';
import { FavouriteList } from './FavouriteList';
import { bookmarksSource, workspaceSource } from './favouritesSource';
import { FolderTree } from './FolderTree';
import type { ItemActions, ItemMenuRequest } from './itemGestures';
import { PlaceList } from './PlaceList';
import { usePlacesClient } from './PlacesClientContext';
import { SidebarMenu } from './SidebarMenu';
import { WorkspaceList, type WorkspaceMenuRequest } from './WorkspaceList';
import { WorkspaceMenu } from './WorkspaceMenu';
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
	workspaces: 'sidebar.section.workspaces',
};

interface SectionProps {
	section: SidebarSection;
	/** Overrides the section's own name, for a heading that says what it currently shows. */
	title?: string;
	children: ReactNode;
}

/** A labelled group whose heading is a button that collapses it; a collapsed body is not in the DOM. */
function Section({ section, title, children }: SectionProps) {
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
					{title ?? t(TITLES[section])}
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

	const api = useTabsApi();
	const snapshot = useTabsSnapshot();
	const workspaces = useMemo(() => snapshot?.workspaces ?? [], [snapshot?.workspaces]);
	const activeId = snapshot?.workspace ?? null;
	const activeWorkspace = workspaces.find((workspace) => workspace.id === activeId) ?? null;
	const [workspaceMenu, setWorkspaceMenu] = useState<WorkspaceMenuRequest | null>(null);
	const [renamingWorkspace, setRenamingWorkspace] = useState<WorkspaceId | null>(null);

	// The Favourites section shows the window's active workspace when it has one, and the shared
	// bookmarks otherwise; every edit below goes through whichever source is showing.
	const bookmarks = useMemo(() => places?.favourites ?? [], [places]);
	const source = useMemo(
		() =>
			activeWorkspace
				? workspaceSource(api, activeWorkspace)
				: bookmarksSource(placesClient, bookmarks),
		[api, activeWorkspace, placesClient, bookmarks],
	);
	const favourites = source.favourites;
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
			source.move(location, to).then(
				(count) =>
					setAnnouncement(
						tf('sidebar.moved', {
							name: name ?? location.display,
							position: Math.min(to, count - 1) + 1,
							count,
						}),
					),
				fail,
			);
		},
		[source, favourites, fail],
	);

	const menuActions = useMemo(
		() => ({
			open: goTo,
			openInNewTab,
			startRename: (location: Location) => setRenaming(location.uri),
			remove: (location: Location) => void source.remove(location).catch(fail),
			move: (location: Location, by: number) => {
				const index = favourites.findIndex((favourite) => favourite.location.uri === location.uri);
				if (index >= 0) move(location, index + by);
			},
			add: (location: Location) => void source.add(location).catch(fail),
		}),
		[goTo, openInNewTab, source, favourites, move, fail],
	);

	const failWorkspace = useCallback(
		(error: unknown) => {
			console.warn('could not change the workspace', error);
			onNotice(t('sidebar.workspaces.failed'));
		},
		[onNotice],
	);
	const workspaceActions = useMemo(
		() => ({
			choose: (id: WorkspaceId | null) => {
				if (id === activeId) return;
				const name = workspaces.find((workspace) => workspace.id === id)?.name ?? '';
				api
					.setActiveWorkspace(id)
					.then(
						() =>
							setAnnouncement(
								id === null
									? t('workspaces.announce.cleared')
									: tf('workspaces.announce.switched', { name }),
							),
						failWorkspace,
					);
			},
			rename: (workspace: Workspace, name: string) => {
				setRenamingWorkspace(null);
				if (name === workspace.name) return;
				api.renameWorkspace(workspace.id, name).then(
					() => setAnnouncement(tf('workspaces.announce.renamed', { name })),
					(error: unknown) => {
						if (!isWorkspaceNameTaken(error)) return failWorkspace(error);
						// Two workspaces never share a name: ask again.
						setAnnouncement(tf('workspaces.announce.nameTaken', { name }));
						setRenamingWorkspace(workspace.id);
					},
				);
			},
			remove: (workspace: Workspace) =>
				void api
					.deleteWorkspace(workspace.id)
					.then(
						() => setAnnouncement(tf('workspaces.announce.deleted', { name: workspace.name })),
						failWorkspace,
					),
			// Each folder opens right after the one before it, starting beside the active tab, and the
			// first becomes the current tab so the person lands in the workspace.
			openAll: (workspace: Workspace) => {
				if (workspace.locations.length === 0) {
					setAnnouncement(tf('sidebar.workspaces.noFolders', { name: workspace.name }));
					return;
				}
				void (async () => {
					// The active tab is read once, now (not as it was at render), and each folder opens
					// after the tab the one before it made, so a change of tab midway does not scatter them.
					let after = (await api.getSnapshot()).active ?? undefined;
					const total = workspace.locations.length;
					let opened = 0;
					try {
						for (const location of workspace.locations) {
							after = await api.openTab(location, { after, activate: opened === 0 });
							opened++;
						}
					} catch (error) {
						if (opened === 0) throw error;
						console.warn('could not open every folder of the workspace', error);
						const text = tf('sidebar.workspaces.openedSome', {
							opened,
							total,
							name: workspace.name,
						});
						setAnnouncement(text);
						onNotice(text);
						return;
					}
					setAnnouncement(
						tf(
							total === 1
								? 'sidebar.workspaces.openedAll.one'
								: 'sidebar.workspaces.openedAll.other',
							{ count: total, name: workspace.name },
						),
					);
				})().catch(failWorkspace);
			},
		}),
		[api, activeId, workspaces, failWorkspace, onNotice],
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
						<Section
							section="favourites"
							title={
								activeWorkspace
									? tf('sidebar.section.favouritesIn', { name: activeWorkspace.name })
									: undefined
							}
						>
							{(places || activeWorkspace) && (
								<FavouriteList
									favourites={favourites}
									currentUri={currentUri}
									actions={actions}
									renaming={renaming}
									canRename={source.canRename}
									onRenameStart={(location) => setRenaming(location.uri)}
									onRenameCommit={(location, label) => {
										setRenaming(null);
										source.rename(location, label).catch(fail);
									}}
									onRenameCancel={() => setRenaming(null)}
									onMove={move}
								/>
							)}
						</Section>
						<Section section="workspaces">
							<WorkspaceList
								workspaces={workspaces}
								active={activeWorkspace?.id ?? null}
								renaming={renamingWorkspace}
								onChoose={workspaceActions.choose}
								onOpenMenu={setWorkspaceMenu}
								onRenameStart={setRenamingWorkspace}
								onRenameCommit={workspaceActions.rename}
								onRenameCancel={() => setRenamingWorkspace(null)}
								onDelete={workspaceActions.remove}
							/>
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
					canRename={source.canRename}
					actions={menuActions}
					onClose={() => setMenu(null)}
				/>
			)}
			{workspaceMenu && (
				<WorkspaceMenu
					request={workspaceMenu}
					actions={{
						openAll: workspaceActions.openAll,
						startRename: (workspace) => setRenamingWorkspace(workspace.id),
						remove: workspaceActions.remove,
					}}
					onClose={() => setWorkspaceMenu(null)}
				/>
			)}
		</nav>
	);
}
