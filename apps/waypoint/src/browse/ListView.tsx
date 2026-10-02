// The virtualised file list: paged rows, a sortable header, keyboard and pointer selection, live updates
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SortKey } from '@liminal-hq/waypoint-protocol/generated/SortKey';
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
import { t, tf, tn, type MessageId } from '../i18n/messages';
import { useCutNames } from '../ops/ClipboardContext';
import { useFileCommands } from '../ops/FileCommandsContext';
import { Thumbnail } from '../thumbnails/Thumbnail';
import { devicePixelRatio, useEntryThumbnailLoader } from '../thumbnails/ThumbnailsContext';
import {
	entryThumbKey,
	listShowsThumbnails,
	listThumbnailPixels,
	thumbSizeFor,
	wantsThumbnail,
} from '../thumbnails/thumbnailModel';
import { useViewportThumbnails } from '../thumbnails/useViewportThumbnails';
import { FileIcon } from './FileIcon';
import { InlineRename } from './InlineRename';
import styles from './ListView.module.css';
import { formatModified, formatSize } from './format';
import { useHourCycle } from './TimeFormatContext';
import { ErrorState, ListingGate, MessageState } from './ListingGate';
import type { ListingSession, SessionState } from './useListingSession';
import { useListingSession } from './useListingSession';
import { groupCount, groupLabel, groupTitle } from './groupHeader';
import { GroupLayout, groupId } from './groupLayout';
import { firstTarget } from './groupNav';
import { mapPosition, isReset } from './patch';
import { DEFAULT_ROW_HEIGHT, measureRowHeight, visibleRows } from './scrollCap';
import { isSelected, selectedCount } from './selection';
import { useVfsClient } from './VfsClientContext';
import {
	useListInteractions,
	type MenuRequest,
	type OpenHandler,
	type OpenInNewHandler,
} from './useListInteractions';

/** Rows drawn beyond the viewport on each side, so a fast scroll meets rows, not gaps. */
const OVERSCAN = 12;

/** A column of the header. One with no `sort` is not a sort key (where an item was trashed from). */
interface Column {
	/** Names the column for the row cells and the container queries that hide it when narrow. */
	id: 'name' | 'size' | 'modified' | 'kind' | 'original' | 'deleted';
	sort?: SortKey;
	label: MessageId;
}

const FOLDER_COLUMNS: Column[] = [
	{ id: 'name', sort: 'name', label: 'browse.column.name' },
	{ id: 'size', sort: 'size', label: 'browse.column.size' },
	{ id: 'modified', sort: 'modified', label: 'browse.column.modified' },
	{ id: 'kind', sort: 'kind', label: 'browse.column.kind' },
];

/** The Trash shows where each item came from and when it was trashed in place of Modified and Kind. */
const TRASH_COLUMNS: Column[] = [
	{ id: 'name', sort: 'name', label: 'browse.column.name' },
	{ id: 'original', label: 'browse.column.original' },
	{ id: 'deleted', sort: 'deleted', label: 'browse.column.deleted' },
	{ id: 'size', sort: 'size', label: 'browse.column.size' },
];

interface ListViewProps {
	/** The folder to list. Changing it opens a new listing and discards the old one's selection. */
	location: Location;
	/** Enter and double-click on an entry. Navigation and opening belong to the caller. */
	onOpen?: (entry: Entry, handle: ListingHandle) => void;
}

/** Opens the listing of `location` itself; a host that manages listings uses `ListingView`. */
export function ListView({ location, onOpen }: ListViewProps) {
	const client = useVfsClient();
	const state = useListingSession(client, location);
	return <ListingView state={state} onOpen={onOpen} />;
}

interface ListingViewProps {
	state: SessionState;
	onOpen?: OpenHandler | undefined;
	onOpenInNewTab?: OpenInNewHandler | undefined;
	/** Right-click, the menu key and Shift+F10. The host renders the menu. */
	onMenu?: ((request: MenuRequest) => void) | undefined;
	/** Whether the list announces selection changes itself; a host with a status bar does that. */
	announceSelection?: boolean | undefined;
}

/** The list for a listing that is opening, failed or ready. */
export function ListingView({
	state,
	onOpen,
	onOpenInNewTab,
	onMenu,
	announceSelection = true,
}: ListingViewProps) {
	return (
		<ListingGate state={state}>
			{(session) => (
				<ListingBody
					key={session.model.handle}
					session={session}
					onOpen={onOpen}
					onOpenInNewTab={onOpenInNewTab}
					onMenu={onMenu}
					announceSelection={announceSelection}
				/>
			)}
		</ListingGate>
	);
}

