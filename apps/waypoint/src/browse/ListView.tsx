// The virtualised file list: paged rows, a sortable header, and states for loading, scanning and errors
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { SortKey } from '@liminal-hq/waypoint-protocol/generated/SortKey';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
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
import { t, tf, type MessageId } from '../i18n/messages';
import { FileIcon } from './FileIcon';
import styles from './ListView.module.css';
import { formatModified, formatSize } from './format';
import type { ListingSession } from './useListingSession';
import { useListingSession } from './useListingSession';
import { DEFAULT_ROW_HEIGHT, measureRowHeight, visibleRows } from './scrollCap';
import { useVfsClient } from './VfsClientContext';

/** Rows drawn beyond the viewport on each side, so a fast scroll meets rows, not gaps. */
const OVERSCAN = 12;

const COLUMNS: Array<{ key: SortKey; label: MessageId }> = [
	{ key: 'name', label: 'browse.column.name' },
	{ key: 'size', label: 'browse.column.size' },
	{ key: 'modified', label: 'browse.column.modified' },
	{ key: 'kind', label: 'browse.column.kind' },
];

interface ListViewProps {
	/** The folder to list. Changing it opens a new listing. */
	location: Location;
}

export function ListView({ location }: ListViewProps) {
	const client = useVfsClient();
	const state = useListingSession(client, location);
	if (state.status === 'opening') {
		return (
			<div className={styles.message} role="status">
				{t('browse.opening')}
			</div>
		);
	}
	if (state.status === 'error') return <ErrorState error={state.error} />;
	return <ListingBody key={state.session.model.handle} session={state.session} />;
}

function errorMessages(error: VfsError): { title: MessageId; detail: MessageId } {
	switch (error.kind) {
		case 'notFound':
			return { title: 'browse.error.notFound.title', detail: 'browse.error.notFound.detail' };
		case 'permissionDenied':
			return {
				title: 'browse.error.permissionDenied.title',
				detail: 'browse.error.permissionDenied.detail',
			};
		case 'notADirectory':
			return {
				title: 'browse.error.notADirectory.title',
				detail: 'browse.error.notADirectory.detail',
			};
		default:
			return { title: 'browse.error.other.title', detail: 'browse.error.other.detail' };
	}
}

function ErrorState({ error }: { error: VfsError }) {
	const { title, detail } = errorMessages(error);
	const location =
		error.kind === 'notFound' || error.kind === 'permissionDenied' || error.kind === 'notADirectory'
			? error.location.display
			: '';
	return (
		<div className={styles.message} role="alert" data-error={error.kind}>
			<h2 className={styles.messageTitle}>{t(title)}</h2>
			<p className={styles.messageDetail}>{tf(detail, { location })}</p>
		</div>
	);
}

interface ListingBodyProps {
	session: ListingSession;
}

function ListingBody({ session }: ListingBodyProps) {
	const { model } = session;
	const version = useSyncExternalStore(model.subscribe, model.getVersion);

	const listId = useId();
	const scroller = useRef<HTMLDivElement | null>(null);
	const [rowHeight, setRowHeight] = useState(DEFAULT_ROW_HEIGHT);

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

	const count = model.count;
	const onSort = (key: SortKey) => {
		const { sort } = model;
		void model.setSort({ ...sort, key, descending: sort.key === key ? !sort.descending : false });
	};

	if (model.error) return <ErrorState error={model.error} />;

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
				<div className={styles.message} role="status" data-state="empty">
					{t('browse.empty')}
				</div>
			) : (
				<div ref={scroller} className={styles.scroller}>
					<div
						role="listbox"
						tabIndex={0}
						aria-label={t('browse.list.label')}
						aria-multiselectable="true"
						aria-rowcount={shown}
						className={styles.list}
						style={{ '--wp-list-height': `${virtualizer.getTotalSize()}px` } as CSSProperties}
					>
						{items.map((item) => {
							const entry = model.entryAt(item.index);
							return (
								<div
									key={item.key}
									id={`${listId}-row-${item.index}`}
									role="option"
									className={`${styles.columns} ${styles.row}`}
									style={{ '--wp-row-y': `${item.start}px` } as CSSProperties}
									aria-setsize={shown}
									aria-posinset={item.index + 1}
									aria-busy={entry ? undefined : true}
									data-placeholder={entry ? undefined : ''}
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
		</div>
	);
}
