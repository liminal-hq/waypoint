// The icon grid: virtualised rows of columns over the same listing, selection and keyboard as the list
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useVirtualizer } from '@tanstack/react-virtual';
import {
	useEffect,
	useId,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
	useSyncExternalStore,
	type CSSProperties,
} from 'react';
import { useStore } from 'zustand';
import { entryDropAttributes, SCROLL_ATTRIBUTE } from '../dnd/dropTargets';
import { t, tf, tn } from '../i18n/messages';
import { useCutNames } from '../ops/ClipboardContext';
import { useFileCommands } from '../ops/FileCommandsContext';
import { Thumbnail } from '../thumbnails/Thumbnail';
import { devicePixelRatio, useEntryThumbnailLoader } from '../thumbnails/ThumbnailsContext';
import { entryThumbKey, thumbSizeFor, wantsThumbnail } from '../thumbnails/thumbnailModel';
import { useViewportThumbnails } from '../thumbnails/useViewportThumbnails';
import { InlineRename } from './InlineRename';
import { cellFor, columnsFor, GROUP_HEADER_HEIGHT, gridMove } from './gridLayout';
import { groupCount, groupLabel, groupTitle } from './groupHeader';
import { GroupLayout, groupId } from './groupLayout';
import { firstTarget } from './groupNav';
import styles from './GridView.module.css';
import { ErrorState, ListingGate, MessageState } from './ListingGate';
import { isReset, mapPosition } from './patch';
import { visibleRows } from './scrollCap';
import { isSelected, selectedCount } from './selection';
import {
	useListInteractions,
	type MenuRequest,
	type OpenHandler,
	type OpenInNewHandler,
} from './useListInteractions';
import type { ListingSession, SessionState } from './useListingSession';
import { formatLocale } from '../i18n/active';

/** Rows drawn beyond the viewport on each side. */
const OVERSCAN = 4;

interface GridViewProps {
	state: SessionState;
	/** Icon size in pixels (48 to 256). */
	size: number;
	onOpen?: OpenHandler | undefined;
	onOpenInNewTab?: OpenInNewHandler | undefined;
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
	const renaming = useStore(store, (state) => state.renaming);
	const scrollRequest = useStore(store, (state) => state.scrollRequest);
	const collapsed = useStore(store, (state) => state.collapsed);
	const focusHeader = useStore(store, (state) => state.focusHeader);
	const commands = useFileCommands();
	// What a cut holds in this folder is drawn dimmed until it is pasted or replaced.
	const cut = useCutNames(model.location.uri);

	const listId = useId();
	const scroller = useRef<HTMLDivElement | null>(null);
	const listbox = useRef<HTMLDivElement | null>(null);
	const [width, setWidth] = useState(0);
	const cell = cellFor(size);
	const columns = columnsFor(width, cell);
	const count = model.count;
	// A header takes a row of its own between the groups' rows of cells.
	const layout = useMemo(
		() => new GroupLayout(model.groups, collapsed, count, columns),
		[model.groups, collapsed, count, columns],
	);
	const layoutRef = useRef(layout);
	layoutRef.current = layout;
	const { shown: rows } = visibleRows(layout.rowCount, cell.height);
	const shownItems = layout.entriesWithin(rows);
	const hiddenItems = count - shownItems;

	// The first item of the row at the top of the viewport is what patches keep steady.
	const anchor = useRef({ top: 0, position: 0 });
	const reanchor = useRef(false);

