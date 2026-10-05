// The Shelf dock: references grouped by the folder they came from, laid out as a strip of tiles along the bottom, which can be selected, removed, opened and dragged out
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { ContextMenu } from '@liminal-hq/waypoint-chrome/ContextMenu';
import { ConfirmDialog } from '@liminal-hq/waypoint-chrome/Dialog/ConfirmDialog';
import type { ShelfItem } from '@liminal-hq/waypoint-protocol/generated/ShelfItem';
import {
	useCallback,
	useEffect,
	useMemo,
	useRef,
	useState,
	type CSSProperties,
	type KeyboardEvent,
	type MouseEvent,
	type PointerEvent,
} from 'react';
import { useStore } from 'zustand';
import { modifiersOf } from '../dnd/dropAction';
import { dropAttributes } from '../dnd/dropTargets';
import { useFileDragApi } from '../dnd/FileDragContext';
import { Thumbnail } from '../thumbnails/Thumbnail';
import { useLocationThumbnailLoader } from '../thumbnails/ThumbnailsContext';
import type { ThumbnailLoader } from '../thumbnails/thumbnailLoader';
import type { LocationRequest } from '../thumbnails/thumbnailsClient';
import { t, tf, tn } from '../i18n/messages';
import { ChevronRightSmallIcon, CloseSmallIcon } from '../icons/AppIcons';
import { MoreIcon } from '../icons/MenuIcons';
import { useShelfActions, useShelfPlacement } from './ShelfContext';
import { DockShelfIcon, UndockShelfIcon } from './ShelfIcons';
import {
	commonOrigin,
	groupItems,
	iconFor,
	itemKey,
	orderOf,
	rowOnAdjacentLine,
	visibleRows,
	type ItemState,
	type ShelfRow,
} from './shelfModel';
import { shelfItemMenuItems, shelfPanelMenuItems, type ShelfItemCommand } from './shelfMenu';
import { DEFAULT_HEIGHT, MAX_HEIGHT, MIN_HEIGHT, useShelfStore } from './shelfStore';
import styles from './ShelfPanel.module.css';

/** How far an arrow key moves the divider, and how far with Shift. */
const DIVIDER_STEP = 16;
const DIVIDER_BIG_STEP = 64;

/** Pictures asked for at once on the Shelf, which is not virtualised. */
const MAX_SHELF_THUMBNAILS = 120;

const rowDomId = (key: string) => `shelf-${key.replace(/[^a-zA-Z0-9_-]/g, '_')}`;

interface MenuRequest {
	kind: 'item' | 'panel';
	position: { x: number; y: number };
	keyboard: boolean;
}

/**
 * The Shelf, docked along the bottom of the window. The whole dock is a drop target
 * (`data-drop="shelf"`), and each tile is a drag source of references (`pressLocations`). It is a
 * tree with one level of groups laid out as a wrapping strip: a group's chip, then its tiles. The
 * keyboard moves along the strip (Left and Right; Up and Down between the lines when it wraps;
 * Home, End), Enter opens an item or toggles a group, Space and Ctrl+Space select, Shift extends,
 * Delete removes from the Shelf (never a file), Ctrl+C copies the items for a paste, and the Menu
 * key opens the item's menu.
 *
 * In the Shelf window (`layout="window"`) the same panel fills the window: it has no divider (the
 * window is resized instead) and no close button (the title bar closes the window, which docks the
 * Shelf), and its header offers Dock where the dock's offers Undock.
 */
