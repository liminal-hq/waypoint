// Verifies the empty-space menu: choosing the active sort key leaves the sort alone
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { BackgroundContextMenu } from './BackgroundContextMenu';
import type { ListingSession } from './useListingSession';

afterEach(cleanup);

function sessionWith(sort: {
	key: string;
	descending: boolean;
	directoriesFirst: boolean;
	groupBy: string;
}) {
	const setSort = vi.fn();
	return { setSort, session: { model: { sort, setSort } } as unknown as ListingSession };
}

function open(session: ListingSession, onClose: () => void = () => {}) {
	render(
		<BackgroundContextMenu
			session={session}
			showHidden={false}
			position={{ x: 0, y: 0 }}
			keyboard={false}
			onToggleHidden={() => {}}
			onClose={onClose}
		/>,
	);
}

/** Opens the menu and then the named submenu by clicking its row, as a pointer user does. */
function openSubmenu(session: ListingSession, name: 'Sort by' | 'Group by') {
	open(session);
	fireEvent.click(screen.getByRole('menuitem', { name }));
}

describe('the empty-space menu’s Sort by and Group by submenus', () => {
	const sort = { key: 'size', descending: true, directoriesFirst: false, groupBy: 'kind' };

	it('lists the two submenus as rows, with no heading and none of their choices at the top level', () => {
		open(sessionWith(sort).session);
		const menu = screen.getByRole('menu', { name: 'Folder actions' });
		expect(
			within(menu)
				.getAllByRole('menuitem')
				.map((row) => row.textContent),
		).toEqual(['Sort by', 'Group by', 'Properties']);
		expect(
			within(menu)
				.getAllByRole('menuitemcheckbox')
				.map((row) => row.textContent),
		).toEqual(['Show hidden filesCtrl+H']);
		expect(screen.getByRole('menuitem', { name: 'Sort by' })).toHaveAttribute(
			'aria-haspopup',
			'menu',
		);
		expect(screen.getByRole('menuitem', { name: 'Group by' })).toHaveAttribute(
			'aria-expanded',
			'false',
		);
	});

	it('opens Sort by with the keys, then Descending and Folders first, keeping what is checked', () => {
		openSubmenu(sessionWith(sort).session, 'Sort by');
		const submenu = screen.getByRole('menu', { name: 'Sort by' });
		const rows = within(submenu).getAllByRole('menuitemcheckbox');
		expect(rows.map((row) => row.textContent)).toEqual([
			'Name',
			'Size',
			'Modified',
			'Kind',
			'Descending',
			'Folders first',
		]);
		expect(rows.map((row) => row.getAttribute('aria-checked'))).toEqual([
			'false',
			'true',
			'false',
			'false',
			'true',
			'false',
		]);
	});

	it('opens Group by on its row with the grouping in use checked', () => {
		openSubmenu(sessionWith(sort).session, 'Group by');
		const submenu = screen.getByRole('menu', { name: 'Group by' });
		const checked = within(submenu)
			.getAllByRole('menuitemcheckbox')
			.filter((row) => row.getAttribute('aria-checked') === 'true');
		expect(checked.map((row) => row.textContent)).toEqual(['Kind']);
	});

	it('opens a submenu with Right, chooses with Enter and closes the menu, as a click does', () => {
		const { session, setSort } = sessionWith({ ...sort, key: 'name' });
		const onClose = vi.fn();
		open(session, onClose);
		const row = screen.getByRole('menuitem', { name: 'Sort by' });
		row.focus();
		fireEvent.keyDown(row, { key: 'ArrowRight' });
		const submenu = screen.getByRole('menu', { name: 'Sort by' });
		const first = within(submenu).getByRole('menuitemcheckbox', { name: 'Name' });
		expect(first).toHaveFocus();
		fireEvent.keyDown(first, { key: 'ArrowDown' });
		const size = within(submenu).getByRole('menuitemcheckbox', { name: 'Size' });
		expect(size).toHaveFocus();
		fireEvent.keyDown(size, { key: 'Enter' });
		expect(setSort).toHaveBeenCalledWith({ ...sort, key: 'size', descending: false });
		expect(onClose).toHaveBeenCalled();
	});

	it('closes the submenu with Left and puts the focus back on its row', () => {
		open(sessionWith(sort).session);
		const row = screen.getByRole('menuitem', { name: 'Group by' });
		row.focus();
		fireEvent.keyDown(row, { key: 'ArrowRight' });
		const first = within(screen.getByRole('menu', { name: 'Group by' })).getAllByRole(
			'menuitemcheckbox',
		)[0]!;
		fireEvent.keyDown(first, { key: 'ArrowLeft' });
		expect(screen.queryByRole('menu', { name: 'Group by' })).toBeNull();
		expect(row).toHaveFocus();
	});

	it('toggles Descending and Folders first from inside Sort by', () => {
		const { session, setSort } = sessionWith(sort);
		openSubmenu(session, 'Sort by');
		fireEvent.click(screen.getByRole('menuitemcheckbox', { name: 'Descending' }));
		expect(setSort).toHaveBeenLastCalledWith({ ...sort, descending: false });
		cleanup();
		const again = sessionWith(sort);
		openSubmenu(again.session, 'Sort by');
		fireEvent.click(screen.getByRole('menuitemcheckbox', { name: 'Folders first' }));
		expect(again.setSort).toHaveBeenCalledWith({ ...sort, directoriesFirst: true });
	});
});

