// The pair pill in the tab strip: the seam between a pair's tabs, its name, and its joint menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuPosition } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { SessionSnapshot } from '@liminal-hq/waypoint-protocol/generated/SessionSnapshot';
import type { TabId } from '@liminal-hq/waypoint-protocol/generated/TabId';
import { useRef, useState, type KeyboardEvent, type MouseEvent, type SyntheticEvent } from 'react';
import { t, tf } from '../i18n/messages';
import { usePairActions } from './pairActions';
import { pairOfTab } from './pairLayout';
import { PairJointMenu } from './PairJointMenu';
import { SplitGlyph } from './PairIcons';
import { locationLabel } from './tabTitle';
import styles from './PairPill.module.css';

/** A menu key press also raises a `contextmenu` event in some webviews; the second is ignored. */
const KEYBOARD_MENU_DEBOUNCE_MS = 150;

/** "Split: A and B" for a pair of any size ("A, B and C"), read from the snapshot so it never waits on a lookup. */
export function pairName(pair: Pair, snapshot: SessionSnapshot | null): string {
	const titles = pair.panes.map((pane) => {
		const tab = snapshot?.tabs.find((candidate) => candidate.id === pane);
		return tab ? locationLabel(tab.location) : '';
	});
	const last = titles[titles.length - 1] ?? '';
	return tf('pair.pill.label', { first: titles.slice(0, -1).join(', '), second: last });
}

/**
 * What the strip puts on a tab's slot so the halves of a pair read as one pill: which end of the
 * pill the slot is. Each half is still a tab of its own, with its own focus stop.
 */
export function pairSlotAttributes(
	snapshot: SessionSnapshot | null,
	tab: TabId,
): { 'data-pair'?: string; 'data-pair-end'?: 'start' | 'middle' | 'end' } {
	const pair = pairOfTab(snapshot?.pairs ?? [], tab);
	if (!pair) return {};
	const at = pair.panes.indexOf(tab);
	return {
		'data-pair': String(pair.id),
		'data-pair-end': at === 0 ? 'start' : at === pair.panes.length - 1 ? 'end' : 'middle',
	};
}

interface PairJointProps {
	pair: Pair;
	snapshot: SessionSnapshot | null;
	/** The tab whose slot holds this joint (the joint sits on its trailing edge). */
	tab: TabId;
}

/**
 * The seam between two panes' tabs, drawn on the trailing edge of the first of them (and of each
 * middle one). Right-click, the Menu key or Shift+F10 opens the pair menu (D30), double-click
 * resets the pane sizes, and a plain click does nothing. The same actions are on either half's
 * tab menu, which is the keyboard route: the joint itself is not a tab stop. Dragging the joint to
 * move both tabs arrives with the tab drag engine.
 */
export function PairJoint({ pair, snapshot, tab }: PairJointProps) {
	const actions = usePairActions();
	const button = useRef<HTMLButtonElement | null>(null);
	const lastKeyboardMenu = useRef(0);
	const [menu, setMenu] = useState<{ position: MenuPosition; keyboard: boolean } | null>(null);
	const name = pairName(pair, snapshot);
	// The joint is part of the slot it sits on; what happens to a tab there must not happen here.
	const own = (event: SyntheticEvent) => event.stopPropagation();

	if (pair.panes[pair.panes.length - 1] === tab) return null;

	const onContextMenu = (event: MouseEvent) => {
		event.preventDefault();
		event.stopPropagation();
		if (Date.now() - lastKeyboardMenu.current < KEYBOARD_MENU_DEBOUNCE_MS) return;
		setMenu({ position: { x: event.clientX, y: event.clientY }, keyboard: false });
	};

	const onKeyDown = (event: KeyboardEvent<HTMLButtonElement>) => {
		if (event.key !== 'ContextMenu' && !(event.key === 'F10' && event.shiftKey)) return;
		event.preventDefault();
		event.stopPropagation();
		const rect = event.currentTarget.getBoundingClientRect();
		lastKeyboardMenu.current = Date.now();
		setMenu({ position: { x: rect.left, y: rect.bottom }, keyboard: true });
	};

	return (
		<span className={styles.joint} role="presentation">
			<button
				ref={button}
				type="button"
				className={styles.button}
				tabIndex={-1}
				aria-label={name}
				aria-haspopup="menu"
				title={`${name} — ${t('pair.menu.label')}`}
				onPointerDown={own}
				onClick={own}
				onAuxClick={own}
				onMouseDown={own}
				onContextMenu={onContextMenu}
				onKeyDown={onKeyDown}
				onDoubleClick={(event) => {
					event.stopPropagation();
					actions.resetSizes(pair);
				}}
			>
				<SplitGlyph width={12} height={12} />
			</button>
			{menu ? (
				<PairJointMenu
					pair={pair}
					position={menu.position}
					openedWithKeyboard={menu.keyboard}
					returnFocusTo={button.current}
					onClose={() => setMenu(null)}
				/>
			) : null}
		</span>
	);
}
