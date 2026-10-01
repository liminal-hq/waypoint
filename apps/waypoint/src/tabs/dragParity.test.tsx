// Verifies every tab drag outcome has a non-pointer path, so a new outcome cannot ship without one
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Group } from '@liminal-hq/waypoint-protocol/generated/Group';
import type { WindowSummary } from '@liminal-hq/waypoint-protocol/generated/WindowSummary';
import { act, cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { stubLayout } from '../test/browseHarness';
import { DOCS, MUSIC, renderWorkspace } from '../test/workspaceHarness';
import { pairTabItems } from './pairMenus';
import { NON_POINTER_PATHS, type TabDragOutcome } from './tabDrag';
import { tabMenuItems } from './tabMenus';

/** Every outcome, written out so that adding one to `TabDragOutcome` fails to compile until it is here. */
const OUTCOMES = [
	'reorder',
	'holdSplit',
	'holdGroup',
	'overGroupLabel',
	'leaveGroup',
	'splitPane',
	'newWindow',
	'cancel',
] as const satisfies readonly TabDragOutcome[];
type Missing = Exclude<TabDragOutcome, (typeof OUTCOMES)[number]>;
const complete: [Missing] extends [never] ? true : never = true;

const idsOf = (items: readonly MenuItem[]): string[] =>
	items
		.flatMap((item) => [item.id, ...(item.type === 'submenu' ? idsOf(item.items) : [])])
		.filter((id): id is string => id !== undefined);

const group: Group = { id: 1, name: 'Work', colour: null, collapsed: false };
const window2: WindowSummary = { label: 'main-2', title: 'Music', tabCount: 2, active: false };

describe('non-pointer parity', () => {
	it('has a path for every outcome, and no stale ones', () => {
		expect(complete).toBe(true);
		expect(Object.keys(NON_POINTER_PATHS).sort()).toEqual([...OUTCOMES].sort());
	});

	it('finds a menu item for every outcome that names one', () => {
		const single = {
			id: 1,
			location: { display: '/a', uri: 'file:///a' },
			back: [],
			forward: [],
			pinned: false,
			colour: null,
			group: null,
			hints: { scrollTop: 0, focused: null },
		};
		const grouped = { ...single, group: 1 };
		const other = { ...single, id: 2 };
		// A tab alone, where Split With and Add to Group are offered, and a grouped one, where Remove is.
		const alone = [
			...tabMenuItems(single, [], [group], [window2]),
			...pairTabItems(single, undefined, [single, other], []),
		];
		const inGroup = [
			...tabMenuItems(grouped, [], [group], [window2]),
			...pairTabItems(grouped, undefined, [grouped, other], []),
		];
		const available = new Set([...idsOf(alone), ...idsOf(inGroup)]);
		for (const outcome of OUTCOMES) {
			const path = NON_POINTER_PATHS[outcome];
			if (path.kind === 'menu') expect(available, `${outcome}: ${path.item}`).toContain(path.item);
		}
		// Removing from a group is only offered where there is a group to leave.
		expect(idsOf(alone)).not.toContain('removeFromGroup');
		expect(idsOf(inGroup)).toContain('removeFromGroup');
	});

	it('describes why an outcome has no path only when nothing is dragged without a pointer', () => {
		for (const outcome of OUTCOMES) {
			const path = NON_POINTER_PATHS[outcome];
			if (path.kind === 'none') expect(outcome).toBe('cancel');
		}
	});
});

describe('the key path of a reorder', () => {
	let restoreLayout: () => void;
	beforeEach(() => {
		restoreLayout = stubLayout(280);
	});
	afterEach(() => {
		cleanup();
		restoreLayout();
		vi.restoreAllMocks();
	});

	it('moves the focused tab with the keys the table names', async () => {
		const keys = NON_POINTER_PATHS.reorder;
		if (keys.kind !== 'keys') throw new Error('reorder has a key path');
		expect(keys.keys).toBe('Ctrl+Shift+ArrowLeft');
		const h = await renderWorkspace();
		await h.tabs.openTab(DOCS);
		await h.tabs.openTab(MUSIC);
		await waitFor(() => expect(screen.getAllByRole('tab')).toHaveLength(3));
		const last = screen.getAllByRole('tab')[2]!;
		act(() => last.focus());
		fireEvent.keyDown(last, { key: 'ArrowLeft', ctrlKey: true, shiftKey: true });
		await waitFor(async () =>
			expect((await h.tabs.getSnapshot()).tabs.map((tab) => tab.id)).toEqual([1, 3, 2]),
		);
	});
});
