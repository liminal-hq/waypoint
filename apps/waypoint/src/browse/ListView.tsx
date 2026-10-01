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
	useRef,
	useState,
	useSyncExternalStore,
	type CSSProperties,
} from 'react';
import { useStore } from 'zustand';
import { t, tf, tn, type MessageId } from '../i18n/messages';
import { FileIcon } from './FileIcon';
import styles from './ListView.module.css';
import { formatModified, formatSize } from './format';
import { ErrorState, ListingGate, MessageState } from './ListingGate';
import type { ListingSession, SessionState } from './useListingSession';
import { useListingSession } from './useListingSession';
import { mapPosition, isReset } from './patch';
import { DEFAULT_ROW_HEIGHT, measureRowHeight, visibleRows } from './scrollCap';
import { isSelected, selectedCount } from './selection';
import { useVfsClient } from './VfsClientContext';
import { useListInteractions, type MenuRequest, type OpenHandler } from './useListInteractions';

/** Rows drawn beyond the viewport on each side, so a fast scroll meets rows, not gaps. */
const OVERSCAN = 12;

const COLUMNS: Array<{ key: SortKey; label: MessageId }> = [
	{ key: 'name', label: 'browse.column.name' },
	{ key: 'size', label: 'browse.column.size' },
	{ key: 'modified', label: 'browse.column.modified' },
	{ key: 'kind', label: 'browse.column.kind' },
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
	onOpenInNewTab?: OpenHandler | undefined;
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
	onOpenInNewTab: OpenHandler | undefined;
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

	const listId = useId();
	const scroller = useRef<HTMLDivElement | null>(null);
	const [rowHeight, setRowHeight] = useState(DEFAULT_ROW_HEIGHT);

	// The scroll position as the top of the viewport and the row under it. It is the anchor patches
	// keep steady: see `followPatches` below.
	const anchor = useRef({ top: 0, position: 0 });
	const reanchor = useRef(false);

	const { shown, hidden } = visibleRows(model.count, rowHeight);

	const virtualizer = useVirtualizer({
		count: shown,
		getScrollElement: () => scroller.current,
		estimateSize: () => rowHeight,
		overscan: OVERSCAN,
	});
	const items = virtualizer.getVirtualItems();
	const first = items[0]?.index ?? 0;
	const last = items[items.length - 1]?.index ?? 0;

	// The scroller only exists once there are rows (or a scan under way), so measure when it appears,
	// not just on mount: a listing that opens empty would otherwise keep the default height.
	const scanning = model.phase === 'scanning' || model.phase === 'rescanning';
	const empty = model.count === 0 && !scanning;
	useLayoutEffect(() => {
		if (scroller.current) setRowHeight(measureRowHeight(scroller.current));
	}, [empty]);

	useEffect(() => {
		virtualizer.measure();
	}, [rowHeight, virtualizer]);

	// Fetch what is on screen plus a page either side; re-run when the model changes so pages that
	// a patch invalidated are requested again.
	useEffect(() => {
		if (items.length > 0) model.ensure(first, last);
	}, [model, first, last, items.length, version]);

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
				if (position === current.position) return;
				anchor.current = {
					top: Math.max(0, current.top + (position - current.position) * rowHeight),
					position,
				};
				reanchor.current = true;
			}),
		[model, rowHeight],
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
		anchor.current = { top, position: Math.floor(top / rowHeight) };
	};

	const count = model.count;
	const selectionText = touched
		? selectedCount(selection, count) === 0
			? t('browse.selection.none')
			: tn('browse.selection', selectedCount(selection, count))
		: '';

	const scrollToRow = (position: number) =>
		virtualizer.scrollToIndex(Math.max(0, Math.min(shown - 1, position)), { align: 'auto' });

	const page = () => Math.max(1, Math.floor((scroller.current?.clientHeight ?? 0) / rowHeight) - 1);
	const interactions = useListInteractions({
		session,
		itemId: (position) => `${listId}-row-${position}`,
		shown,
		scrollTo: scrollToRow,
		onOpen,
		onMenu,
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
	const { onKeyDown, onItemClick, onItemContextMenu, onBackgroundContextMenu } = interactions;

	const onSort = (key: SortKey) => {
		const { sort } = model;
		void model.setSort({ ...sort, key, descending: sort.key === key ? !sort.descending : false });
	};

	if (model.error) return <ErrorState error={model.error} />;

	const activeId = focus === null ? undefined : `${listId}-row-${focus}`;

	return (
		<div className={styles.view}>
			<div
				className={`${styles.columns} ${styles.header}`}
				role="group"
				aria-label={t('browse.columns.label')}
			>
				{COLUMNS.map((column, index) => {
					const active = model.sort.key === column.key;
					return (
						<button
							key={column.key}
							type="button"
							className={styles.headerButton}
							data-column={column.key}
							data-sorted={
								active ? (model.sort.descending ? 'descending' : 'ascending') : undefined
							}
							onClick={() => onSort(column.key)}
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
						aria-rowcount={shown}
						aria-activedescendant={activeId}
						className={styles.list}
						style={{ '--wp-list-height': `${virtualizer.getTotalSize()}px` } as CSSProperties}
						onKeyDown={onKeyDown}
						onFocus={() => {
							if (store.getState().focus === null && count > 0) store.getState().moveTo(0, false);
						}}
					>
						{items.map((item) => {
							const entry = model.entryAt(item.index);
							const selected = entry ? isSelected(selection, entry.id) : false;
							return (
								<div
									key={item.key}
									id={`${listId}-row-${item.index}`}
									role="option"
									className={`${styles.columns} ${styles.row}`}
									style={{ '--wp-row-y': `${item.start}px` } as CSSProperties}
									aria-selected={selected}
									aria-setsize={shown}
									aria-posinset={item.index + 1}
									aria-busy={entry ? undefined : true}
									data-placeholder={entry ? undefined : ''}
									data-selected={selected ? '' : undefined}
									data-active={focus === item.index ? '' : undefined}
									onClick={(event) => onItemClick(event, item.index, entry)}
									onContextMenu={(event) => onItemContextMenu(event, item.index, entry)}
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
											<span className={styles.name}>
												<FileIcon group={entry.group} />
												<span className={styles.nameText}>{entry.name}</span>
											</span>
											<span className={styles.cell}>
												{entry.size === null ? t('browse.value.none') : formatSize(entry.size)}
											</span>
											<span className={styles.cell}>
												{entry.modifiedMs === null
													? t('browse.value.none')
													: formatModified(entry.modifiedMs)}
											</span>
											<span className={styles.cell}>{t(`browse.group.${entry.group}`)}</span>
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