	const virtualizer = useVirtualizer({
		count: rows,
		getScrollElement: () => scroller.current,
		estimateSize: (index) =>
			layout.rowAt(index).kind === 'header' ? GROUP_HEADER_HEIGHT : cell.height,
		overscan: OVERSCAN,
	});
	const virtualRows = virtualizer.getVirtualItems();
	const firstRow = virtualRows[0]?.index ?? 0;
	const lastRow = virtualRows[virtualRows.length - 1]?.index ?? 0;
	// What is in view, without the rows drawn beyond the edge: thumbnails are asked for from this. A
	// header row has no entries, so the span is the first to the last entry among the rows seen.
	const inView = virtualizer.range;
	let viewFirst = count;
	let viewLast = -1;
	for (
		let index = inView?.startIndex ?? firstRow;
		index <= (inView?.endIndex ?? lastRow);
		index++
	) {
		const row = layout.rowAt(index);
		if (row.kind !== 'entries') continue;
		viewFirst = Math.min(viewFirst, row.first);
		viewLast = Math.max(viewLast, row.first + row.count - 1);
	}
	if (viewLast < 0) viewFirst = 0;
	const thumbnails = useEntryThumbnailLoader(model.handle, thumbSizeFor(size, devicePixelRatio()));
	useViewportThumbnails({
		loader: thumbnails,
		first: viewFirst,
		last: viewLast,
		count: shownItems,
		version,
		itemAt: (position) => {
			const entry = model.entryAt(position);
			return entry && wantsThumbnail(entry) ? { key: entryThumbKey(entry), id: entry.id } : null;
		},
	});

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
	}, [cell.height, columns, layout, virtualizer]);

	// Fetch what is on screen plus a page either side. A folded group between two visible rows is
	// not fetched: the entries on either side are asked for separately.
	useEffect(() => {
		if (virtualRows.length === 0) return;
		let run: [number, number] | null = null;
		for (let index = firstRow; index <= lastRow; index++) {
			const row = layout.rowAt(index);
			if (row.kind !== 'entries') continue;
			const end = row.first + row.count - 1;
			if (run && row.first === run[1] + 1) run[1] = end;
			else {
				if (run) model.ensure(run[0], run[1]);
				run = [row.first, end];
			}
		}
		if (run) model.ensure(run[0], run[1]);
	}, [model, layout, firstRow, lastRow, virtualRows.length, version]);

	// Scroll anchoring, as in the list but in rows of `columns`: the first item of the top row is
	// followed through each patch and the offset moves by the rows it moved.
	useEffect(
		() =>
			model.onPatch((report) => {
				const current = anchor.current;
				if (current.top === 0 || isReset(report.ops)) return;
				const { position } = mapPosition(current.position, report.ops);
				// Where that row stood, and where it stands now that the groups have moved with the patch.
				const before = layoutRef.current;
				const after = new GroupLayout(
					model.groups,
					store.getState().collapsed,
					report.count,
					columns,
				);
				const height = (layout: GroupLayout, row: number) =>
					layout.offsetOfRow(row, GROUP_HEADER_HEIGHT, cell.height);
				const above = current.top - height(before, before.rowNear(current.position));
				const top = Math.max(0, height(after, after.rowNear(position)) + above);
				if (top === current.top && position === current.position) return;
				anchor.current = { top, position };
				reanchor.current = true;
			}),
		[model, store, columns, cell.height],
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

	// A restored offset is applied once the listing is tall enough to reach it (or has stopped growing).
	useLayoutEffect(() => {
		const element = scroller.current;
		const pending = session.view.pendingScroll;
		if (!element || pending === null) return;
		if (element.scrollHeight - element.clientHeight < pending && model.phase === 'scanning') return;
		session.view.pendingScroll = null;
		element.scrollTop = pending;
		element.dispatchEvent(new Event('scroll'));
	}, [session, model, version]);

	const recordAnchor = () => {
		if (session.view.pendingScroll !== null) return;
		const top = scroller.current?.scrollTop ?? 0;
		session.view.gridScrollTop = top;
		const layout = layoutRef.current;
		anchor.current = {
			top,
			position: layout.positionAtRow(layout.rowAtOffset(top, GROUP_HEADER_HEIGHT, cell.height)),
		};
	};

	const scrollToItem = (position: number) =>
		virtualizer.scrollToIndex(
			Math.max(0, Math.min(rows - 1, layout.rowNear(Math.max(0, position)))),
			{
				align: 'auto',
			},
		);
	const scrollToHeader = (group: number) =>
		virtualizer.scrollToIndex(Math.max(0, Math.min(rows - 1, layout.rowOfHeader(group))), {
			align: 'auto',
		});

	// A command that made or found an entry asks for it to be brought into sight.
	useEffect(() => {
		if (scrollRequest) scrollToItem(scrollRequest.position);
	}, [scrollRequest]);

	const pageRows = () =>
		Math.max(1, Math.floor((scroller.current?.clientHeight ?? 0) / cell.height) - 1);
	const {
		onKeyDown,
		onItemClick,
		onItemPointerDown,
		onItemDoubleClick,
		onItemContextMenu,
		onBackgroundContextMenu,
		onBackgroundClick,
		onHeaderClick,
	} = useListInteractions({
		session,
		itemId: (position) => `${listId}-item-${position}`,
		shown: shownItems,
		scrollTo: scrollToItem,
		layout,
		pageRows,
		scrollToHeader,
		onOpen,
		onMenu,
		thumbnailOf: (entry) => thumbnails?.urlOf(entryThumbKey(entry)) ?? null,
		move: (key, from, last) => gridMove(key, from, last, columns, pageRows()),
	});

	if (model.error) return <ErrorState error={model.error} />;

	const scanning = model.phase === 'scanning' || model.phase === 'rescanning';
	const empty = count === 0 && !scanning;
	const headerGroup =
		focusHeader === null ? -1 : model.groups.findIndex((run) => groupId(run.key) === focusHeader);
	const activeId =
		headerGroup >= 0
			? `${listId}-group-${headerGroup}`
			: focus === null
				? undefined
				: `${listId}-item-${focus}`;
	const selectionText = touched
		? selectedCount(selection, count) === 0
			? t('browse.selection.none')
			: tn('browse.selection', selectedCount(selection, count))
		: '';
	const gridVars = {
		'--wp-grid-size': `${size}px`,
		'--wp-grid-columns': columns,
		'--wp-grid-height': `${virtualizer.getTotalSize()}px`,
		'--wp-group-header-height': `${GROUP_HEADER_HEIGHT}px`,
	} as CSSProperties;

	return (
		<div className={styles.view}>
			{scanning && (
				<div className={styles.notice} role="status" data-notice="scanning">
					{tf('browse.scanning', {
						count: new Intl.NumberFormat(formatLocale()).format(model.scanned),
					})}
				</div>
			)}
			{hiddenItems > 0 && (
				<div className={styles.notice} role="status" data-notice="capped">
					{tf('browse.capped', {
						shown: new Intl.NumberFormat(formatLocale()).format(shownItems),
						total: new Intl.NumberFormat(formatLocale()).format(count),
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
					{...{ [SCROLL_ATTRIBUTE]: '' }}
					onScroll={recordAnchor}
					onContextMenu={onBackgroundContextMenu}
					onClick={onBackgroundClick}
				>
					<div
						ref={listbox}
						role="listbox"
						tabIndex={0}
						aria-label={t('browse.list.label')}
						aria-multiselectable="true"
						aria-activedescendant={activeId}
						className={styles.grid}
						style={gridVars}
						onKeyDown={onKeyDown}
						onFocus={() => {
							const state = store.getState();
							if (state.focus !== null || state.focusHeader !== null || count === 0) return;
							const first = firstTarget(layout);
							if ('header' in first) state.focusGroup(groupId(model.groups[first.header]!.key));
							else state.moveTo(first.position, false);
						}}
					>
						{virtualRows.map((row) => {
							const at = layout.rowAt(row.index);
							if (at.kind === 'header') {
								const run = model.groups[at.group]!;
								const folded = layout.isCollapsed(at.group);
								return (
									<div
										key={row.key}
										id={`${listId}-group-${at.group}`}
										role="group"
										aria-label={groupLabel(run.key, run.count, folded)}
										className={styles.groupHeader}
										style={{ '--wp-row-y': `${row.start}px` } as CSSProperties}
										data-collapsed={folded ? '' : undefined}
										data-active={headerGroup === at.group ? '' : undefined}
										onClick={() => onHeaderClick(at.group)}
									>
										<span className={styles.chevron} aria-hidden="true" />
										<span className={styles.groupTitle}>{groupTitle(run.key)}</span>
										<span className={styles.groupCount}>{groupCount(run.count)}</span>
									</div>
								);
							}
							const start = at.first;
							const end = Math.min(shownItems, start + at.count);
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
												title={
													entry?.originalPath ? `${entry.name}\n${entry.originalPath}` : entry?.name
												}
												aria-selected={selected}
												aria-setsize={count}
												aria-posinset={position + 1}
												aria-busy={entry ? undefined : true}
												data-placeholder={entry ? undefined : ''}
												data-selected={selected ? '' : undefined}
												data-cut={entry && cut.has(entry.name) ? '' : undefined}
												data-active={focus === position && headerGroup < 0 ? '' : undefined}
												{...(entry ? entryDropAttributes(entry, model) : undefined)}
												onPointerDown={(event) => onItemPointerDown(event, position, entry)}
												onClick={(event) => onItemClick(event, position, entry)}
												onContextMenu={(event) => onItemContextMenu(event, position, entry)}
												onDoubleClick={() => onItemDoubleClick(entry)}
												onMouseDown={(event) => {
													// Stops middle-click from starting the platform's autoscroll.
													if (event.button === 1) event.preventDefault();
												}}
												onAuxClick={(event) => {
													if (event.button === 1 && entry) {
														event.preventDefault();
														onOpenInNewTab?.(entry, model.handle, event.ctrlKey);
													}
												}}
											>
												{entry ? (
													<>
														<Thumbnail
															loader={thumbnails}
															thumbKey={wantsThumbnail(entry) ? entryThumbKey(entry) : null}
															group={entry.group}
															className={styles.thumbnail}
															iconClassName={styles.glyph}
														/>
														{commands && renaming === entry.id ? (
															<InlineRename
																entry={entry}
																session={session}
																commands={commands}
																variant="grid"
																onFinish={() => listbox.current?.focus()}
															/>
														) : (
															<span className={styles.label}>{entry.name}</span>
														)}
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
