// Verifies that an archive entry an extraction leaves out because of its stored name is marked in the list
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { FakeVfsClient, makeEntry } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { leftOutNote } from './leftOut';
import { ListView } from './ListView';
import { VfsClientProvider } from './VfsClientContext';

const ARCHIVE: Location = {
	display: 'unsafe.zip',
	uri: 'archive:file:///tmp/unsafe.zip!/',
};

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(400, 900);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

describe('entries left out when extracting', () => {
	it('reads the archive provider’s attribute', () => {
		expect(leftOutNote(makeEntry(1, 'ok.txt'))).toBeNull();
		expect(
			leftOutNote(makeEntry(2, 'a:b.txt', { attributes: { 'archive.unsafe': 'absolute' } })),
		).toBe('Left out when extracting');
	});

	it('marks the row under the name as stored', async () => {
		const vfs = new FakeVfsClient();
		vfs.setFolder(ARCHIVE, [
			makeEntry(1, 'a:b.txt', { attributes: { 'archive.unsafe': 'absolute' } }),
			makeEntry(2, 'ok.txt'),
		]);
		render(
			<VfsClientProvider client={vfs}>
				<ListView location={ARCHIVE} />
			</VfsClientProvider>,
		);
		await waitFor(() => expect(screen.getAllByRole('option')).toHaveLength(2));
		const unsafe = screen.getAllByRole('option').find((o) => within(o).queryByText('a:b.txt'))!;
		expect(within(unsafe).getByRole('img', { name: 'Left out when extracting' })).toBeVisible();
		const ok = screen.getAllByRole('option').find((o) => within(o).queryByText('ok.txt'))!;
		expect(within(ok).queryByRole('img', { name: 'Left out when extracting' })).toBeNull();
	});
});