interface ListingBodyProps {
	session: ListingSession;
	onOpen: OpenHandler | undefined;
	onOpenInNewTab: OpenInNewHandler | undefined;
	onMenu: ((request: MenuRequest) => void) | undefined;
	announceSelection: boolean;
}

function ListingBody({
	session,
	onOpen,
	onOpenInNewTab,
	onMenu,
	announceSelection,
}: ListingBodyProps) {
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
	const hourCycle = useHourCycle();

	const listId = useId();
	const scroller = useRef<HTMLDivElement | null>(null);
	const listbox = useRef<HTMLDivElement | null>(null);
	const [rowHeight, setRowHeight] = useState(DEFAULT_ROW_HEIGHT);

	// The scroll position as the top of the viewport and the row under it. It is the anchor patches
	// keep steady: see `followPatches` below.
	const anchor = useRef({ top: 0, position: 0 });
	const reanchor = useRef(false);

	// A header takes a row of its own, so the rows are the entries and the headers between them.
	const layout = useMemo(
		() => new GroupLayout(model.groups, collapsed, model.count),
		[model.groups, collapsed, model.count],
	);
	const layoutRef = useRef(layout);
	layoutRef.current = layout;
	const { shown, hidden } = visibleRows(layout.rowCount, rowHeight);

	const virtualizer = useVirtualizer({
		count: shown,
		getScrollElement: () => scroller.current,
		estimateSize: () => rowHeight,
		overscan: OVERSCAN,
	});
	const items = virtualizer.getVirtualItems();
	const first = items[0]?.index ?? 0;
	const last = items[items.length - 1]?.index ?? 0;

	// Rows tall enough for a picture (the Roomy density, touch mode) show thumbnails; the others keep icons.
	const withPictures = listShowsThumbnails(rowHeight);
	const pictureSize = listThumbnailPixels(rowHeight);
	const thumbnails = useEntryThumbnailLoader(
		model.handle,
		withPictures ? thumbSizeFor(pictureSize, devicePixelRatio()) : null,
	);
	const inView = virtualizer.range;
	useViewportThumbnails({
		loader: thumbnails,
		first: inView?.startIndex ?? first,
		last: inView?.endIndex ?? last,
		count: shown,
		version,
		itemAt: (position) => {
			const entry = model.entryAt(position);
			return entry && wantsThumbnail(entry) ? { key: entryThumbKey(entry), id: entry.id } : null;
		},
	});

	// The scroller only exists once there are rows (or a scan under way), so measure when it appears,
	// not just on mount: a listing that opens empty would otherwise keep the default height.
	const scanning = model.phase === 'scanning' || model.phase === 'rescanning';
	const empty = model.count === 0 && !scanning;
	const trash = model.layout === 'trash';
	const columns = trash ? TRASH_COLUMNS : FOLDER_COLUMNS;
	useLayoutEffect(() => {
		if (scroller.current) setRowHeight(measureRowHeight(scroller.current));
	}, [empty]);

	useEffect(() => {
		virtualizer.measure();
	}, [rowHeight, virtualizer]);

	// Fetch what is on screen plus a page either side; re-run when the model changes so pages that
	// a patch invalidated are requested again. A folded group between two visible rows is not
	// fetched: the entries on either side are asked for separately.
	useEffect(() => {
		if (items.length === 0) return;
		if (!layout.grouped) return model.ensure(first, last);
		let run: [number, number] | null = null;
		for (let index = first; index <= last; index++) {
			const row = layout.rowAt(index);
			if (row.kind !== 'entries') continue;
			if (run && row.first === run[1] + 1) run[1] = row.first;
			else {
				if (run) model.ensure(run[0], run[1]);
				run = [row.first, row.first];
			}
		}
		if (run) model.ensure(run[0], run[1]);
	}, [model, layout, first, last, items.length, version]);

	// Scroll anchoring. The model tells this, synchronously and before React renders, how a patch
	// moved entries. The row at the top of the viewport is followed through the patch, and the
	// scroll offset moves by the same number of rows, so entries inserted or removed above the
	// viewport do not shift what the person is looking at. A list scrolled to the very top stays
	// there, because new entries at the top should be seen.
	useEffect(
		() =>
			model.onPatch((report) => {
				const current = anchor.current;
				if (current.top === 0 || isReset(report.ops)) return;
				const { position } = mapPosition(current.position, report.ops);
				// Where that row stood, and where it stands now that the groups have moved with the patch.
				const before = layoutRef.current;
				const after = new GroupLayout(model.groups, store.getState().collapsed, report.count);
				const above =
					current.top - before.offsetOfRow(before.rowNear(current.position), rowHeight, rowHeight);
				const row = after.rowNear(position);
				const top = Math.max(0, after.offsetOfRow(row, rowHeight, rowHeight) + above);
				// Headers can come and go above the row without the row's own position moving.
				if (top === current.top && position === current.position) return;
				anchor.current = { top, position };
				reanchor.current = true;
			}),
		[model, store, rowHeight],
	);

	useLayoutEffect(() => {
		const element = scroller.current;
		if (!reanchor.current || !element) return;
		reanchor.current = false;
		element.scrollTop = anchor.current.top;
		// The virtualiser learns the new offset from a scroll event, which would arrive after this
		// frame has painted the old window of rows against the new offset. Deliver it now.
		element.dispatchEvent(new Event('scroll'));
	}, [version]);

	// A tab that returns to this listing finds the scroll position it left.
	useLayoutEffect(() => {
		const element = scroller.current;
		if (!element || session.view.scrollTop <= 0) return;
		element.scrollTop = session.view.scrollTop;
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
		session.view.scrollTop = top;
		anchor.current = {
			top,
			position: layoutRef.current.positionAtRow(Math.floor(top / rowHeight)),
		};
	};

	const count = model.count;
	const selectionText = touched
		? selectedCount(selection, count) === 0
			? t('browse.selection.none')
			: tn('browse.selection', selectedCount(selection, count))
		: '';

	const scrollToRow = (position: number) =>
		virtualizer.scrollToIndex(Math.max(0, Math.min(shown - 1, layout.rowNear(position))), {
			align: 'auto',
		});
	const scrollToHeader = (group: number) =>
		virtualizer.scrollToIndex(Math.max(0, Math.min(shown - 1, layout.rowOfHeader(group))), {
			align: 'auto',
		});

	// A command that made or found an entry asks for it to be brought into sight.
	useEffect(() => {
		if (scrollRequest) scrollToRow(scrollRequest.position);
	}, [scrollRequest]);

	const page = () => Math.max(1, Math.floor((scroller.current?.clientHeight ?? 0) / rowHeight) - 1);
	const interactions = useListInteractions({
		session,
		itemId: (position) => `${listId}-row-${position}`,
		shown,
		scrollTo: scrollToRow,
		layout,
		pageRows: page,
		scrollToHeader,
		onOpen,
		onMenu,
		thumbnailOf: (entry) => thumbnails?.urlOf(entryThumbKey(entry)) ?? null,
		move: (key, from, last) => {
			switch (key) {
				case 'ArrowDown':
					return from === null ? 0 : from + 1;
				case 'ArrowUp':
					return from === null ? 0 : from - 1;
				case 'PageDown':
					return from === null ? 0 : from + page();
				case 'PageUp':
					return from === null ? 0 : from - page();
				case 'Home':
					return 0;
				case 'End':
					return last;
				default:
					return null;
			}
		},
	});
	const {
		onKeyDown,
		onItemClick,
		onItemPointerDown,
		onItemDoubleClick,
		onItemContextMenu,
		onBackgroundContextMenu,
		onHeaderClick,
	} = interactions;

	const onSort = (key: SortKey) => {
		const { sort } = model;
		void model.setSort({ ...sort, key, descending: sort.key === key ? !sort.descending : false });
	};

	if (model.error) return <ErrorState error={model.error} />;

	const headerGroup =
		focusHeader === null ? -1 : model.groups.findIndex((run) => groupId(run.key) === focusHeader);
	const activeId =
		headerGroup >= 0
			? `${listId}-group-${headerGroup}`
			: focus === null
				? undefined
				: `${listId}-row-${focus}`;

	return (
		<div
			className={styles.view}
			data-layout={model.layout}
			style={withPictures ? ({ '--wp-list-icon': `${pictureSize}px` } as CSSProperties) : undefined}
		>
			<div
				className={`${styles.columns} ${styles.header}`}
				role="group"
				aria-label={t('browse.columns.label')}
			>
				{columns.map((column, index) => {
					const { sort } = column;
					if (sort === undefined) {
						return (
							<span
								key={column.id}
								className={`${styles.headerButton} ${styles.headerStatic}`}
								data-column={column.id}
							>
								<span>{t(column.label)}</span>
							</span>
						);
					}
					const active = model.sort.key === sort;
					return (
						<button
							key={column.id}
							type="button"
							className={styles.headerButton}
							data-column={column.id}
							data-sorted={
								active ? (model.sort.descending ? 'descending' : 'ascending') : undefined
							}
							onClick={() => onSort(sort)}
						>
							{index === 0 && <span className={styles.iconSpacer} aria-hidden="true" />}
							<span>{t(column.label)}</span>
							{active && (
								<>
									<span className={styles.sortMark} aria-hidden="true" />
									<span className={styles.srOnly}>
										{t(model.sort.descending ? 'browse.sort.descending' : 'browse.sort.ascending')}
									</span>
								</>
							)}
						</button>
					);
				})}
			</div>

			{scanning && (
				<div className={styles.notice} role="status" data-notice="scanning">
					{tf('browse.scanning', { count: new Intl.NumberFormat().format(model.scanned) })}
				</div>
			)}
			{hidden > 0 && (
				<div className={styles.notice} role="status" data-notice="capped">
					{tf('browse.capped', {
						shown: new Intl.NumberFormat().format(shown),
						total: new Intl.NumberFormat().format(count),
					})}
				</div>
			)}

			{empty ? (
				<div className={styles.emptyArea} onContextMenu={onBackgroundContextMenu}>
					<MessageState role="status" data-state="empty">
						{t(trash ? 'trash.view.empty' : 'browse.empty')}
					</MessageState>
				</div>
			) : (
				<div
					ref={scroller}
					className={styles.scroller}
					{...{ [SCROLL_ATTRIBUTE]: '' }}
					onScroll={recordAnchor}
					onContextMenu={onBackgroundContextMenu}
				>
					<div
						ref={listbox}
						role="listbox"
						tabIndex={0}
						aria-label={t('browse.list.label')}
						aria-multiselectable="true"
						aria-rowcount={layout.grouped ? undefined : shown}
						aria-activedescendant={activeId}
						className={styles.list}
						style={{ '--wp-list-height': `${virtualizer.getTotalSize()}px` } as CSSProperties}
						onKeyDown={onKeyDown}
						onFocus={() => {
							const state = store.getState();
							if (state.focus !== null || state.focusHeader !== null || count === 0) return;
							const first = firstTarget(layout);
							if ('header' in first) state.focusGroup(groupId(model.groups[first.header]!.key));
							else state.moveTo(first.position, false);
						}}
					>
						{items.map((item) => {
							const at = layout.rowAt(item.index);
							if (at.kind === 'header') {
								const run = model.groups[at.group]!;
								const folded = layout.isCollapsed(at.group);
								return (
									<div
										key={item.key}
										id={`${listId}-group-${at.group}`}
										role="group"
										aria-label={groupLabel(run.key, run.count, folded)}
										className={`${styles.groupHeader}`}
										style={{ '--wp-row-y': `${item.start}px` } as CSSProperties}
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
							const position = at.first;
							const entry = model.entryAt(position);
							const selected = entry ? isSelected(selection, entry.id) : false;
							return (
								<div
									key={item.key}
									id={`${listId}-row-${position}`}
									role="option"
									className={`${styles.columns} ${styles.row}`}
									style={{ '--wp-row-y': `${item.start}px` } as CSSProperties}
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
											<span className={styles.name}>
												{withPictures ? (
													<Thumbnail
														loader={thumbnails}
														thumbKey={wantsThumbnail(entry) ? entryThumbKey(entry) : null}
														group={entry.group}
														className={styles.thumbnail}
													/>
												) : (
													<FileIcon group={entry.group} />
												)}
												{commands && renaming === entry.id ? (
													<InlineRename
														entry={entry}
														session={session}
														commands={commands}
														variant="list"
														onFinish={() => listbox.current?.focus()}
													/>
												) : (
													<span className={styles.nameText}>{entry.name}</span>
												)}
											</span>
											{trash ? (
												<>
													<span
														className={styles.cell}
														data-column="original"
														title={entry.originalPath ?? undefined}
													>
														{entry.originalPath ?? t('browse.value.none')}
													</span>
													<span className={styles.cell} data-column="deleted">
														{entry.deletedMs == null
															? t('browse.value.none')
															: formatModified(entry.deletedMs, undefined, hourCycle)}
													</span>
													<span className={styles.cell} data-column="size">
														{entry.size === null ? t('browse.value.none') : formatSize(entry.size)}
													</span>
												</>
											) : (
												<>
													<span className={styles.cell} data-column="size">
														{entry.size === null ? t('browse.value.none') : formatSize(entry.size)}
													</span>
													<span className={styles.cell} data-column="modified">
														{entry.modifiedMs === null
															? t('browse.value.none')
															: formatModified(entry.modifiedMs, undefined, hourCycle)}
													</span>
													<span className={styles.cell} data-column="kind">
														{t(`browse.group.${entry.group}`)}
													</span>
												</>
											)}
										</>
									) : (
										<>
											<span className={styles.name}>
												<span className={styles.skeleton} />
												<span className={styles.srOnly}>{t('browse.row.loading')}</span>
											</span>
										</>
									)}
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
