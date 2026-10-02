// Verifies what the destination host starts the dialog with: a copy resumes at the last folder, a move never does
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FakeVfsClient, fileLocation } from '../services/fakeVfsClient';
import { FOLDER } from '../test/browseHarness';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { createRecentDestinations } from './destinationModel';
import { DestinationHost } from './DestinationHost';
import { createDestinationStore } from './destinationStore';

vi.mock('../sidebar/PlacesClientContext', () => ({ usePlacesClient: () => null }));
vi.mock('../sidebar/usePlaces', () => ({ usePlaces: () => null }));
vi.mock('../tabs/TabsContext', () => ({ useTabsSnapshot: () => null }));
vi.mock('./OpsContext', () => ({ useOps: () => null }));

afterEach(cleanup);

const LAST = fileLocation('/home/test/last');

function open(forbidOrigin: boolean) {
	const store = createDestinationStore();
	const recent = createRecentDestinations({
		getItem: () => null,
		setItem: () => {},
	} as unknown as Storage);
	recent.remember(LAST);
	render(
		<VfsClientProvider client={new FakeVfsClient({ home: '/home/test' })}>
			<DestinationHost store={store} recent={recent} />
		</VfsClientProvider>,
	);
	store.setState({
		request: {
			options: { title: 'T', confirmLabel: 'Go', base: FOLDER, origin: FOLDER, forbidOrigin },
			resolve: vi.fn(),
		},
	});
	return screen.findByRole('textbox', { name: 'Folder' });
}

describe('DestinationHost', () => {
	it('starts a copy at the last destination', async () => {
		expect(await open(false)).toHaveValue('/home/test/last');
	});

	it('starts a move with an empty field, so Enter cannot move anything to the last folder', async () => {
		expect(await open(true)).toHaveValue('');
	});
});
