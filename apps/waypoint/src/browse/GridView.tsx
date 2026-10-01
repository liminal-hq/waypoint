// The icon grid: virtualised rows of columns over the same listing, selection and keyboard as the list
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useVirtualizer } from '@tanstack/react-virtual';
import {
	useEffect,
	useId,
	useLayoutEffect,
	useRef,
	useState,
	useSyncExternalStore,
	type CSSProperties,
} from 'react';
import { useStore } from 'zustand';
import { t, tf, tn } from '../i18n/messages';
import { FileIcon } from './FileIcon';
import { cappedGrid, cellFor, columnsFor, gridMove } from './gridLayout';
import styles from './GridView.module.css';
import { ErrorState, ListingGate, MessageState } from './ListingGate';
import { isReset, mapPosition } from './patch';
import { isSelected, selectedCount } from './selection';
import { useListInteractions, type MenuRequest, type OpenHandler } from './useListInteractions';
import type { ListingSession, SessionState } from './useListingSession';

/** Rows drawn beyond the viewport on each side. */
const OVERSCAN = 4;

interface GridViewProps {
	state: SessionState;
	/** Icon size in pixels (48 to 256). */
	size: number;
	onOpen?: OpenHandler | undefined;
	onOpenInNewTab?: OpenHandler | undefined;
	onMenu?: ((request: MenuRequest) => void) | undefined;
	/** Whether the grid announces selection changes itself; a host with a status bar does that. */
	announceSelection?: boolean | undefined;
}

/** The grid for a listing that is opening, failed or ready. */
export function GridView({
	state,
	size,
	onOpen,
	onOpenInNewTab,
	onMenu,
	announceSelection = true,
}: GridViewProps) {
	return (
		<ListingGate state={state}>
			{(session) => (
				<GridBody
					key={session.model.handle}
					session={session}
					size={size}
					onOpen={onOpen}
					onOpenInNewTab={onOpenInNewTab}
					onMenu={onMenu}
					announceSelection={announceSelection}
				/>
			)}
		</ListingGate>
	);
}

interface GridBodyProps extends Omit<GridViewProps, 'state' | 'announceSelection'> {
	session: ListingSession;
	announceSelection: boolean;
}