export function ShelfPanel({ layout = 'dock' }: { layout?: 'dock' | 'window' }) {
	const store = useShelfStore();
	const placement = useShelfPlacement();
	const actions = useShelfActions();
	const drag = useFileDragApi();
	const items = useStore(store, (s) => s.items);
	const height = useStore(store, (s) => s.height);
	const selected = useStore(store, (s) => s.selected);
	const focus = useStore(store, (s) => s.focus);
	const collapsed = useStore(store, (s) => s.collapsed);
	const status = useStore(store, (s) => s.status);
	const focusRequests = useStore(store, (s) => s.focusRequests);
	const treeRef = useRef<HTMLDivElement>(null);
	const [menu, setMenu] = useState<MenuRequest | null>(null);
	const [confirmClear, setConfirmClear] = useState(false);

	const groups = useMemo(() => groupItems(items), [items]);
	const rows = useMemo(() => visibleRows(groups, collapsed), [groups, collapsed]);
	const order = useMemo(() => orderOf(rows), [rows]);
	const selectedItems = useMemo(
		() => items.filter((item) => selected.has(item.id)),
		[items, selected],
	);

	// Focus Shelf: the tree takes the keyboard, on its first row when nothing was focused.
	const lastRequest = useRef(focusRequests);
	useEffect(() => {
		if (focusRequests === lastRequest.current) return;
		lastRequest.current = focusRequests;
		if (store.getState().focus === null && rows[0]) store.getState().setFocus(rows[0].key);
		treeRef.current?.focus();
	}, [focusRequests, rows, store]);

	// Files on the Shelf show a picture where one can be made; the loader is null where none should be asked for.
	const thumbnails = useLocationThumbnailLoader('normal');
	useEffect(() => {
		if (!thumbnails) return;
		const wanted = rows.flatMap((row) =>
			row.kind === 'item' && status.get(row.item.location.uri) === 'file'
				? [{ key: row.item.location.uri, location: row.item.location }]
				: [],
		);
		thumbnails.want(wanted.slice(0, MAX_SHELF_THUMBNAILS));
	}, [thumbnails, rows, status]);

	const focusedRow = rows.find((row) => row.key === focus) ?? null;
	const moveFocus = useCallback(
		(to: ShelfRow | undefined, extend: boolean) => {
			if (!to) return;
			const state = store.getState();
			if (to.kind === 'item') {
				if (extend) state.select(to.item.id, 'range', order);
				else state.setFocus(to.key);
			} else state.setFocus(to.key);
			rowElement(to.key)?.scrollIntoView?.({ block: 'nearest' });
		},
		[order, store],
	);
	const rowElement = (key: string) => document.getElementById(rowDomId(key));

	/** The row Up or Down lands on: on the next line of the strip, nearest across. */
	const rowOnLine = (from: number, direction: 1 | -1): ShelfRow | undefined => {
		const boxes = rows.map((row) => {
			const box = rowElement(row.key)?.getBoundingClientRect();
			return { top: box?.top ?? 0, left: box?.left ?? 0, width: box?.width ?? 0 };
		});
		const to = rowOnAdjacentLine(boxes, from, direction);
		return to === null ? undefined : rows[to];
	};

	const itemMenuAt = (position: { x: number; y: number }, keyboard: boolean) =>
		setMenu({ kind: 'item', position, keyboard });

	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		if (event.defaultPrevented || event.nativeEvent.isComposing) return;
		const state = store.getState();
		const at = focusedRow ? rows.indexOf(focusedRow) : -1;
		const ctrl = event.ctrlKey || event.metaKey;
		const consume = () => event.preventDefault();
		switch (event.key) {
			case 'ArrowRight':
			case 'ArrowLeft': {
				consume();
				// Along the strip, the way the text runs: Right is toward the end in left-to-right text.
				const rtl = getComputedStyle(event.currentTarget).direction === 'rtl';
				const forward = (event.key === 'ArrowRight') !== rtl;
				const to = at < 0 ? 0 : forward ? Math.min(rows.length - 1, at + 1) : Math.max(0, at - 1);
				return moveFocus(rows[to], event.shiftKey);
			}
			case 'ArrowDown':
			case 'ArrowUp':
				consume();
				if (at < 0) return moveFocus(rows[0], event.shiftKey);
				return moveFocus(rowOnLine(at, event.key === 'ArrowDown' ? 1 : -1), event.shiftKey);
			case 'Home':
				consume();
				return moveFocus(rows[0], event.shiftKey);
			case 'End':
				consume();
				return moveFocus(rows[rows.length - 1], event.shiftKey);
			case 'Enter':
				consume();
				if (focusedRow?.kind === 'group') state.toggleGroup(focusedRow.group.origin.uri);
				else if (focusedRow?.kind === 'item') void actions?.open(focusedRow.item);
				return;
			case ' ':
				consume();
				if (focusedRow?.kind === 'item') {
					state.select(focusedRow.item.id, ctrl || event.shiftKey ? 'toggle' : 'only', order);
				} else if (focusedRow?.kind === 'group') state.toggleGroup(focusedRow.group.origin.uri);
				return;
			case 'Delete':
			case 'Backspace': {
				consume();
				const ids =
					selectedItems.length > 0
						? selectedItems.map((item) => item.id)
						: focusedRow?.kind === 'item'
							? [focusedRow.item.id]
							: [];
				// Land on the row after the ones removed, so a run of Deletes walks down the list.
				const next = rows
					.slice(at + 1)
					.find((row) => row.kind === 'item' && !ids.includes(row.item.id));
				if (next) state.setFocus(next.key);
				void actions?.remove(ids);
				return;
			}
			case 'a':
			case 'A':
				if (!ctrl) return;
				consume();
				return state.selectAll(order);
			case 'c':
			case 'C':
				if (!ctrl) return;
				consume();
				return void actions?.copyFiles(
					selectedItems.length > 0
						? selectedItems
						: focusedRow?.kind === 'item'
							? [focusedRow.item]
							: [],
				);
			case 'Escape':
				if (state.selected.size > 0) {
					consume();
					state.clearSelection();
				}
				return;
			case 'ContextMenu':
			case 'F10': {
				if (event.key === 'F10' && !event.shiftKey) return;
				consume();
				if (focusedRow?.kind === 'item') {
					if (!selected.has(focusedRow.item.id)) state.select(focusedRow.item.id, 'only', order);
					const box = rowElement(focusedRow.key)?.getBoundingClientRect();
					itemMenuAt({ x: box?.left ?? 0, y: (box?.bottom ?? 0) - 4 }, true);
				}
				return;
			}
			default:
		}
	};

	const onRowPointerDown = (event: PointerEvent<HTMLElement>, item: ShelfItem) => {
		if (event.button !== 0) return;
		const state = store.getState();
		treeRef.current?.focus({ preventScroll: true });
		if (event.ctrlKey || event.metaKey) return state.select(item.id, 'toggle', order);
		if (event.shiftKey) return state.select(item.id, 'range', order);
		const wasSelected = state.selected.has(item.id);
		if (!wasSelected) state.select(item.id, 'only', order);
		// What is dragged: the selection when the pressed row is part of it, else just this row. A
		// missing file has nothing to drag out.
		const carried = (
			wasSelected ? items.filter((i) => store.getState().selected.has(i.id)) : [item]
		)
			.filter((i) => store.getState().status.get(i.location.uri) !== 'missing')
			.sort((a, b) => order.indexOf(a.id) - order.indexOf(b.id));
		if (carried.length === 0 || !drag) return;
		drag.pressLocations({
			pointerId: event.pointerId,
			clientX: event.clientX,
			clientY: event.clientY,
			button: 0,
			element: treeRef.current,
			locations: carried.map((i) => i.location),
			name: carried.length === 1 ? (carried[0]?.name ?? null) : null,
			groups: carried
				.slice(0, 3)
				.map((i) => iconFor(store.getState().status.get(i.location.uri) ?? 'unknown')),
			icons: carried
				.slice(0, 3)
				.map((i) => ({ name: i.name, source: { location: i.location, modifiedMs: null } })),
			thumbnails: carried.slice(0, 3).map((i) => thumbnails?.urlOf(i.location.uri) ?? null),
			folder: commonOrigin(carried),
			modifiers: modifiersOf(event.nativeEvent),
		});
	};

	const onRowClick = (event: MouseEvent<HTMLElement>, item: ShelfItem) => {
		// The click that ends a drag is not a click.
		if (drag?.consumeClick()) return;
		if (event.ctrlKey || event.metaKey || event.shiftKey) return;
		store.getState().select(item.id, 'only', order);
	};

	const onRowContextMenu = (event: MouseEvent<HTMLElement>, item: ShelfItem) => {
		event.preventDefault();
		if (!store.getState().selected.has(item.id)) store.getState().select(item.id, 'only', order);
		itemMenuAt({ x: event.clientX, y: event.clientY }, false);
	};

	const runItemCommand = (command: ShelfItemCommand) => {
		if (!actions) return;
		const targets = selectedItems;
		const first = targets[0];
		switch (command) {
			case 'open':
				if (first) void actions.open(first);
				return;
			case 'reveal':
				if (first) void actions.reveal(first);
				return;
			case 'copyPath':
				return void actions.copyPath(targets);
			case 'copyFiles':
				return void actions.copyFiles(targets);
			case 'remove':
				return void actions.remove(targets.map((item) => item.id));
		}
	};

	const clear = () => {
		if (items.length > 1) setConfirmClear(true);
		else void actions?.clear();
	};

	return (
		<aside
			className={layout === 'window' ? `${styles.shelf} ${styles.inWindow}` : styles.shelf}
			aria-label={t('shelf.label')}
			style={
				layout === 'window'
					? undefined
					: ({ height, '--wp-shelf-height': `${height}px` } as CSSProperties)
			}
			data-shelf=""
			data-layout={layout}
			{...dropAttributes('shelf', 'shelf', t('shelf.title'))}
		>
			{layout === 'dock' && <ShelfDivider />}
			<div className={styles.header}>
				<h2 className={styles.title}>
					{t('shelf.title')}
					{items.length > 0 && <span className={styles.count}>{items.length}</span>}
				</h2>
				<button
					type="button"
					className={styles.iconButton}
					aria-label={t('shelf.options.label')}
					aria-haspopup="menu"
					aria-expanded={menu?.kind === 'panel'}
					title={t('shelf.options.label')}
					onClick={(event) => {
						const box = event.currentTarget.getBoundingClientRect();
						setMenu({
							kind: 'panel',
							position: { x: box.left, y: box.bottom },
							keyboard: event.detail === 0,
						});
					}}
				>
					<MoreIcon />
				</button>
				{placement && (
					<button
						type="button"
						className={styles.iconButton}
						aria-label={layout === 'window' ? t('shelf.dock') : t('shelf.undock')}
						title={layout === 'window' ? t('shelf.dock') : t('shelf.undock')}
						onClick={() => void (layout === 'window' ? placement.dock() : placement.undock())}
					>
						{layout === 'window' ? <DockShelfIcon /> : <UndockShelfIcon />}
					</button>
				)}
				{layout === 'dock' && (
					<button
						type="button"
						className={styles.iconButton}
						aria-label={t('shelf.close')}
						title={t('shelf.close')}
						onClick={() => store.getState().setOpen(false)}
					>
						<CloseSmallIcon />
					</button>
				)}
			</div>
			{items.length === 0 ? (
				<div className={styles.empty}>
					<h3 className={styles.emptyTitle}>{t('shelf.empty.title')}</h3>
					<p className={styles.emptyBody}>{t('shelf.empty.body')}</p>
				</div>
			) : (
				<div
					ref={treeRef}
					className={styles.tree}
					role="tree"
					tabIndex={0}
					aria-label={t('shelf.list.label')}
					aria-multiselectable="true"
					aria-activedescendant={focusedRow ? rowDomId(focusedRow.key) : undefined}
					onKeyDown={onKeyDown}
					onFocus={() => {
						if (store.getState().focus === null && rows[0]) store.getState().setFocus(rows[0].key);
					}}
				>
					{rows.map((row) =>
						row.kind === 'group' ? (
							<div
								key={row.key}
								id={rowDomId(row.key)}
								role="treeitem"
								aria-level={1}
								aria-expanded={row.expanded}
								aria-selected={false}
								className={styles.groupHeader}
								title={row.group.origin.display}
								data-focused={focus === row.key ? '' : undefined}
								onClick={() => {
									store.getState().setFocus(row.key);
									store.getState().toggleGroup(row.group.origin.uri);
								}}
							>
								<ChevronRightSmallIcon className={styles.chevron} />
								<span className={styles.groupName}>
									{tf('shelf.group.label', { name: row.group.name, count: row.group.items.length })}
								</span>
							</div>
						) : (
							<ItemRow
								key={row.key}
								item={row.item}
								state={status.get(row.item.location.uri) ?? 'unknown'}
								thumbnails={thumbnails}
								selected={selected.has(row.item.id)}
								focused={focus === row.key}
								onPointerDown={(event) => onRowPointerDown(event, row.item)}
								onClick={(event) => onRowClick(event, row.item)}
								onContextMenu={(event) => onRowContextMenu(event, row.item)}
								onOpen={() => void actions?.open(row.item)}
								onRemove={() => void actions?.remove([row.item.id])}
							/>
						),
					)}
				</div>
			)}
			{menu?.kind === 'item' && selectedItems.length > 0 && (
				<ContextMenu
					items={shelfItemMenuItems(selectedItems.length, true)}
					position={menu.position}
					ariaLabel={t('shelf.menu.label')}
					openedWithKeyboard={menu.keyboard}
					onClose={() => setMenu(null)}
					onSelect={(item) => {
						setMenu(null);
						runItemCommand(item.id as ShelfItemCommand);
					}}
				/>
			)}
			{menu?.kind === 'panel' && (
				<ContextMenu
					items={shelfPanelMenuItems(items.length)}
					position={menu.position}
					ariaLabel={t('shelf.options.menu')}
					openedWithKeyboard={menu.keyboard}
					onClose={() => setMenu(null)}
					onSelect={() => {
						setMenu(null);
						clear();
					}}
				/>
			)}
			<ConfirmDialog
				open={confirmClear}
				title={t('shelf.clear.confirm.title')}
				message={tn('shelf.clear.confirm.message', items.length)}
				confirmLabel={t('shelf.clear.confirm.confirm')}
				cancelLabel={t('shelf.clear.confirm.cancel')}
				danger
				onConfirm={() => {
					setConfirmClear(false);
					void actions?.clear();
				}}
				onCancel={() => setConfirmClear(false)}
			/>
		</aside>
	);
}

