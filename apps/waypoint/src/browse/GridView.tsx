// The icon grid: virtualised rows of columns over the same listing, selection and keyboard as the list
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { defaultRangeExtractor, useVirtualizer } from '@tanstack/react-virtual';
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
import { SCROLL_ATTRIBUTE } from '../dnd/dropTargets';
import { t, tf, tn } from '../i18n/messages';
import { useCutNames } from '../ops/ClipboardContext';
import { useFileCommands } from '../ops/FileCommandsContext';
import { devicePixelRatio, useEntryThumbnailLoader } from '../thumbnails/ThumbnailsContext';
import { entryThumbKey, thumbSizeFor, wantsThumbnail } from '../thumbnails/thumbnailModel';
import { createThumbnailPause, ThumbnailPauseContext } from '../thumbnails/thumbnailPause';
import { useViewportThumbnails } from '../thumbnails/useViewportThumbnails';
import { GridRow, type GridCellActions } from './GridCell';
import { useScrollStepRender } from './scrollStepRender';
import { IconPictures } from './IconPictures';
import {
	cellFor,
	columnsFor,
	firstColumnWithRoom,
	GROUP_HEADER_HEIGHT,
	gridMove,
} from './gridLayout';
import { groupCount, groupLabel, groupTitle } from './groupHeader';
import { GroupLayout, groupId } from './groupLayout';
import { firstTarget } from './groupNav';
import styles from './GridView.module.css';
import { ErrorState, ListingGate, MessageState } from './ListingGate';
import { isReset, mapPosition } from './patch';
import { visibleRows } from './scrollCap';
import { selectedCount } from './selection';
import {
	useListInteractions,
	type MenuRequest,
	type OpenHandler,
	type OpenInNewHandler,
} from './useListInteractions';
import type { ListingSession, SessionState } from './useListingSession';
import { formatLocale } from '../i18n/active';

/** The first row in view, without the rows drawn beyond the edge. */
const inViewFrom = (virtualizer: { range: { startIndex: number } | null }) =>
	virtualizer.range?.startIndex ?? 0;

/**
 * Rows drawn beyond the viewport on each side. A scroll step is drawn in its own frame
 * (`useScrollStepRender`), so these are for the keyboard and a jump, not to hide a late render; each is
 * a layer of its own that is painted whenever its items change, so fewer is cheaper.
 */
const OVERSCAN = 2;
/** Rows fetched beyond those drawn on each side, as a multiple of the rows drawn. */
const FETCH_AHEAD = 2;
/** Thumbnails are asked for once a scroll has stopped for this long (see `useViewportThumbnails`). */
const THUMBNAIL_SETTLE_MS = 120;

interface GridViewProps {
	state: SessionState;
	/** Icon size in pixels (48 to 256). */
	size: number;
	onOpen?: OpenHandler | undefined;
	onOpenInNewTab?: OpenInNewHandler | undefined;
	onMenu?: ((request: MenuRequest) => void) | undefined;
	/** Whether the grid announces selection changes itself; a host with a status bar does that. */
	announceSelection?: boolean | undefined;
	/** The grid's accessible name when "Files" does not tell it apart from another list on screen (a pane of a pair). */
	label?: string | undefined;
}