function GridBody({
	session,
	size,
	onOpen,
	onOpenInNewTab,
	onMenu,
	announceSelection,
}: GridBodyProps) {
	const { model, store } = session;
	const version = useSyncExternalStore(model.subscribe, model.getVersion);
	const selection = useStore(store, (state) => state.selection);
	const focus = useStore(store, (state) => state.focus);
	const touched = useStore(store, (state) => state.touched);

	const listId = useId();
	const scroller = useRef<HTMLDivElement | null>(null);
	const [width, setWidth] = useState(0);
	const cell = cellFor(size);
	const columns = columnsFor(width, cell);
	const count = model.count;
	const { rows, shownItems, hiddenItems } = cappedGrid(count, columns, cell);

	// The first item of the row at the top of the viewport is what patches keep steady.
	const anchor = useRef({ top: 0, position: 0 });
	const reanchor = useRef(false);

	const virtualizer = useVirtualizer({
		count: rows,
		getScrollElement: () => scroller.current,
		estimateSize: () => cell.height,
		overscan: OVERSCAN,
	});
	const virtualRows = virtualizer.getVirtualItems();
	const firstRow = virtualRows[0]?.index ?? 0;
	const lastRow = virtualRows[virtualRows.length - 1]?.index ?? 0;
	const firstItem = firstRow * columns;
	const lastItem = Math.min(count - 1, (lastRow + 1) * columns - 1);

	// The container's width decides the column count, so it is measured, not assumed.
	useLayoutEffect(() => {
		const element = scroller.current;
		if (!element) return;
		setWidth(element.clientWidth);
		if (typeof ResizeObserver === 'undefined') return;
		const observer = new ResizeObserver(() => setWidth(element.clientWidth));
		observer.observe(element);
		return () => observer.disconnect();
	}, []);

	useEffect(() => {
		virtualizer.measure();
	}, [cell.height, columns, virtualizer]);

	useEffect(() => {
		if (virtualRows.length > 0) model.ensure(firstItem, lastItem);
	}, [model, firstItem, lastItem, virtualRows.length, version]);

	// Scroll anchoring, as in the list but in rows of `columns`: the first item of the top row is
	// followed through each patch and the offset moves by the rows it moved.
	useEffect(
		() =>
			model.onPatch((report) => {
				const current = anchor.current;
				if (current.top === 0 || isReset(report.ops)) return;
				const { position } = mapPosition(current.position, report.ops);
				const movedRows = Math.floor(position / columns) - Math.floor(current.position / columns);
				if (movedRows === 0) return;
				anchor.current = {
					top: Math.max(0, current.top + movedRows * cell.height),
					position,
				};
				reanchor.current = true;
			}),
		[model, columns, cell.height],
	);

	useLayoutEffect(() => {
		const element = scroller.current;
		if (!reanchor.current || !element) return;
		reanchor.current = false;
		element.scrollTop = anchor.current.top;
		element.dispatchEvent(new Event('scroll'));
	}, [version]);

	// A tab that returns to this listing finds the scroll position it left.
	useLayoutEffect(() => {
		const element = scroller.current;
		if (!element || session.view.gridScrollTop <= 0) return;
		element.scrollTop = session.view.gridScrollTop;
		element.dispatchEvent(new Event('scroll'));
	}, [session]);

	const recordAnchor = () => {
		const top = scroller.current?.scrollTop ?? 0;
		session.view.gridScrollTop = top;
		anchor.current = { top, position: Math.floor(top / cell.height) * columns };
	};

	const scrollToItem = (position: number) =>
		virtualizer.scrollToIndex(Math.floor(Math.max(0, position) / columns), { align: 'auto' });

	const pageRows = () =>
		Math.max(1, Math.floor((scroller.current?.clientHeight ?? 0) / cell.height) - 1);
	const { onKeyDown, onItemClick, onItemContextMenu, onBackgroundContextMenu } =
		useListInteractions({
			session,
			itemId: (position) => `${listId}-item-${position}`,
			shown: shownItems,
			scrollTo: scrollToItem,
			onOpen,
			onMenu,
			move: (key, from, last) => gridMove(key, from, last, columns, pageRows()),
		});

	if (model.error) return <ErrorState error={model.error} />;

	const scanning = model.phase === 'scanning' || model.phase === 'rescanning';
	const empty = count === 0 && !scanning;
	const activeId = focus === null ? undefined : `${listId}-item-${focus}`;
	const selectionText = touched
		? selectedCount(selection, count) === 0
			? t('browse.selection.none')
			: tn('browse.selection', selectedCount(selection, count))
		: '';
	const gridVars = {
		'--wp-grid-size': `${size}px`,
		'--wp-grid-columns': columns,
		'--wp-grid-height': `${virtualizer.getTotalSize()}px`,
	} as CSSProperties;

	return (
		<div className={styles.view}>
			{scanning && (
				<div className={styles.notice} role="status" data-notice="scanning">
					{tf('browse.scanning', { count: new Intl.NumberFormat().format(model.scanned) })}
				</div>
			)}
			{hiddenItems > 0 && (
				<div className={styles.notice} role="status" data-notice="capped">
					{tf('browse.capped', {
						shown: new Intl.NumberFormat().format(shownItems),
						total: new Intl.NumberFormat().format(count),
					})}
				</div>
			)}

			{empty ? (
				<div className={styles.emptyArea} onContextMenu={onBackgroundContextMenu}>
					<MessageState role="status" data-state="empty">
						{t('browse.empty')}
					</MessageState>
				</div>
			) : (
				<div
					ref={scroller}
					className={styles.scroller}
					onScroll={recordAnchor}
					onContextMenu={onBackgroundContextMenu}
				>
					<div
						role="listbox"
						tabIndex={0}
						aria-label={t('browse.list.label')}
						aria-multiselectable="true"
						aria-activedescendant={activeId}
						className={styles.grid}
						style={gridVars}
						onKeyDown={onKeyDown}
						onFocus={() => {
							if (store.getState().focus === null && count > 0) store.getState().moveTo(0, false);
						}}
					>
						{virtualRows.map((row) => {
							const start = row.index * columns;
							const end = Math.min(shownItems, start + columns);
							return (
								<div
									key={row.key}
									role="presentation"
									className={styles.row}
									style={{ '--wp-row-y': `${row.start}px` } as CSSProperties}
								>
									{Array.from({ length: Math.max(0, end - start) }, (_, offset) => {
										const position = start + offset;
										const entry = model.entryAt(position);
										const selected = entry ? isSelected(selection, entry.id) : false;
										return (
											<div
												key={position}
												id={`${listId}-item-${position}`}
												role="option"
												className={styles.cell}
												title={entry?.name}
												aria-selected={selected}
												aria-setsize={shownItems}
												aria-posinset={position + 1}
												aria-busy={entry ? undefined : true}
												data-placeholder={entry ? undefined : ''}
												data-selected={selected ? '' : undefined}
												data-active={focus === position ? '' : undefined}
												onClick={(event) => onItemClick(event, position, entry)}
												onContextMenu={(event) => onItemContextMenu(event, position, entry)}
												onDoubleClick={() => entry && onOpen?.(entry, model.handle)}
												onMouseDown={(event) => {
													// Stops middle-click from starting the platform's autoscroll.
													if (event.button === 1) event.preventDefault();
												}}
												onAuxClick={(event) => {
													if (event.button === 1 && entry) {
														event.preventDefault();
														onOpenInNewTab?.(entry, model.handle);
													}
												}}
											>
												{entry ? (
													<>
														<FileIcon group={entry.group} className={styles.glyph} />
														<span className={styles.label}>{entry.name}</span>
													</>
												) : (
													<>
														<span className={styles.skeleton} />
														<span className={styles.srOnly}>{t('browse.row.loading')}</span>
													</>
												)}
											</div>
										);
									})}
								</div>
							);
						})}
					</div>
				</div>
			)}

			{announceSelection && (
				<div className={styles.srOnly} role="status" aria-live="polite">
					{selectionText}
				</div>
			)}
		</div>
	);
}