interface ItemRowProps {
	item: ShelfItem;
	state: ItemState;
	thumbnails: ThumbnailLoader<LocationRequest> | null;
	selected: boolean;
	focused: boolean;
	onPointerDown: (event: PointerEvent<HTMLElement>) => void;
	onClick: (event: MouseEvent<HTMLElement>) => void;
	onContextMenu: (event: MouseEvent<HTMLElement>) => void;
	onOpen: () => void;
	onRemove: () => void;
}

function ItemRow({
	item,
	state,
	thumbnails,
	selected,
	focused,
	onPointerDown,
	onClick,
	onContextMenu,
	onOpen,
	onRemove,
}: ItemRowProps) {
	const missing = state === 'missing';
	return (
		<div
			id={rowDomId(itemKey(item.id))}
			role="treeitem"
			aria-level={2}
			aria-selected={selected}
			className={styles.row}
			data-focused={focused ? '' : undefined}
			data-missing={missing ? '' : undefined}
			data-shelf-item={item.id}
			title={
				missing ? tf('shelf.missing.title', { path: item.location.display }) : item.location.display
			}
			onPointerDown={onPointerDown}
			onClick={onClick}
			onDoubleClick={onOpen}
			onContextMenu={onContextMenu}
		>
			<Thumbnail
				loader={thumbnails}
				thumbKey={state === 'file' ? item.location.uri : null}
				group={iconFor(state)}
				name={item.name}
				source={{ location: item.location, modifiedMs: null }}
				className={styles.tile}
				iconClassName={styles.icon}
			/>
			<span className={styles.text}>
				<span className={styles.name}>{item.name}</span>
				<span className={styles.origin}>{item.origin.display}</span>
			</span>
			{missing && <span className={styles.missing}>{t('shelf.missing')}</span>}
			<button
				type="button"
				tabIndex={-1}
				className={styles.remove}
				aria-label={tf('shelf.removeNamed', { name: item.name })}
				title={t('shelf.remove.title')}
				onPointerDown={(event) => event.stopPropagation()}
				onClick={(event) => {
					event.stopPropagation();
					onRemove();
				}}
			>
				<CloseSmallIcon />
			</button>
		</div>
	);
}

