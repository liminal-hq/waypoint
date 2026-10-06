// Verifies Favourites on servers: listed with the connection's state, dimmed when the protocol is off, opened lazily, and pinned with Ctrl+D
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Favourite } from '@liminal-hq/waypoint-protocol/generated/Favourite';
import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { draft, FakeConnectionsClient, serverLocation } from '../connections/fakeConnectionsClient';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { FakeVfsClient, makeEntry } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

const PICTURES = serverLocation('sftp://me@nas.lan/home/me/Pictures');
const CLOUD = serverLocation('davs://me@cloud.example.org/files');

function treeWithServer(): FakeVfsClient {
	const client = createTree();
	(client as unknown as { options: { remoteSchemes: string[] } }).options.remoteSchemes = ['sftp'];
	client.setFolder(PICTURES, [makeEntry(1, 'a.jpg')]);
	return client;
}

function places(favourites: Favourite[] = []) {
	return new FakePlacesClient({ places: fakePlaces('/home/test'), favourites });
}

const favourites = () =>
	within(screen.getByRole('navigation', { name: 'Sidebar' })).getByRole('group', {
		name: 'Favourites',
	});

describe('server favourites', () => {
	it('shows the connection state without connecting, and updates it as the state changes', async () => {
		const connections = new FakeConnectionsClient({ connections: [] });
		await renderWorkspace(
			treeWithServer(),
			undefined,
			places([{ label: 'NAS pictures', location: PICTURES }]),
			{ sidebar: true, connections },
		);
		const row = await within(favourites()).findByRole('button', {
			name: /NAS pictures.*Not connected/,
		});
		expect(row).toHaveAttribute('data-server');
		expect(row).toHaveAttribute('data-drop', 'place');
		expect(connections.calls.some((call) => call.method === 'connect')).toBe(false);
		act(() => connections.emitState('sftp://me@nas.lan', { kind: 'connected' }));
		await waitFor(() => expect(row).toHaveAccessibleName(/NAS pictures.*Connected/));
	});

	it('opens in the active tab on click and in a background tab on middle click', async () => {
		const h = await renderWorkspace(
			treeWithServer(),
			undefined,
			places([{ label: 'NAS pictures', location: PICTURES }]),
			{ sidebar: true, connections: new FakeConnectionsClient() },
		);
		const row = await within(favourites()).findByRole('button', { name: /NAS pictures/ });
		fireEvent(row, new MouseEvent('auxclick', { bubbles: true, button: 1 }));
		await waitFor(async () => expect((await h.tabs.getSnapshot()).tabs.length).toBeGreaterThan(1));
		fireEvent.click(row);
		await waitFor(async () => {
			const snapshot = await h.tabs.getSnapshot();
			expect(snapshot.tabs.find((tab) => tab.id === snapshot.active)!.location.uri).toBe(
				PICTURES.uri,
			);
		});
	});

	it('dims a favourite on a protocol that is turned off, with the reason', async () => {
		const connections = new FakeConnectionsClient({
			connections: [draft({ host: 'nas.lan', user: 'me', name: 'NAS' })],
			schemes: ['sftp'],
			off: ['dav', 'davs'],
		});
		await renderWorkspace(
			treeWithServer(),
			undefined,
			places([
				{ label: 'Cloud files', location: CLOUD },
				{ label: 'NAS pictures', location: PICTURES },
			]),
			{ sidebar: true, connections },
		);
		const cloud = await within(favourites()).findByRole('button', { name: /Cloud files/ });
		expect(cloud).toHaveAccessibleName(/Turned off in Settings → Experimental/);
		expect(cloud.closest('li')).toHaveAttribute('data-off');
		const nas = within(favourites()).getByRole('button', { name: /NAS pictures/ });
		expect(nas.closest('li')).not.toHaveAttribute('data-off');
	});

	it('pins the folder a tab shows on a server with Ctrl+D, which opens it without a password', async () => {
		const connections = new FakeConnectionsClient({
			connections: [draft({ host: 'nas.lan', user: 'me', name: 'NAS' })],
		});
		const fake = places();
		await renderWorkspace(treeWithServer(), undefined, fake, { sidebar: true, connections });
		const nas = await within(await screen.findByRole('group', { name: 'Network' })).findByRole(
			'button',
			{ name: /NAS/ },
		);
		fireEvent.click(nas);
		await screen.findByRole('option', { name: /a\.jpg/ }).catch(() => undefined);
		fireEvent.keyDown(window, { key: 'd', ctrlKey: true });
		await waitFor(() =>
			expect(fake.calls.some((call) => /^add sftp:\/\/me@nas\.lan/.test(call))).toBe(true),
		);
		expect(fake.calls.join('\n')).not.toMatch(/:[^/@]*@/);
		await within(favourites()).findByRole('button', { name: /Not connected|Connected|Connecting/ });
	});
});
