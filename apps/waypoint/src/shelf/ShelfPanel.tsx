// The Shelf panel: references grouped by the folder they came from, which can be selected, removed, opened and dragged out
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
import { FileIcon } from '../browse/FileIcon';
import { modifiersOf } from '../dnd/dropAction';
import { dropAttributes } from '../dnd/dropTargets';
import { useFileDragApi } from '../dnd/FileDragContext';
import { t, tf, tn } from '../i18n/messages';
import { ChevronRightSmallIcon, CloseSmallIcon } from '../icons/AppIcons';
import { MoreIcon } from '../icons/MenuIcons';
import { useShelfActions } from './ShelfContext';
import {
	commonOrigin,
	groupItems,
	groupKey,
	iconFor,
	itemKey,
	orderOf,
	visibleRows,
	type ItemState,
	type ShelfRow,
} from './shelfModel';
import { shelfItemMenuItems, shelfPanelMenuItems, type ShelfItemCommand } from './shelfMenu';
import { DEFAULT_WIDTH, MAX_WIDTH, MIN_WIDTH, useShelfStore } from './shelfStore';
import styles from './ShelfPanel.module.css';

/** How far an arrow key moves the divider, and how far with Shift. */
const DIVIDER_STEP = 16;
const DIVIDER_BIG_STEP = 64;

const rowDomId = (key: string) => `shelf-${key.replace(/[^a-zA-Z0-9_-]/g, '_')}`;

interface MenuRequest {
	kind: 'item' | 'panel';
	position: { x: number; y: number };
	keyboard: boolean;
}

/**
 * The docked Shelf. The whole panel is a drop target (`data-drop="shelf"`), and each row is a
 * drag source of references (`pressLocations`). The list is a tree with one level of groups; the
 * keyboard moves through the rows (Up, Down, Home, End; Left and Right close and open a group),
 * Enter opens an item, Space and Ctrl+Space select, Shift extends, Delete removes from the Shelf
 * (never a file), Ctrl+C copies the items for a paste, and the Menu key opens the item's menu.
 */
export function ShelfPanel() {
	const store = useShelfStore();
	const actions = useShelfActions();
	const drag = useFileDragApi();
	const items = useStore(store, (s) => s.items);
	const width = useStore(store, (s) => s.width);
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

	const itemMenuAt = (position: { x: number; y: number }, keyboard: boolean) =>
		setMenu({ kind: 'item', position, keyboard });

	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		if (event.defaultPrevented || event.nativeEvent.isComposing) return;
		const state = store.getState();
		const at = focusedRow ? rows.indexOf(focusedRow) : -1;
		const ctrl = event.ctrlKey || event.metaKey;
		const consume = () => event.preventDefault();
		switch (event.key) {
			case 'ArrowDown':
				consume();
				return moveFocus(rows[at < 0 ? 0 : Math.min(rows.length - 1, at + 1)], event.shiftKey);
			case 'ArrowUp':
				consume();
				return moveFocus(rows[at < 0 ? 0 : Math.max(0, at - 1)], event.shiftKey);
			case 'Home':
				consume();
				return moveFocus(rows[0], event.shiftKey);
			case 'End':
				consume();
				return moveFocus(rows[rows.length - 1], event.shiftKey);
			case 'ArrowRight':
				consume();
				if (focusedRow?.kind === 'group' && !focusedRow.expanded) {
					state.toggleGroup(focusedRow.group.origin.uri, false);
				} else if (focusedRow?.kind === 'group') moveFocus(rows[at + 1], false);
				return;
			case 'ArrowLeft':
				consume();
				if (focusedRow?.kind === 'group' && focusedRow.expanded) {
					state.toggleGroup(focusedRow.group.origin.uri, true);
				} else if (focusedRow?.kind === 'item') {
					moveFocus(
						rows.find((row) => row.key === groupKey(focusedRow.group.origin)),
						false,
					);
				}
				return;
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
			className={styles.shelf}
			aria-label={t('shelf.label')}
			style={{ width, '--wp-shelf-width': `${width}px` } as CSSProperties}
			data-shelf=""
			{...dropAttributes('shelf', 'shelf', t('shelf.title'))}
		>
			<ShelfDivider />
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
				<button
					type="button"
					className={styles.iconButton}
					aria-label={t('shelf.close')}
					title={t('shelf.close')}
					onClick={() => store.getState().setOpen(false)}
				>
					<CloseSmallIcon />
				</button>
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
			<FileIcon group={iconFor(state)} className={styles.icon} />
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

/** The edge that resizes the panel: drag it, or focus it and use the arrow keys (Shift for bigger steps). */
function ShelfDivider() {
	const store = useShelfStore();
	const width = useStore(store, (s) => s.width);
	const drag = useRef<{
		startX: number;
		startWidth: number;
		sign: 1 | -1;
		pointerId: number;
	} | null>(null);
	const [dragging, setDragging] = useState(false);

	const onPointerDown = (event: PointerEvent<HTMLDivElement>) => {
		if (event.button !== 0) return;
		const rtl = getComputedStyle(event.currentTarget).direction === 'rtl';
		event.currentTarget.setPointerCapture?.(event.pointerId);
		event.preventDefault();
		event.currentTarget.focus();
		// The panel is docked at the end edge: dragging toward the start widens it.
		drag.current = {
			startX: event.clientX,
			startWidth: width,
			sign: rtl ? 1 : -1,
			pointerId: event.pointerId,
		};
		setDragging(true);
	};
	const onPointerMove = (event: PointerEvent<HTMLDivElement>) => {
		const state = drag.current;
		if (!state) return;
		store.getState().setWidth(state.startWidth + (event.clientX - state.startX) * state.sign);
	};
	const finish = (event: PointerEvent<HTMLDivElement>, keep: boolean) => {
		const state = drag.current;
		if (!state) return;
		drag.current = null;
		setDragging(false);
		event.currentTarget.releasePointerCapture?.(state.pointerId);
		if (!keep) store.getState().setWidth(state.startWidth);
	};
	const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
		const rtl = getComputedStyle(event.currentTarget).direction === 'rtl';
		const step = event.shiftKey ? DIVIDER_BIG_STEP : DIVIDER_STEP;
		// The start edge is the one that moves: Left widens in left-to-right text.
		const wider = rtl ? 'ArrowRight' : 'ArrowLeft';
		const narrower = rtl ? 'ArrowLeft' : 'ArrowRight';
		if (event.key === wider) store.getState().setWidth(width + step);
		else if (event.key === narrower) store.getState().setWidth(width - step);
		else return;
		event.preventDefault();
	};
	useEffect(() => {
		const onKey = (event: globalThis.KeyboardEvent) => {
			if (event.key !== 'Escape' || !drag.current) return;
			event.stopPropagation();
			store.getState().setWidth(drag.current.startWidth);
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
			aria-orientation="vertical"
			aria-label={t('shelf.divider.label')}
			aria-valuenow={width}
			aria-valuemin={MIN_WIDTH}
			aria-valuemax={MAX_WIDTH}
			aria-valuetext={tf('shelf.divider.value', { width })}
			data-dragging={dragging ? '' : undefined}
			onPointerDown={onPointerDown}
			onPointerMove={onPointerMove}
			onPointerUp={(event) => finish(event, true)}
			onPointerCancel={(event) => finish(event, false)}
			onDoubleClick={() => store.getState().setWidth(DEFAULT_WIDTH)}
			onKeyDown={onKeyDown}
		/>
	);
}
