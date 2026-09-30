// Verifies the empty-space menu: choosing the active sort key leaves the sort alone
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { BackgroundContextMenu } from './BackgroundContextMenu';
import type { ListingSession } from './useListingSession';

afterEach(cleanup);

function sessionWith(sort: { key: string; descending: boolean; directoriesFirst: boolean }) {
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
		});
		open(session);
		fireEvent.click(screen.getByRole('menuitemcheckbox', { name: /size/i }));
		expect(setSort).toHaveBeenCalledWith({
			key: 'size',
			descending: false,
			directoriesFirst: true,
		});
	});
});
