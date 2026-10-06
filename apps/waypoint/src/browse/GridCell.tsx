// A row of the icon grid and its items: memoised, so a scroll redraws only the rows and cells whose items or state changed
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import {
	memo,
	type CSSProperties,
	type MouseEvent,
	type PointerEvent,
	type RefObject,
} from 'react';
import { entryDropAttributes } from '../dnd/dropTargets';
import { GitMarkView } from '../git/GitMarkView';
import { t } from '../i18n/messages';
import type { FileCommands } from '../ops/fileCommands';
import { Thumbnail } from '../thumbnails/Thumbnail';
import type { ThumbnailLoader } from '../thumbnails/thumbnailLoader';
import { entryThumbKey, wantsThumbnail } from '../thumbnails/thumbnailModel';
import styles from './GridView.module.css';
import { InlineRename } from './InlineRename';
import { leftOutNote } from './leftOut';
import { isSelected, type Selection } from './selection';
import type { ListingSession } from './useListingSession';

/**
 * What a cell does with the pointer. The grid passes one object that never changes and reads the
 * handlers of its latest render through it, so a new render of the grid does not count as a change
 * to every cell.
 */
export interface GridCellActions {
	pointerDown(event: PointerEvent, position: number, entry: Entry | undefined): void;
	click(event: MouseEvent, position: number, entry: Entry | undefined): void;
	contextMenu(event: MouseEvent, position: number, entry: Entry | undefined): void;
	doubleClick(entry: Entry | undefined): void;
	openInNewTab(entry: Entry, inNewWindow: boolean): void;
	finishRename(): void;
}

export interface GridCellProps {
	/** The element id of the cell, which `aria-activedescendant` points at. */
	id: string;
	position: number;
	/** `undefined` while the item's page has not arrived: a placeholder is drawn. */
	entry: Entry | undefined;
	/** How many items the listing has, for `aria-setsize`. */
	count: number;
	size: number;
	selected: boolean;
	active: boolean;
	/** Empty space lies under the cell, so its whole name can show without covering another. */
	room: boolean;
	cut: boolean;
	renaming: boolean;
	session: ListingSession;
	commands: FileCommands | null;
	thumbnails: ThumbnailLoader<{ key: string }> | null;
	actions: RefObject<GridCellActions>;
}

/**
 * A grid item: the icon or thumbnail, the Git chip and the name. The grid recycles its cells as it
 * scrolls (a cell keeps its element and takes the item that scrolled into its place), so nothing
 * here may keep state about the item it showed before: the rename field and the picture are keyed
 * by what they show.
 */
export const GridCell = memo(function GridCell({
	id,
	position,
	entry,
	count,
	size,
	selected,
	active,
	room,
	cut,
	renaming,
	session,
	commands,
	thumbnails,
	actions,
}: GridCellProps) {
	const { model } = session;
	return (
		<div
			id={id}
			role="option"
			className={styles.cell}
			title={
				entry?.originalPath
					? `${entry.name}\n${entry.originalPath}`
					: entry && leftOutNote(entry)
						? `${entry.name}\n${leftOutNote(entry)}`
						: entry?.name
			}
			aria-selected={selected}
			aria-setsize={count}
			aria-posinset={position + 1}
			aria-busy={entry ? undefined : true}
			data-placeholder={entry ? undefined : ''}
			data-selected={selected ? '' : undefined}
			data-room={room ? '' : undefined}
			data-cut={cut ? '' : undefined}
			data-ignored={entry?.git?.unstaged === 'ignored' ? '' : undefined}
			data-active={active ? '' : undefined}
			{...(entry ? entryDropAttributes(entry, model) : undefined)}
			onPointerDown={(event) => actions.current.pointerDown(event, position, entry)}
			onClick={(event) => actions.current.click(event, position, entry)}
			onContextMenu={(event) => actions.current.contextMenu(event, position, entry)}
			onDoubleClick={() => actions.current.doubleClick(entry)}
			onMouseDown={(event) => {
				// Stops middle-click from starting the platform's autoscroll.
				if (event.button === 1) event.preventDefault();
			}}
			onAuxClick={(event) => {
				if (event.button === 1 && entry) {
					event.preventDefault();
					actions.current.openInNewTab(entry, event.ctrlKey);
				}
			}}
		>
			{/*
			 * The same elements whether the item has arrived or not, so a placeholder that fills in (and a
			 * recycled cell that takes a placeholder) changes attributes and text, not elements: in a
			 * fling every cell does one or the other each frame.
			 */}
			<Thumbnail
				loader={entry ? thumbnails : null}
				thumbKey={entry && wantsThumbnail(entry) ? entryThumbKey(entry) : null}
				group={entry?.group ?? 'other'}
				name={entry?.name}
				iconSize={size}
				special={entry?.special}
				badge={entry?.git?.repository ? 'git' : undefined}
				className={entry ? styles.thumbnail : `${styles.thumbnail} ${styles.skeleton}`}
				iconClassName={styles.glyph}
			/>
			{entry?.git && (
				<span className={styles.gitChip}>
					<GitMarkView mark={entry.git} variant="chip" />
				</span>
			)}
			{entry && commands && renaming ? (
				<InlineRename
					key={entry.id}
					entry={entry}
					session={session}
					commands={commands}
					variant="grid"
					onFinish={() => actions.current.finishRename()}
				/>
			) : (
				<span className={entry ? styles.label : styles.srOnly}>
					{entry ? entry.name : t('browse.row.loading')}
				</span>
			)}
			{entry && leftOutNote(entry) && (
				<span className={styles.leftOut} role="img" aria-label={leftOutNote(entry) ?? undefined}>
					{'\u2298'}
				</span>
			)}
		</div>
	);
});

