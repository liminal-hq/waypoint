// The items of the pair joint menu and the tab menu's split section, and what choosing each one does
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem, SelectableMenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Pair } from '@liminal-hq/waypoint-protocol/generated/Pair';
import type { PairLayout } from '@liminal-hq/waypoint-protocol/generated/PairLayout';
import type { TabColour } from '@liminal-hq/waypoint-protocol/generated/TabColour';
import type { TabSnapshot } from '@liminal-hq/waypoint-protocol/generated/TabSnapshot';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import { createElement } from 'react';
import { t } from '../i18n/messages';
import type { PairActions } from './pairActions';
import { colourMessageId, TAB_COLOURS } from './tabColours';
import { TabColourSwatch } from './TabColourSwatch';
import { runWindowMoveChoice, windowMoveItems } from './windowMoveMenu';
import { locationLabel } from './tabTitle';

const PAIR_PREFIX = 'pair:';
const LAYOUT_PREFIX = 'pair:layout:';
const COLOUR_PREFIX = 'pair:colour:';
const SPLIT_WITH_PREFIX = 'splitWith:';
const TAB_COLOUR_PREFIX = 'colour:';

const LAYOUTS: Array<{
	layout: PairLayout;
	label: 'pair.layout.sideBySide' | 'pair.layout.stacked';
}> = [
	{ layout: 'sideBySide', label: 'pair.layout.sideBySide' },
	{ layout: 'stacked', label: 'pair.layout.stacked' },
];

function layoutItem(pair: Pair): MenuItem {
	return {
		type: 'submenu',
		id: 'pair:layout',
		label: t('pair.menu.layout'),
		items: LAYOUTS.map(({ layout, label }) => ({
			type: 'checkbox' as const,
			id: `${LAYOUT_PREFIX}${layout}`,
			label: t(label),
			checked: pair.layout === layout,
		})),
	};
}

/** The colour every pane shares, or `null` when none or when they differ. */
function sharedColour(pair: Pair, tabs: readonly TabSnapshot[]): TabColour | null {
	const colours = pair.panes.map((pane) => tabs.find((tab) => tab.id === pane)?.colour ?? null);
	const [first] = colours;
	return first !== undefined && colours.every((colour) => colour === first) ? first : null;
}

/**
 * The menu on the pair's joint (D30). Sync Navigation and Compare Folders are parked, so they are
 * not listed. `groupItems` is where Add to Group goes once the group menu exists; the joint menu
 * does not depend on it.
 */
export function pairJointItems(
	pair: Pair,
	tabs: readonly TabSnapshot[],
	groupItems: MenuItem[] = [],
	others: readonly WindowSummary[] = [],
): MenuItem[] {
	const lead = tabs.find((tab) => tab.id === pair.panes[0]);
	const colour = sharedColour(pair, tabs);
	return [
		{ type: 'action', id: 'pair:separate', label: t('pair.menu.separate') },
		{ type: 'action', id: 'pair:swap', label: t('pair.menu.swap') },
		layoutItem(pair),
		{ type: 'action', id: 'pair:resetSizes', label: t('pair.menu.resetSizes') },
		{ type: 'separator', id: 'sep-pair-look' },
		{
			type: 'action',
			id: lead?.pinned ? 'pair:unpin' : 'pair:pin',
			label: t(lead?.pinned ? 'tabs.menu.unpin' : 'tabs.menu.pin'),
		},
		{
			type: 'submenu',
			id: 'pair:colour',
			label: t('tabs.menu.colour'),
			items: [
				{
					type: 'checkbox',
					id: `${COLOUR_PREFIX}none`,
					label: t('tabs.colour.none'),
					checked: colour === null,
				},
				...TAB_COLOURS.map((name) => ({
					type: 'checkbox' as const,
					id: `${COLOUR_PREFIX}${name}`,
					label: t(colourMessageId(name)),
					checked: colour === name,
					icon: createElement(TabColourSwatch, { colour: name }),
				})),
			],
		},
		// Extension point: the group menu's "Add to Group ▸" items are passed in here.
		...groupItems,
		{ type: 'separator', id: 'sep-pair-copy' },
		{ type: 'action', id: 'pair:duplicate', label: t('pair.menu.duplicate') },
		...windowMoveItems(others, {
			newWindow: t('pair.menu.moveToNewWindow'),
			toWindow: t('pair.menu.moveToWindow'),
		}),
		{ type: 'separator', id: 'sep-pair-close' },
		{ type: 'action', id: 'pair:closeBoth', label: t('pair.menu.closeBoth') },
	];
}

