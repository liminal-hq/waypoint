// Verifies what the Trash's menus offer, by availability and not by guesswork
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import { describe, expect, it } from 'vitest';
import { backgroundMenuItems } from '../browse/BackgroundContextMenu';
import { sidebarMenuItems } from '../sidebar/SidebarMenu';
import { trashEntryMenuItems } from './TrashEntryMenu';

const state = { favouritePosition: null, pinned: false, canRename: false };
const ids = (items: MenuItem[]) => items.flatMap((item) => ('id' in item ? [item.id] : []));
const find = (items: MenuItem[], id: string) =>
	items.find((item) => 'id' in item && item.id === id);

describe('the Trash item menu', () => {
	it('restores or deletes, and offers nothing that opens, renames, copies or pastes', () => {
		const items = trashEntryMenuItems();
		expect(ids(items)).toEqual(['restore', 'delete']);
		expect(find(items, 'delete')).toMatchObject({ danger: true, shortcut: 'Delete' });
	});
});

describe('the Trash empty-space menu', () => {
	const sort = {
		key: 'name' as const,
		descending: false,
		directoriesFirst: true,
		groupBy: 'none' as const,
	};

	it('sorts by name, size and date deleted, never modified or kind', () => {
		const items = backgroundMenuItems(sort, false, { trash: { count: 2 } });
		expect(ids(items).filter((id) => id?.startsWith('sort:'))).toEqual([
			'sort:name',
			'sort:size',
			'sort:deleted',
		]);
	});

	it('has no hidden-files toggle and ends with Empty Trash, off while it is empty', () => {
		const full = backgroundMenuItems(sort, false, { trash: { count: 2 } });
		expect(ids(full)).not.toContain('showHidden');
		expect(ids(full).at(-1)).toBe('emptyTrash');
		expect(find(full, 'emptyTrash')).toMatchObject({ danger: true, disabled: false });
		expect(
			find(backgroundMenuItems(sort, false, { trash: { count: 0 } }), 'emptyTrash'),
		).toMatchObject({
			disabled: true,
		});
	});

	it('is unchanged for a folder, which has no date deleted and no Empty Trash', () => {
		const items = backgroundMenuItems(sort, true);
		expect(ids(items)).toEqual([
			'sort:name',
			'sort:size',
			'sort:modified',
			'sort:kind',
			'descending',
			'foldersFirst',
			'groupBy',
			'showHidden',
		]);
	});
});

describe('the sidebar’s Trash menu', () => {
	it('opens like any place and empties the Trash, which is off when it is empty or cannot be emptied', () => {
		const items = sidebarMenuItems('trash', { ...state, trash: { count: 3, available: true } });
		expect(ids(items)).toEqual(['open', 'openInNewTab', 'openInNewWindow', 'emptyTrash']);
		expect(find(items, 'emptyTrash')).toMatchObject({ danger: true, disabled: false });
		for (const trash of [{ count: 0, available: true }, { count: 3, available: false }, null]) {
			expect(find(sidebarMenuItems('trash', { ...state, trash }), 'emptyTrash')).toMatchObject({
				disabled: true,
			});
		}
	});

	it('is not added to the other places', () => {
		expect(ids(sidebarMenuItems('place', state))).not.toContain('emptyTrash');
	});
});