/** The grid for a listing that is opening, failed or ready. */
export function GridView({
	state,
	size,
	onOpen,
	onOpenInNewTab,
	onMenu,
	announceSelection = true,
	label,
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
					label={label}
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
	label,
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

	// See the slots below: how many rows are drawn, at the most the view has needed so far.
	const pool = useRef(1);
	// A new icon size starts the pool again, so a larger size does not keep drawing the rows a smaller one needed.
	const pooledHeight = useRef(cell.height);
	if (pooledHeight.current !== cell.height) {
		pooledHeight.current = cell.height;
		pool.current = 1;
	}
	const drawStep = useScrollStepRender();
	// Pictures not drawn yet wait while the grid scrolls (see `ThumbnailPause`).
	const [pause] = useState(createThumbnailPause);
	// The first row in view at the last render, and whether that render was a jump, to tell a fling.
	const drawn = useRef({ start: 0, jumped: false, sketch: false });
	const virtualizer = useVirtualizer({
		count: rows,
		getScrollElement: () => scroller.current,
		estimateSize: (index) =>
			layout.rowAt(index).kind === 'header' ? GROUP_HEADER_HEIGHT : cell.height,
		overscan: OVERSCAN,
		// Always as many rows as the pool holds, so a row is never taken away and put back as the run
		// in view grows and shrinks by one from one step to the next.
		rangeExtractor: (range) => {
			const indexes = defaultRangeExtractor(range);
			while (indexes.length > 0 && indexes.length < pool.current) {
				const first = indexes[0]!;
				const last = indexes[indexes.length - 1]!;
				if (last < range.count - 1) indexes.push(last + 1);
				else if (first > 0) indexes.unshift(first - 1);
				else break;
			}
			return indexes;
		},
		// Off: the default flushes a state update synchronously from the scroll and resize callbacks, which React
		// rejects (and logs) whenever one lands during a render. A scroll step is drawn in its own frame
		// by `drawStep` instead (see `useScrollStepRender`).
		useFlushSync: false,
		onChange: (instance, sync) => {
			pause.set(instance.isScrolling);
			drawStep(sync);
		},
	});
	const virtualRows = virtualizer.getVirtualItems();
	const firstRow = virtualRows[0]?.index ?? 0;
	const lastRow = virtualRows[virtualRows.length - 1]?.index ?? 0;
	// Rows are keyed by slot, not by index: a row that scrolls out hands its elements to the row that
	// scrolls in, so a scroll changes the text and attributes of a few cells instead of building new
	// ones (creating and laying out the icons and names of every new row cost most of a frame in a
	// big window, #565). The rendered rows are a run of consecutive indices, so the index modulo a
	// pool at least as long as the run never gives two of them the same slot; the pool only grows (a
	// new icon size starts it again), so the slots stay put while the window keeps its size. The rows are also drawn in slot order:
	// in index order, the row that changed hands would move from one end of the grid's children to
	// the other each step, and a moved element is taken out of the page and laid out again whole.
	// Rows are placed by transform, so the order of the children does not show; a raised row orders
	// itself by its index (see `GridRow`).
	pool.current = Math.max(pool.current, virtualRows.length);
	const slots = pool.current;
	const slotted = [...virtualRows].sort((a, b) => (a.index % slots) - (b.index % slots));
	// A fling jumps past every row drawn, frame after frame. From its second jump on, the rows are
	// drawn as placeholders: nobody reads four thousand names a frame, most of their pages are not
	// there yet, and drawing a whole grid of names and icons every frame made frames twice as long.
	// The next step that is not a jump (the fling slowing) or the scroll ending draws the items; a page
	// arriving mid-fling does not. A single jump (End, a click on the scrollbar) draws them at once.
	const viewStart = inViewFrom(virtualizer);
	const moved = viewStart !== drawn.current.start;
	const jumped = moved ? Math.abs(viewStart - drawn.current.start) >= slots : drawn.current.jumped;
	const sketch = moved
		? jumped && drawn.current.jumped
		: drawn.current.sketch && virtualizer.isScrolling;
	useLayoutEffect(() => {
		drawn.current = { start: viewStart, jumped, sketch };
	});
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
		settleMs: THUMBNAIL_SETTLE_MS,
		scrolling: virtualizer.isScrolling,
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

	// Fetch what is drawn plus `FETCH_AHEAD` times as many rows again on each side (and the model adds
	// a page either side). A row of a wide grid holds dozens of items, so a page lasts a frame or two
	// of a fast scroll: fetching only around the drawn rows left new rows blank while their page was
	// on its way (#565). After a jump past every drawn row (a fling) only the drawn rows are fetched:
	// the next frame is likely another jump, and pages for rows it skips would be thrown away. A
	// folded group between two rows is not fetched: the entries on either side are asked for
	// separately.
	const fetched = useRef({ from: firstRow, jumped: false });
	useEffect(() => {
		if (virtualRows.length === 0) return;
		// Decided when the rows move; a page arriving afterwards keeps the decision.
		if (firstRow !== fetched.current.from) {
			fetched.current = {
				from: firstRow,
				jumped: Math.abs(firstRow - fetched.current.from) >= slots,
			};
		}
		const ahead = fetched.current.jumped ? 0 : (lastRow - firstRow + 1) * FETCH_AHEAD;
		let run: [number, number] | null = null;
		for (
			let index = Math.max(0, firstRow - ahead);
			index <= Math.min(rows - 1, lastRow + ahead);
			index++
		) {
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
	}, [model, layout, rows, slots, firstRow, lastRow, virtualRows.length, version]);

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
	// One object for every cell, pointing at this render's handlers, so the memoised cells see no change.
	const latest = {
		pointerDown: onItemPointerDown,
		click: onItemClick,
		contextMenu: onItemContextMenu,
		doubleClick: onItemDoubleClick,
		openInNewTab: (entry: Entry, inNewWindow: boolean) =>
			onOpenInNewTab?.(entry, model.handle, inNewWindow),
		finishRename: () => listbox.current?.focus(),
	} satisfies GridCellActions;
	const cellActions = useRef<GridCellActions>(latest);
	cellActions.current = latest;

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

			<ThumbnailPauseContext.Provider value={pause}>
				<IconPictures cellClassName={styles.cell} glyphClassName={styles.glyph}>
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
								aria-label={label ?? t('browse.list.label')}
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
								{slotted.map((row) => {
									const at = layout.rowAt(row.index);
									if (at.kind === 'header') {
										const run = model.groups[at.group]!;
										const folded = layout.isCollapsed(at.group);
										return (
											<div
												key={`header-${row.index % slots}`}
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
										<GridRow
											key={`entries-${row.index % slots}`}
											index={row.index}
											y={row.start}
											start={start}
											entries={Array.from({ length: Math.max(0, end - start) }, (_, offset) =>
												sketch ? undefined : model.entryAt(start + offset),
											)}
											roomFrom={firstColumnWithRoom(layout, row.index)}
											listId={listId}
											count={count}
											size={size}
											selection={selection}
											active={
												headerGroup < 0 && focus !== null && focus >= start && focus < end
													? focus
													: null
											}
											cut={cut}
											renaming={renaming}
											session={session}
											commands={commands}
											thumbnails={thumbnails}
											actions={cellActions}
										/>
									);
								})}
							</div>
						</div>
					)}
				</IconPictures>
			</ThumbnailPauseContext.Provider>

			{announceSelection && (
				<div className={styles.srOnly} role="status" aria-live="polite">
					{selectionText}
				</div>
			)}
		</div>
	);
}
