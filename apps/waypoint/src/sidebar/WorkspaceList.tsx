// The Workspaces section: named folder sets, one of which can stand in for the Favourites
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuPosition } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Workspace } from '@liminal-hq/waypoint-protocol/generated/Workspace';
import type { WorkspaceId } from '@liminal-hq/waypoint-protocol/generated/WorkspaceId';
import { useEffect, useRef, type KeyboardEvent, type MouseEvent } from 'react';
import { t } from '../i18n/messages';
import { RenameField } from './FavouriteList';
import { ITEM_ATTRIBUTE, moveFocusInList } from './itemList';
import styles from './Sidebar.module.css';

/** What a workspace's context menu needs to know about where it opened. */
export interface WorkspaceMenuRequest {
	workspace: Workspace;
	position: MenuPosition;
	/** Opened from the keyboard, so focus goes into the menu. */
	keyboard: boolean;
	returnFocus: HTMLElement | null;
}

interface WorkspaceListProps {
	workspaces: readonly Workspace[];
	/** The window's active workspace; `null` when the Favourites show the bookmarks. */
	active: WorkspaceId | null;
	renaming: WorkspaceId | null;
	/** `null` chooses the bookmarks again. */
	onChoose(workspace: WorkspaceId | null): void;
	onOpenMenu(request: WorkspaceMenuRequest): void;
	onRenameStart(workspace: WorkspaceId): void;
	/** Called with the typed name; nothing is called on cancel. */
	onRenameCommit(workspace: Workspace, name: string): void;
	onRenameCancel(): void;
	onDelete(workspace: Workspace): void;
}

const NONE = 'none';

/**
 * One choice among the bookmarks ("None") and each workspace, marked with `aria-current` on the
 * one in use. Arrow keys, Home and End move focus like the other lists; F2 renames, Delete
 * deletes, and the Menu key or Shift+F10 opens the item's menu.
 */
export function WorkspaceList({
	workspaces,
	active,
	renaming,
	onChoose,
	onOpenMenu,
	onRenameStart,
	onRenameCommit,
	onRenameCancel,
	onDelete,
}: WorkspaceListProps) {
	const list = useRef<HTMLUListElement | null>(null);
	// Where focus goes once the list has changed under it: after a rename ends, or an item is deleted.
	const refocus = useRef<string | null>(null);
	useEffect(() => {
		if (renaming !== null || refocus.current === null) return;
		const key = refocus.current;
		refocus.current = null;
		list.current?.querySelector<HTMLElement>(`[data-workspace="${CSS.escape(key)}"]`)?.focus();
	}, [renaming, workspaces]);

	if (workspaces.length === 0) {
		return <p className={styles.empty}>{t('sidebar.workspaces.empty')}</p>;
	}

	const openMenu = (workspace: Workspace, event: MouseEvent | KeyboardEvent, keyboard: boolean) => {
		const element = event.currentTarget as HTMLElement;
		const rect = element.getBoundingClientRect();
		onOpenMenu({
			workspace,
			position: keyboard
				? { x: rect.left, y: rect.bottom }
				: { x: (event as MouseEvent).clientX, y: (event as MouseEvent).clientY },
			keyboard,
			returnFocus: element,
		});
	};

	const onKeyDown = (event: KeyboardEvent<HTMLElement>, workspace: Workspace, index: number) => {
		if (event.key === 'ContextMenu' || (event.key === 'F10' && event.shiftKey)) {
			event.preventDefault();
			openMenu(workspace, event, true);
		} else if (event.key === 'F2' && !event.altKey && !event.ctrlKey && !event.metaKey) {
			event.preventDefault();
			refocus.current = String(workspace.id);
			onRenameStart(workspace.id);
		} else if (event.key === 'Delete' && !event.altKey && !event.ctrlKey && !event.metaKey) {
			event.preventDefault();
			// The row is about to go, so focus moves to its neighbour above (or None).
			refocus.current = String(workspaces[index - 1]?.id ?? NONE);
			onDelete(workspace);
		}
	};

	return (
		<ul
			ref={list}
			className={styles.list}
			aria-label={t('sidebar.workspaces.list')}
			onKeyDown={moveFocusInList}
		>
			<li>
				<button
					type="button"
					{...{ [ITEM_ATTRIBUTE]: '' }}
					data-workspace={NONE}
					className={styles.item}
					aria-current={active === null ? 'true' : undefined}
					onClick={() => onChoose(null)}
				>
					<span className={styles.label}>{t('sidebar.workspaces.none')}</span>
				</button>
			</li>
			{workspaces.map((workspace, index) => {
				if (renaming === workspace.id) {
					return (
						<li key={workspace.id}>
							<RenameField
								label={workspace.name}
								ariaLabel={t('sidebar.workspaces.rename.label')}
								onCommit={(name, viaKeyboard) => {
									if (viaKeyboard) refocus.current = String(workspace.id);
									if (name === null) onRenameCancel();
									else onRenameCommit(workspace, name);
								}}
								onCancel={() => {
									refocus.current = String(workspace.id);
									onRenameCancel();
								}}
							/>
						</li>
					);
				}
				return (
					<li key={workspace.id}>
						<button
							type="button"
							{...{ [ITEM_ATTRIBUTE]: '' }}
							data-workspace={workspace.id}
							className={styles.item}
							aria-current={active === workspace.id ? 'true' : undefined}
							onClick={() => onChoose(workspace.id)}
							onContextMenu={(event) => {
								event.preventDefault();
								openMenu(workspace, event, false);
							}}
							onKeyDown={(event) => onKeyDown(event, workspace, index)}
						>
							<span className={styles.label}>{workspace.name}</span>
						</button>
					</li>
				);
			})}
		</ul>
	);
}
