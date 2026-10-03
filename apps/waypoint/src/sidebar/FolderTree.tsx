// The Folders section: a lazy tree that follows the active tab's folder, with the WAI-ARIA tree keyboard
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import {
	useCallback,
	type CSSProperties,
	useEffect,
	useMemo,
	useRef,
	useState,
	useSyncExternalStore,
	type KeyboardEvent,
} from 'react';
import { TypeAheadBuffer } from '../browse/typeAhead';
import { useVfsClient } from '../browse/VfsClientContext';
import { FileIcon } from '../browse/FileIcon';
import { inlineKey, isRtl } from '../i18n/direction';
import { tf, t } from '../i18n/messages';
import { ChevronRightSmallIcon } from '../icons/AppIcons';
import { useLocationInfo } from '../nav/locationInfo';
import {
	findRowByPrefix,
	flattenTree,
	folderRows,
	moveInTree,
	type TreeKey,
	type TreeRow,
} from './treeRows';
import { FolderTreeModel, type FolderChild } from './folderTreeModel';
import { itemGestures, type ItemActions } from './itemGestures';
import { useSidebarStore, useSidebarState } from './sidebarStore';
import styles from './Sidebar.module.css';

const NAVIGATION_KEYS = new Set<string>([
	'ArrowDown',
	'ArrowUp',
	'ArrowRight',
	'ArrowLeft',
	'Home',
	'End',
]);

interface FolderTreeProps {
	/** The active tab's folder, which the tree expands to and marks. */
	location: Location | undefined;
	/** Whether hidden folders show, which follows the tab's hidden-files choice. */
	showHidden: boolean;
	actions: ItemActions;
}

export function FolderTree({ location, showHidden, actions }: FolderTreeProps) {
	const client = useVfsClient();
	const store = useSidebarStore();
	const expanded = useSidebarState((state) => state.expanded);
	const info = useLocationInfo(client, location);
	const [model] = useState(() => new FolderTreeModel(client));
	useSyncExternalStore(model.subscribe, model.getVersion);
	useEffect(() => () => model.dispose(), [model]);

	const currentUri = location?.uri;
	const segments = info?.segments;
	// Follow the active tab: open the path down to its folder. Each navigation does this once, so
	// a branch the person then closes stays closed until the next navigation.
	const followed = useRef<string | null>(null);
	useEffect(() => {
		if (
			!segments ||
			segments.length === 0 ||
			segments[segments.length - 1]!.location.uri !== currentUri
		) {
			return;
		}
		if (followed.current === currentUri) return;
		followed.current = currentUri ?? null;
		const ancestors = segments.slice(0, Math.max(1, segments.length - 1));
		store.getState().expandAll(ancestors.map((segment) => segment.location.uri));
	}, [segments, currentUri, store]);

	const root: FolderChild | null = segments?.[0]
		? { name: segments[0].label, location: segments[0].location }
		: null;
	const entries = useMemo(
		() => (root ? flattenTree(root, expanded, (uri) => model.childrenOf(uri)) : []),
		// `model.getVersion()` changes whenever a node's children do.
		[root?.location.uri, root?.name, expanded, model.getVersion()],
	);
	const rows = useMemo(() => folderRows(entries), [entries]);

	const wantedKey = rows
		.filter((row) => row.expanded)
		.map((row) => row.key)
		.join('\n');
	useEffect(() => {
		model.sync(
			rows.filter((row) => row.expanded).map((row) => row.location),
			showHidden,
		);
		// `wantedKey` stands for the rows' expanded locations.
	}, [model, wantedKey, showHidden]);

	// Roving focus: one row is in the tab order, the current folder's unless focus went elsewhere.
	const [focusKey, setFocusKey] = useState<string | null>(null);
	const tabStop =
		(focusKey && rows.some((row) => row.key === focusKey) ? focusKey : null) ??
		rows.find((row) => row.key === currentUri)?.key ??
		rows[0]?.key ??
		null;
	const elements = useRef(new Map<string, HTMLElement>());
	const moveFocus = useCallback((key: string) => {
		setFocusKey(key);
		elements.current.get(key)?.focus();
	}, []);

	// Keep the current folder in view as the tab navigates.
	useEffect(() => {
		if (currentUri) elements.current.get(currentUri)?.scrollIntoView?.({ block: 'nearest' });
	}, [currentUri, rows.length]);

	const typeAhead = useRef(new TypeAheadBuffer());
	const onKeyDown = (event: KeyboardEvent<HTMLElement>) => {
		if (event.defaultPrevented || event.ctrlKey || event.metaKey || event.altKey) return;
		const from = (event.target as HTMLElement).closest<HTMLElement>('[role="treeitem"]')?.dataset
			.key;
		if (!from) return;
		if (NAVIGATION_KEYS.has(event.key)) {
			event.preventDefault();
			// Right opens a folder and Left closes it, so in a right-to-left tree the keys swap.
			const key = inlineKey(event.key, isRtl(event.currentTarget)) as TreeKey;
			const move = moveInTree(rows, from, key);
			if (move.expand) store.getState().setExpanded(move.expand.key, move.expand.expanded);
			if (move.focus) moveFocus(move.focus);
		} else if (event.key === 'Enter') {
			event.preventDefault();
			const row = rows.find((candidate) => candidate.key === from);
			if (row) actions.open(row.location);
		} else if (event.key.length === 1 && event.key !== ' ' && !event.nativeEvent.isComposing) {
			const target = findRowByPrefix(rows, from, typeAhead.current.push(event.key));
			if (target) {
				event.preventDefault();
				moveFocus(target);
			}
		}
	};

	const renderRow = (row: TreeRow) => {
		const gestures = itemGestures(actions, {
			kind: 'folder',
			location: row.location,
			label: row.name,
		});
		return (
			<div
				key={row.key}
				ref={(element) => {
					if (element) elements.current.set(row.key, element);
					else elements.current.delete(row.key);
				}}
				role="treeitem"
				data-key={row.key}
				aria-level={row.level}
				aria-setsize={row.siblings}
				aria-posinset={row.position}
				aria-expanded={row.expandable ? row.expanded : undefined}
				aria-busy={row.loading || undefined}
				aria-current={row.key === currentUri ? 'page' : undefined}
				tabIndex={row.key === tabStop ? 0 : -1}
				className={styles.treeItem}
				// The indent is the only dynamic value; the level steps it.
				style={{ '--wp-tree-level': row.level - 1 } as CSSProperties}
				onFocus={() => setFocusKey(row.key)}
				{...gestures}
			>
				<span
					className={styles.twisty}
					data-expanded={row.expanded || undefined}
					data-leaf={!row.expandable || undefined}
					aria-hidden="true"
					onClick={(event) => {
						event.stopPropagation();
						if (row.expandable) store.getState().setExpanded(row.key, !row.expanded);
					}}
				>
					<ChevronRightSmallIcon />
				</span>
				<FileIcon group="folder" special={row.special} />
				<span className={styles.label}>{row.name}</span>
			</div>
		);
	};

	return (
		<div
			role="tree"
			aria-label={t('sidebar.folders.tree')}
			className={styles.tree}
			onKeyDown={onKeyDown}
		>
			{entries.map((entry) =>
				entry.kind === 'folder' ? (
					renderRow(entry)
				) : (
					<div
						key={entry.key}
						role="none"
						className={styles.note}
						style={{ '--wp-tree-level': entry.level - 1 } as CSSProperties}
					>
						{tf('sidebar.folders.more', { shown: entry.shown, total: entry.total })}
					</div>
				),
			)}
		</div>
	);
}