describe('the empty-space menu sort keys', () => {
	it('leaves the direction alone when the active key is chosen again', () => {
		const { session, setSort } = sessionWith({
			key: 'name',
			descending: true,
			directoriesFirst: true,
			groupBy: 'none',
		});
		openSubmenu(session, 'Sort by');
		fireEvent.click(screen.getByRole('menuitemcheckbox', { name: /^name$/i }));
		expect(setSort).not.toHaveBeenCalled();
	});

	it('starts a different key ascending', () => {
		const { session, setSort } = sessionWith({
			key: 'name',
			descending: true,
			directoriesFirst: true,
			groupBy: 'none',
		});
		openSubmenu(session, 'Sort by');
		fireEvent.click(screen.getByRole('menuitemcheckbox', { name: /^size$/i }));
		expect(setSort).toHaveBeenCalledWith({
			key: 'size',
			descending: false,
			directoriesFirst: true,
			groupBy: 'none',
		});
	});
});

describe('the empty-space menu group by', () => {
	const sort = {
		key: 'name',
		descending: true,
		directoriesFirst: true,
		groupBy: 'size',
	};

	/** Opens the Group by submenu and returns its items (the full names a screen reader hears). */
	function openGroupBy(session: ListingSession) {
		openSubmenu(session, 'Group by');
		const items = screen
			.getAllByRole('menuitemcheckbox')
			.filter(
				(item) =>
					item.getAttribute('aria-label')?.startsWith('Group by') ||
					item.getAttribute('aria-label') === 'No grouping',
			);
		return {
			items,
			getByRole: (_role: string, { name }: { name: string }) =>
				items.find((item) => item.textContent === name)!,
		};
	}

	it('lists no grouping and each grouping with the one in use checked', () => {
		const { session } = sessionWith(sort);
		const { items } = openGroupBy(session);
		expect(items.map((item) => item.textContent)).toEqual([
			'No grouping',
			'Kind',
			'Modified',
			'Size',
			'Name',
			'Type',
		]);
		expect(items.map((item) => item.getAttribute('aria-checked'))).toEqual([
			'false',
			'false',
			'false',
			'true',
			'false',
			'false',
		]);
	});

	it('groups by the chosen item and keeps the rest of the sort', () => {
		const { session, setSort } = sessionWith(sort);
		fireEvent.click(openGroupBy(session).getByRole('menuitemcheckbox', { name: 'Kind' }));
		expect(setSort).toHaveBeenCalledWith({ ...sort, groupBy: 'kind' });
	});

	it('takes the grouping away with No grouping, and does nothing for the one in use', () => {
		const { session, setSort } = sessionWith(sort);
		fireEvent.click(openGroupBy(session).getByRole('menuitemcheckbox', { name: 'No grouping' }));
		expect(setSort).toHaveBeenCalledWith({ ...sort, groupBy: 'none' });
		cleanup();
		const again = sessionWith(sort);
		fireEvent.click(openGroupBy(again.session).getByRole('menuitemcheckbox', { name: 'Size' }));
		expect(again.setSort).not.toHaveBeenCalled();
	});
});

describe('the empty-space menu’s Reset This Folder’s View', () => {
	const sort = { key: 'name', descending: false, directoriesFirst: true, groupBy: 'none' };

	function openWith(folderView: 'default' | 'remembered' | undefined, onReset = vi.fn()) {
		const { session } = sessionWith(sort);
		render(
			<BackgroundContextMenu
				session={session}
				showHidden={false}
				position={{ x: 0, y: 0 }}
				keyboard={false}
				onToggleHidden={() => {}}
				onClose={() => {}}
				folderView={folderView}
				onResetFolderView={onReset}
			/>,
		);
		return onReset;
	}

	it('resets the folder when it remembers a view', () => {
		const onReset = openWith('remembered');
		fireEvent.click(screen.getByRole('menuitem', { name: 'Reset This Folder’s View' }));
		expect(onReset).toHaveBeenCalledTimes(1);
	});

	it('lists the item disabled for a folder that shows the window’s view, and leaves it out where folders do not remember', () => {
		openWith('default');
		expect(screen.getByRole('menuitem', { name: 'Reset This Folder’s View' })).toHaveAttribute(
			'aria-disabled',
			'true',
		);
		cleanup();
		openWith(undefined);
		expect(screen.queryByRole('menuitem', { name: 'Reset This Folder’s View' })).toBeNull();
	});
});