/** The top edge that resizes the dock: drag it, or focus it and use Up and Down (Shift for bigger steps). */
function ShelfDivider() {
	const store = useShelfStore();
	const height = useStore(store, (s) => s.height);
	const drag = useRef<{ startY: number; startHeight: number; pointerId: number } | null>(null);
	const [dragging, setDragging] = useState(false);

	const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
		if (event.button !== 0) return;
		event.currentTarget.setPointerCapture?.(event.pointerId);
		event.preventDefault();
		event.currentTarget.focus();
		drag.current = { startY: event.clientY, startHeight: height, pointerId: event.pointerId };
		setDragging(true);
	};
	const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
		const state = drag.current;
		if (!state) return;
		// The dock sits at the bottom: dragging its top edge up makes it taller.
		store.getState().setHeight(state.startHeight + (state.startY - event.clientY));
	};
	const finish = (event: PointerEvent<HTMLDivElement>, keep: boolean) => {
		const state = drag.current;
		if (!state) return;
		drag.current = null;
		setDragging(false);
		event.currentTarget.releasePointerCapture?.(state.pointerId);
		if (!keep) store.getState().setHeight(state.startHeight);
	};
	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		const step = event.shiftKey ? DIVIDER_BIG_STEP : DIVIDER_STEP;
		// The top edge is the one that moves: Up makes the dock taller.
		if (event.key === 'ArrowUp') store.getState().setHeight(height + step);
		else if (event.key === 'ArrowDown') store.getState().setHeight(height - step);
		else return;
		event.preventDefault();
	};
	useEffect(() => {
		const onKey = (event: globalThis.KeyboardEvent) => {
			if (event.key !== 'Escape' || !drag.current) return;
			event.stopPropagation();
			store.getState().setHeight(drag.current.startHeight);
			drag.current = null;
			setDragging(false);
		};
		window.addEventListener('keydown', onKey, true);
		return () => window.removeEventListener('keydown', onKey, true);
	}, [store]);

	return (
		<div
			role="separator"
			tabIndex={0}
			className={styles.divider}
			aria-orientation="horizontal"
			aria-label={t('shelf.divider.label')}
			aria-valuenow={height}
			aria-valuemin={MIN_HEIGHT}
			aria-valuemax={MAX_HEIGHT}
			aria-valuetext={tf('shelf.divider.value', { height })}
			data-dragging={dragging ? '' : undefined}
			onPointerDown={onPointerDown}
			onPointerMove={onPointerMove}
			onPointerUp={(event) => finish(event, true)}
			onPointerCancel={(event) => finish(event, false)}
			onDoubleClick={() => store.getState().setHeight(DEFAULT_HEIGHT)}
			onKeyDown={onKeyDown}
		/>
	);
}