/**
 * The split items of the tab menu: Split With ▸ on a tab that is alone (the non-pointer path for
 * holding a tab over another), and the pair's own actions on either half of a pair. Pin and
 * Colour stay the tab menu's own: pinning carries the pair, and `runPairMenuItem` colours both.
 */
export function pairTabItems(
	tab: TabSnapshot,
	pair: Pair | undefined,
	tabs: readonly TabSnapshot[],
	pairs: readonly Pair[],
): MenuItem[] {
	if (pair) {
		return [
			{ type: 'separator', id: 'sep-pair' },
			{ type: 'action', id: 'pair:separate', label: t('pair.menu.separateTabs') },
			{ type: 'action', id: 'pair:swap', label: t('pair.menu.swap') },
			layoutItem(pair),
			{ type: 'action', id: 'pair:resetSizes', label: t('pair.menu.resetSizes') },
			{ type: 'action', id: 'pair:closeBoth', label: t('pair.menu.closeBoth') },
		];
	}
	const candidates = tabs.filter(
		(other) =>
			other.id !== tab.id && !pairs.some((candidate) => candidate.panes.includes(other.id)),
	);
	return [
		{ type: 'separator', id: 'sep-pair' },
		{
			type: 'submenu',
			id: 'splitWith',
			label: t('pair.menu.splitWith'),
			disabled: candidates.length === 0,
			items: candidates.map((other) => ({
				type: 'action' as const,
				id: `${SPLIT_WITH_PREFIX}${other.id}`,
				label: locationLabel(other.location),
			})),
		},
	];
}

/** Puts `section` just before the item whose id is `beforeId` (at the end when there is none). */
export function insertBefore(items: MenuItem[], beforeId: string, section: MenuItem[]): MenuItem[] {
	const at = items.findIndex((item) => item.id === beforeId);
	if (at < 0) return [...items, ...section];
	return [...items.slice(0, at), ...section, ...items.slice(at)];
}

function colourFrom(name: string): TabColour | null {
	return TAB_COLOURS.find((candidate) => candidate === name) ?? null;
}

interface RunContext {
	actions: PairActions;
	pair: Pair | undefined;
	/** The other windows the pair can move to (the joint menu's Move to Window items). */
	others?: readonly WindowSummary[];
	/** The tab the menu was opened on, when it was a tab's menu. */
	tab?: TabSnapshot;
}

/** Runs a pair item; false when `item` is not one, so the tab menu handles it. */
export function runPairMenuItem(
	item: SelectableMenuItem,
	{ actions, pair, tab, others = [] }: RunContext,
): boolean {
	// Only the joint menu carries window items; the tab menu's own move one tab.
	if (
		pair &&
		!tab &&
		runWindowMoveChoice(item, others, {
			toNewWindow: () => actions.moveToNewWindow(pair),
			toWindow: (target) => actions.moveToWindow(pair, target),
		})
	) {
		return true;
	}
	if (item.id.startsWith(SPLIT_WITH_PREFIX)) {
		const other = Number(item.id.slice(SPLIT_WITH_PREFIX.length));
		if (tab) actions.splitWith(tab.id, other);
		return true;
	}
	// Colouring one half of a pair colours both, as a pair has one colour.
	if (pair && item.id.startsWith(TAB_COLOUR_PREFIX)) {
		actions.setColour(pair, colourFrom(item.id.slice(TAB_COLOUR_PREFIX.length)));
		return true;
	}
	if (!item.id.startsWith(PAIR_PREFIX)) return false;
	if (!pair) return true;
	if (item.id.startsWith(LAYOUT_PREFIX)) {
		const layout = LAYOUTS.find(
			(candidate) => candidate.layout === item.id.slice(LAYOUT_PREFIX.length),
		);
		if (layout) actions.setLayout(pair, layout.layout);
		return true;
	}
	if (item.id.startsWith(COLOUR_PREFIX)) {
		actions.setColour(pair, colourFrom(item.id.slice(COLOUR_PREFIX.length)));
		return true;
	}
	switch (item.id) {
		case 'pair:separate':
			actions.separate(pair);
			break;
		case 'pair:swap':
			actions.swapPanes(pair);
			break;
		case 'pair:resetSizes':
			actions.resetSizes(pair);
			break;
		case 'pair:pin':
			actions.pin(pair, true);
			break;
		case 'pair:unpin':
			actions.pin(pair, false);
			break;
		case 'pair:duplicate':
			actions.duplicate(pair);
			break;
		case 'pair:closeBoth':
			actions.closeBoth(pair);
			break;
	}
	return true;
}
