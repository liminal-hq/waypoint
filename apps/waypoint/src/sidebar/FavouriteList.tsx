// The Favourites section: the person's pinned folders, which can be renamed, removed and reordered
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Favourite } from '@liminal-hq/waypoint-protocol/generated/Favourite';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { FileIcon } from '../browse/FileIcon';
import { t } from '../i18n/messages';
import type { GitBadge } from '../services/gitClient';
import { itemGestures, type ItemActions } from './itemGestures';
import { SidebarGitMark } from './SidebarGitMark';
import { ITEM_ATTRIBUTE, moveFocusInList } from './itemList';
import styles from './Sidebar.module.css';

interface FavouriteListProps {
	favourites: readonly Favourite[];
	currentUri: string | undefined;
	actions: ItemActions;
	/** The `uri` of the favourite being renamed, if any. */
	renaming: string | null;
	onRenameStart(location: Location): void;
	/** `label` is `null` to clear it. Called when the person commits; nothing is called on cancel. */
	onRenameCommit(location: Location, label: string | null): void;
	onRenameCancel(): void;
	/** Moves a favourite to index `to` of the list. */
	onMove(location: Location, to: number): void;
	/** Whether F2 renames a row; the favourites of a workspace have no labels to edit. */
	canRename?: boolean;
	/** What changed inside each favourite that is in a Git working tree, by `uri`. */
	gitBadges?: ReadonlyMap<string, GitBadge> | undefined;
}

export function FavouriteList({
	favourites,
	currentUri,
	actions,
	renaming,
	onRenameStart,
	onRenameCommit,
	onRenameCancel,
	onMove,
	canRename = true,
	gitBadges,
}: FavouriteListProps) {
	const [dragging, setDragging] = useState<string | null>(null);
	const [dropOn, setDropOn] = useState<string | null>(null);
	const list = useRef<HTMLUListElement | null>(null);
	// After a rename ends with the keyboard, focus goes back to the row rather than to the page.
	const refocus = useRef<string | null>(null);
	useEffect(() => {
		if (renaming !== null || refocus.current === null) return;
		const uri = refocus.current;
		refocus.current = null;
		list.current?.querySelector<HTMLElement>(`[data-favourite="${CSS.escape(uri)}"]`)?.focus();
	}, [renaming]);

	if (favourites.length === 0) {
		return <p className={styles.empty}>{t('sidebar.favourites.empty')}</p>;
	}

	const onKeyDown = (event: KeyboardEvent<HTMLElement>, favourite: Favourite, index: number) => {
		if (event.altKey && (event.key === 'ArrowUp' || event.key === 'ArrowDown')) {
			event.preventDefault();
			const to = index + (event.key === 'ArrowDown' ? 1 : -1);
			if (to >= 0 && to < favourites.length) onMove(favourite.location, to);
		} else if (
			canRename &&
			event.key === 'F2' &&
			!event.altKey &&
			!event.ctrlKey &&
			!event.metaKey
		) {
			event.preventDefault();
			refocus.current = favourite.location.uri;
			onRenameStart(favourite.location);
		}
	};

	return (
		<ul ref={list} className={styles.list} onKeyDown={moveFocusInList}>
			{favourites.map((favourite, index) => {
				const uri = favourite.location.uri;
				const gestures = itemGestures(actions, {
					kind: 'favourite',
					location: favourite.location,
					label: favourite.label,
				});
				if (renaming === uri) {
					return (
						<li key={uri}>
							<RenameField
								label={favourite.label}
								ariaLabel={t('sidebar.rename.label')}
								onCommit={(label, viaKeyboard) => {
									if (viaKeyboard) refocus.current = uri;
									onRenameCommit(favourite.location, label);
								}}
								onCancel={() => {
									refocus.current = uri;
									onRenameCancel();
								}}
							/>
						</li>
					);
				}
				return (
					<li key={uri}>
						<button
							type="button"
							{...{ [ITEM_ATTRIBUTE]: '' }}
							data-favourite={uri}
							data-drop-target={dropOn === uri || undefined}
							data-dragging={dragging === uri || undefined}
							draggable
							className={styles.item}
							aria-current={uri === currentUri ? 'page' : undefined}
							{...gestures}
							onKeyDown={(event) => {
								gestures.onKeyDown(event);
								if (!event.defaultPrevented) onKeyDown(event, favourite, index);
							}}
							onDragStart={(event) => {
								event.dataTransfer.effectAllowed = 'move';
								event.dataTransfer.setData('text/plain', favourite.location.display);
								setDragging(uri);
							}}
							onDragOver={(event) => {
								if (dragging === null || dragging === uri) return;
								event.preventDefault();
								event.dataTransfer.dropEffect = 'move';
								setDropOn(uri);
							}}
							onDragLeave={() => setDropOn((current) => (current === uri ? null : current))}
							onDrop={(event) => {
								event.preventDefault();
								const from = favourites.find((candidate) => candidate.location.uri === dragging);
								setDragging(null);
								setDropOn(null);
								if (from && from.location.uri !== uri) onMove(from.location, index);
							}}
							onDragEnd={() => {
								setDragging(null);
								setDropOn(null);
							}}
						>
							<FileIcon group="folder" special={favourite.special} />
							<span className={styles.label}>{favourite.label}</span>
							<SidebarGitMark badge={gitBadges?.get(uri)} />
						</button>
					</li>
				);
			})}
		</ul>
	);
}

interface RenameFieldProps {
	label: string;
	/** What the field is called for assistive technology. */
	ariaLabel: string;
	onCommit(label: string | null, viaKeyboard: boolean): void;
	onCancel(): void;
}

/** Enter commits, Escape cancels, and leaving the field commits, like a file manager's inline rename. */
export function RenameField({ label, ariaLabel, onCommit, onCancel }: RenameFieldProps) {
	const input = useRef<HTMLInputElement | null>(null);
	const done = useRef(false);
	useEffect(() => {
		input.current?.focus();
		input.current?.select();
	}, []);
	const commit = (viaKeyboard: boolean) => {
		if (done.current) return;
		done.current = true;
		const value = input.current?.value.trim() ?? '';
		// Unchanged text is not a rename: it would pin the default label as a custom one.
		if (value === label) onCancel();
		else onCommit(value === '' ? null : value, viaKeyboard);
	};
	return (
		<input
			ref={input}
			className={styles.renameField}
			defaultValue={label}
			aria-label={ariaLabel}
			spellCheck={false}
			onKeyDown={(event) => {
				event.stopPropagation();
				if (event.key === 'Enter') {
					event.preventDefault();
					commit(true);
				} else if (event.key === 'Escape') {
					event.preventDefault();
					done.current = true;
					onCancel();
				}
			}}
			onBlur={() => commit(false)}
		/>
	);
}
