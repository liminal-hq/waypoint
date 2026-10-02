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

function open(session: ListingSession) {
	render(
		<BackgroundContextMenu
			session={session}
			showHidden={false}
			position={{ x: 0, y: 0 }}
			keyboard={false}
			onToggleHidden={() => {}}
			onClose={() => {}}
		/>,
	);
}

describe('the empty-space menu sort keys', () => {
	it('leaves the direction alone when the active key is chosen again', () => {
		const { session, setSort } = sessionWith({
			key: 'name',
			descending: true,
			directoriesFirst: true,
			groupBy: 'none',
		});
		open(session);
		fireEvent.click(screen.getByRole('menuitemcheckbox', { name: /name/i }));
		expect(setSort).not.toHaveBeenCalled();
	});

	it('starts a different key ascending', () => {
		const { session, setSort } = sessionWith({
			key: 'name',
			descending: true,
			directoriesFirst: true,
			groupBy: 'none',
		});
		open(session);
		fireEvent.click(screen.getByRole('menuitemcheckbox', { name: /size/i }));
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

	/** Opens the Group by submenu and returns it. */
	function openGroupBy(session: ListingSession) {
		open(session);
		fireEvent.click(screen.getByRole('menuitem', { name: /^Group by/ }));
		return within(screen.getAllByRole('menu').at(-1)!);
	}

	it('lists no grouping and each grouping with the one in use checked', () => {
		const { session } = sessionWith(sort);
		const submenu = openGroupBy(session);
		const items = submenu.getAllByRole('menuitemcheckbox');
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
