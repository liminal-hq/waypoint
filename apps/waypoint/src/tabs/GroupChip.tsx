// A tab group's chip in the strip: its name, colour and count, which collapses the group and renames in place
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import {
	useEffect,
	useRef,
	useState,
	type CSSProperties,
	type KeyboardEvent,
	type MouseEvent,
} from 'react';
import { t, tf, tn } from '../i18n/messages';
import { PinIcon } from '../icons/AppIcons';
import { endRename } from './groupActions';
import { GROUP_SOFT_LIMIT, type ChipItem } from './groupLayout';
import styles from './GroupChip.module.css';

interface GroupChipProps {
	item: ChipItem;
	/** This chip holds the strip's roving tab stop. */
	/** How far a drag in progress has pushed the chip aside, in pixels. */
	shift: number;
	tabStop: boolean;
	renaming: boolean;
	onFocus: () => void;
	/** Sets the group collapsed or expanded, by value so that repeating it cannot flip it back. */
	onSetCollapsed: (collapsed: boolean) => void;
	/** Commits a new name; an empty or unchanged one is ignored by the caller. */
	onRename: (name: string) => void;
	onKeyDown: (event: KeyboardEvent) => void;
	onContextMenu: (event: MouseEvent) => void;
	onStartRename: () => void;
}

export const CHIP_DOM_PREFIX = 'wp-group-chip-';

/** The element id of a group's chip button. */
export function chipDomId(group: number): string {
	return `${CHIP_DOM_PREFIX}${group}`;
}

/**
 * The chip is one focusable button in the strip's roving order (arrows reach it like a tab) but
 * not a `tab`: it names a group of tabs and reports whether they are shown (`aria-expanded`).
 * Its tabs stay `role="tab"` in the same tablist and say which group they are in through their
 * description. A pinned group's chip is an icon-sized stand-in; its name is still the accessible
 * name and the tooltip.
 */
export function GroupChip({
	item,
	shift,
	tabStop,
	renaming,
	onFocus,
	onSetCollapsed,
	onRename,
	onKeyDown,
	onContextMenu,
	onStartRename,
}: GroupChipProps) {
	const { group, members, pinned } = item;
	const count = members.length;
	const overLimit = count > GROUP_SOFT_LIMIT;
	const showCount = group.collapsed || overLimit;
	const tabs = tn('groups.tabCount', count);
	const label = [tf('groups.chip.label', { name: group.name, tabs })]
		.concat(item.containsActive && group.collapsed ? [t('groups.chip.activeInside')] : [])
		.concat(overLimit ? [tf('groups.chip.overLimit', { limit: GROUP_SOFT_LIMIT })] : [])
		.join(', ');
	const button = useRef<HTMLButtonElement | null>(null);
	// The state before the click that began a double-click toggled it, so the rename can restore it.
	const collapsedBeforeClick = useRef<boolean | null>(null);
	const wasRenaming = useRef(false);
	// Whether the name field ended from the keyboard (Enter or Escape) rather than by focus leaving.
	const endedByKey = useRef(false);

	// Ending the name field with the keyboard hands the focus back to the chip; leaving it by
	// clicking elsewhere keeps the focus where that click put it.
	useEffect(() => {
		if (wasRenaming.current && !renaming && endedByKey.current) button.current?.focus();
		if (!renaming) endedByKey.current = false;
		wasRenaming.current = renaming;
	}, [renaming]);

	return (
		<div
			role="presentation"
			className={styles.wrap}
			data-chip={group.id}
			data-pinned={pinned ? '' : undefined}
			data-colour={group.colour ?? undefined}
			data-collapsed={group.collapsed ? '' : undefined}
			data-over-limit={overLimit ? '' : undefined}
			style={
				{ '--wp-pin-index': item.pinIndex, transform: `translateX(${shift}px)` } as CSSProperties
			}
			onContextMenu={onContextMenu}
		>
			{renaming ? (
				<RenameField
					name={group.name}
					onCommit={(name, byKey) => {
						endedByKey.current = byKey;
						onRename(name);
						endRename();
					}}
					onCancel={() => {
						endedByKey.current = true;
						endRename();
					}}
				/>
			) : (
				<button
					ref={button}
					id={chipDomId(group.id)}
					type="button"
					className={styles.chip}
					aria-expanded={!group.collapsed}
					aria-label={label}
					title={showCount ? `${group.name} · ${count}` : group.name}
					tabIndex={tabStop ? 0 : -1}
					data-active-inside={item.containsActive ? '' : undefined}
					onFocus={onFocus}
					onKeyDown={(event) => {
						if (event.key === 'F2') {
							event.preventDefault();
							onStartRename();
							return;
						}
						onKeyDown(event);
					}}
					onClick={(event) => {
						// The second click of a double-click is the rename's, not another toggle.
						if (event.detail > 1) return;
						collapsedBeforeClick.current = group.collapsed;
						onSetCollapsed(!group.collapsed);
					}}
					onDoubleClick={() => {
						if (collapsedBeforeClick.current !== null) onSetCollapsed(collapsedBeforeClick.current);
						collapsedBeforeClick.current = null;
						onStartRename();
					}}
				>
					{pinned ? <PinIcon className={styles.pin} width={10} height={10} /> : null}
					<span className={styles.dot} aria-hidden="true" />
					{pinned ? null : <span className={styles.name}>{group.name}</span>}
					{showCount && !pinned ? (
						<span className={styles.count} aria-hidden="true">
							{`· ${count}`}
						</span>
					) : null}
				</button>
			)}
		</div>
	);
}

/** Enter commits, Escape cancels, and leaving the field commits what was typed. An empty name keeps the old one. */
function RenameField({
	name,
	onCommit,
	onCancel,
}: {
	name: string;
	onCommit: (name: string, byKey: boolean) => void;
	onCancel: () => void;
}) {
	const [value, setValue] = useState(name);
	const input = useRef<HTMLInputElement | null>(null);
	const done = useRef(false);
	useEffect(() => {
		input.current?.focus();
		input.current?.select();
	}, []);
	const finish = (commit: boolean, byKey: boolean) => {
		if (done.current) return;
		done.current = true;
		if (commit) onCommit(value, byKey);
		else onCancel();
	};
	return (
		<input
			ref={input}
			className={styles.input}
			value={value}
			aria-label={t('groups.rename.label')}
			size={Math.max(6, value.length + 1)}
			onChange={(event) => setValue(event.target.value)}
			onBlur={() => finish(true, false)}
			onKeyDown={(event) => {
				// The strip's own keys (arrows, Delete) must not act on the tab behind the field.
				event.stopPropagation();
				if (event.key === 'Enter') {
					event.preventDefault();
					finish(true, true);
				} else if (event.key === 'Escape') {
					event.preventDefault();
					finish(false, true);
				}
			}}
		/>
	);
}
