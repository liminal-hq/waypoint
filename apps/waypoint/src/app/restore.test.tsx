// Verifies restoring a session whose folder has gone: the tab shows "Folder not found" and keeps its history
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, fireEvent, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { stubLayout } from '../test/browseHarness';
import { createTree, DOCS, HOME, MUSIC, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

describe('a restored tab whose folder no longer exists', () => {
	it('shows the error state with its history intact, and Back returns to the previous folder', async () => {
		// The session as it was saved: a tab at the music folder with Home behind it.
		const tabs = new FakeTabsApi();
		const id = await tabs.openTab(HOME);
		await tabs.navigate(id, DOCS);
		await tabs.navigate(id, MUSIC);

		// By the next start, the music folder is gone.
		const client = createTree();
		client.failOpening(MUSIC, { kind: 'notFound', location: MUSIC });
		await renderWorkspace(client, tabs);

		const alert = await screen.findByRole('alert');
		expect(alert).toHaveAttribute('data-error', 'notFound');
		expect(alert).toHaveTextContent('Folder not found');
		const snapshot = await tabs.getSnapshot();
		expect(snapshot.tabs[0]!.back.map((entry) => entry.uri)).toEqual([HOME.uri, DOCS.uri]);
		expect(snapshot.tabs[0]!.location.uri).toBe(MUSIC.uri);

		fireEvent.click(screen.getByRole('button', { name: 'Back' }));
		await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
		expect(await screen.findByRole('option', { name: /^report\.pdf/ })).toBeInTheDocument();
	});
});