export interface GridRowProps extends Pick<
	GridCellProps,
	'count' | 'size' | 'session' | 'commands' | 'thumbnails' | 'actions'
> {
	/** The row's index among the grid's rows. */
	index: number;
	/** Where the row sits, in pixels from the top of the grid. */
	y: number;
	/** The position of the row's first item. */
	start: number;
	/** The row's items in order; `undefined` for one whose page has not arrived. */
	entries: ReadonlyArray<Entry | undefined>;
	/** The first column whose cells have empty space under them. */
	roomFrom: number;
	/** The prefix of the cells' element ids. */
	listId: string;
	selection: Selection;
	/** The focused position when it is in this row, `null` otherwise. */
	active: number | null;
	cut: ReadonlySet<string>;
	/** The id of the entry being renamed, anywhere in the listing. */
	renaming: number | null;
}

function sameRow(before: GridRowProps, after: GridRowProps): boolean {
	for (const key of Object.keys(after) as (keyof GridRowProps)[]) {
		if (key === 'entries') continue;
		if (before[key] !== after[key]) return false;
	}
	const a = before.entries;
	const b = after.entries;
	return a.length === b.length && a.every((entry, index) => entry === b[index]);
}

/**
 * A row of cells. The grid keys its rows by slot and its cells by column (a row that scrolls out
 * takes the items that scroll in), and a row whose items and state did not change is not drawn
 * again: a scroll step redraws only the rows that changed hands.
 */
export const GridRow = memo(function GridRow({
	index,
	y,
	start,
	entries,
	roomFrom,
	listId,
	selection,
	active,
	cut,
	renaming,
	...shared
}: GridRowProps) {
	// A selected name shows whole over the cell below it, so the row that holds one is raised above the
	// rows after it. A name with room under it overflows into empty space and needs no lift (lifting the
	// last row would paint it over the selected name above). Raised rows stack in index order, as they
	// would in document order: the grid draws its rows in slot order.
	const lifted = entries.some((entry) => entry !== undefined && isSelected(selection, entry.id));
	return (
		<div
			role="presentation"
			className={styles.row}
			data-lifted={lifted ? '' : undefined}
			style={{ '--wp-row-y': `${y}px`, zIndex: lifted ? 1 + index : undefined } as CSSProperties}
		>
			{entries.map((entry, offset) => {
				const position = start + offset;
				return (
					<GridCell
						// By column: a recycled row keeps its cells and hands them the items now in it.
						key={offset}
						id={`${listId}-item-${position}`}
						position={position}
						entry={entry}
						selected={entry ? isSelected(selection, entry.id) : false}
						active={active === position}
						room={offset >= roomFrom}
						cut={entry ? cut.has(entry.name) : false}
						renaming={entry !== undefined && renaming === entry.id}
						{...shared}
					/>
				);
			})}
		</div>
	);
}, sameRow);
