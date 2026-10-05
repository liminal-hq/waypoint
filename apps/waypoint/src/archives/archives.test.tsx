// Verifies archives in the window: opening one as a folder, the locked and slow states, and the menu entries that reach them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { connectStore } from '../connections/connectStore';
import { FakeConnectionsClient } from '../connections/fakeConnectionsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { makeEntry } from '../services/fakeVfsClient';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, HOME, renderWorkspace } from '../test/workspaceHarness';
import { ArchiveClientProvider } from './ArchiveContext';
import { ArchiveOpening } from './ArchiveOpening';
import { FakeArchiveClient } from './fakeArchiveClient';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	act(() => {
		connectStore.getState().answer(null);
		connectStore.getState().close();
	});
	cleanup();
	restoreLayout();
});

const PACK: Location = {
	display: 'archive:file:///home/test/pack.zip!/',
	uri: 'archive:file:///home/test/pack.zip!/',
};

/** The tree with a few things in the home folder: a plain file and an archive. */
function treeWithArchive() {
	const client = createTree();
	client.setFolder(HOME, [
		makeEntry(1, 'docs', { kind: 'directory' }),
		makeEntry(3, 'notes.txt'),
		makeEntry(4, 'pack.zip'),
	]);
	client.setFolder(PACK, [makeEntry(1, 'inside.txt')]);
	return client;
}

/** What the provider asks: the archive's `archive:` location, named by its file as people read it. */
const LOCKED: Location = { display: '/home/test/pack.zip', uri: PACK.uri };
const lock = (kind: 'authRequired' | 'authFailed'): VfsError =>
	kind === 'authRequired'
		? { kind, location: LOCKED, prompt: { kind: 'passphrase', subject: '/home/test/pack.zip' } }
		: { kind, location: LOCKED };

describe('opening an archive', () => {
	it('opens it as a folder in a new tab when it is double-clicked (D24)', async () => {
		const client = treeWithArchive();
		const tabs = new FakeTabsApi();
		await renderWorkspace(client, tabs, undefined, { archives: new FakeArchiveClient() });
		fireEvent.doubleClick(await screen.findByText('pack.zip'));
		expect(await screen.findByText('inside.txt')).toBeVisible();
		const snapshot = await tabs.getSnapshot();
		expect(snapshot.tabs).toHaveLength(2);
		expect(snapshot.tabs.find((tab) => tab.id === snapshot.active)?.location.uri).toBe(PACK.uri);
		// The folder it came from is still open beside it.
		expect(snapshot.tabs.some((tab) => tab.location.uri === HOME.uri)).toBe(true);
	});

	it('still opens a plain file in its application, and offers Open as Folder only on an archive', async () => {
		const client = treeWithArchive();
		await renderWorkspace(client, undefined, undefined, {
			archives: new FakeArchiveClient(),
			ops: createFakeOpsClient(),
		});
		fireEvent.contextMenu(await screen.findByText('notes.txt'));
		expect(screen.queryByRole('menuitem', { name: 'Open as Folder' })).toBeNull();
		expect(screen.queryByRole('menuitem', { name: 'Extract Here' })).toBeNull();
		fireEvent.keyDown(document.body, { key: 'Escape' });
		cleanup();
	});

	it('shows the archive in this tab from Open as Folder, and offers Extract on the same menu', async () => {
		const client = treeWithArchive();
		const tabs = new FakeTabsApi();
		await renderWorkspace(client, tabs, undefined, {
			archives: new FakeArchiveClient(),
			ops: createFakeOpsClient(),
		});
		fireEvent.contextMenu(await screen.findByText('pack.zip'));
		expect(await screen.findByRole('menuitem', { name: 'Extract To…' })).toBeVisible();
		expect(screen.getByRole('menuitem', { name: 'Compress…' })).toBeVisible();
		fireEvent.click(screen.getByRole('menuitem', { name: 'Open as Folder' }));
		expect(await screen.findByText('inside.txt')).toBeVisible();
		const snapshot = await tabs.getSnapshot();
		expect(snapshot.tabs).toHaveLength(1);
		expect(snapshot.tabs[0]?.location.uri).toBe(PACK.uri);
	});
});

describe('extracting from the menu', () => {
	it('sends an extract job for the archive under the pointer, beside the archive', async () => {
		const client = treeWithArchive();
		const ops = createFakeOpsClient();
		await renderWorkspace(client, undefined, undefined, {
			archives: new FakeArchiveClient(),
			ops,
			connections: new FakeConnectionsClient(),
		});
		fireEvent.contextMenu(await screen.findByText('pack.zip'));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Extract Here' }));
		await waitFor(() => expect(ops.calls.some((call) => call[0] === 'submit')).toBe(true));
		const request = ops.calls.find((call) => call[0] === 'submit')![1] as {
			kind: { kind: string };
			destination: unknown;
			sources: { kind: string; locations: Location[] };
			archive: unknown;
		};
		expect(request.kind).toEqual({ kind: 'extract' });
		expect(request.destination).toBeNull();
		expect(request.sources.locations.map((location) => location.uri)).toEqual([
			'file:///home/test/pack.zip',
		]);
		expect(request.archive).toEqual({ kind: 'extract', layout: 'auto', allowLarge: false });
	});

	it('is not offered on a plain file', async () => {
		const client = treeWithArchive();
		await renderWorkspace(client, undefined, undefined, {
			archives: new FakeArchiveClient(),
			ops: createFakeOpsClient(),
		});
		fireEvent.contextMenu(await screen.findByText('notes.txt'));
		await screen.findByRole('menuitem', { name: 'Compress…' });
		expect(screen.queryByRole('menuitem', { name: 'Extract Here' })).toBeNull();
		expect(screen.queryByRole('menuitem', { name: 'Extract To…' })).toBeNull();
	});
});

describe('a locked archive', () => {
	async function openLocked(error: VfsError, archives: FakeArchiveClient) {
		const client = treeWithArchive();
		client.failOpening(PACK, error);
		const tabs = new FakeTabsApi();
		await tabs.openTab(PACK);
		// The question dialogs belong to the connections host, which every real window has.
		await renderWorkspace(client, tabs, undefined, {
			archives,
			connections: new FakeConnectionsClient(),
		});
		return client;
	}

	it('says it is locked, asks for the password in the login dialog and opens once it is given', async () => {
		const archives = new FakeArchiveClient();
		const client = await openLocked(lock('authRequired'), archives);
		const state = await screen.findByRole('heading', { name: 'pack.zip is locked' });
		expect(state.parentElement).toHaveTextContent(/encrypted/);
		fireEvent.click(screen.getByRole('button', { name: 'Enter password' }));
		const dialog = await screen.findByRole('dialog', { name: /pack\.zip/ });
		client.succeedOpening(PACK);
		fireEvent.change(within(dialog).getByLabelText('Passphrase'), { target: { value: 's3cret' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Sign In' }));
		expect(await screen.findByText('inside.txt')).toBeVisible();
		// The password is not offered a place in the keyring: it is kept until Waypoint quits.
		expect(within(dialog).queryByRole('checkbox')).toBeNull();
		expect(archives.unlocked).toEqual([{ location: LOCKED, passphrase: 's3cret' }]);
	});

	it('says the password was not accepted when it was refused', async () => {
		await openLocked(lock('authFailed'), new FakeArchiveClient());
		const state = await screen.findByRole('heading', { name: 'pack.zip is locked' });
		expect(state.parentElement).toHaveTextContent(/not accepted/);
	});

	it('stays locked, saying why, when the password cannot be given', async () => {
		const archives = new FakeArchiveClient();
		archives.failNext = { kind: 'unsupported', what: 'archives in this build' };
		await openLocked(lock('authRequired'), archives);
		fireEvent.click(await screen.findByRole('button', { name: 'Enter password' }));
		const dialog = await screen.findByRole('dialog', { name: /pack\.zip/ });
		fireEvent.change(within(dialog).getByLabelText('Passphrase'), { target: { value: 'x' } });
		fireEvent.click(within(dialog).getByRole('button', { name: 'Sign In' }));
		// Shown under the button and read out through the live region.
		expect((await screen.findAllByText('archives in this build')).length).toBeGreaterThanOrEqual(2);
		expect(screen.getByRole('heading', { name: 'pack.zip is locked' })).toBeVisible();
	});

	it('leaves a server’s own passphrase question to the connection state', async () => {
		const server: Location = { display: 'sftp://me@nas.lan/', uri: 'sftp://me@nas.lan/' };
		const client = treeWithArchive();
		(client as unknown as { options: { remoteSchemes: string[] } }).options.remoteSchemes = [
			'sftp',
		];
		client.setFolder(server, []);
		client.failOpening(server, {
			kind: 'authRequired',
			location: server,
			prompt: { kind: 'passphrase', subject: '~/.ssh/id_ed25519' },
		});
		const tabs = new FakeTabsApi();
		await tabs.openTab(server);
		await renderWorkspace(client, tabs, undefined, { archives: new FakeArchiveClient() });
		expect(await screen.findByRole('heading', { name: /Sign in/ })).toBeVisible();
		expect(screen.queryByText(/is locked/)).toBeNull();
	});
});

describe('an archive that is slow to open', () => {
	const container: Location = {
		display: '/home/test/big.tar.gz',
		uri: 'file:///home/test/big.tar.gz',
	};
	const location: Location = { display: '', uri: 'archive:file:///home/test/big.tar.gz!/' };

	it('says why it takes a while, in a status, when its provider says so', async () => {
		const archives = new FakeArchiveClient();
		render(
			<ArchiveClientProvider client={archives}>
				<ArchiveOpening location={location} fallback={<p>Opening…</p>} />
			</ArchiveClientProvider>,
		);
		expect(screen.getByText('Opening…')).toBeVisible();
		act(() => archives.slowListing({ container, bytes: 200_000_000, format: 'tar.gz' }));
		const status = await screen.findByRole('status');
		expect(status).toHaveTextContent(
			'Reading big.tar.gz. A tar.gz archive has no index, so opening it takes a while.',
		);
	});

	it('ignores the notice of another archive', async () => {
		const archives = new FakeArchiveClient();
		render(
			<ArchiveClientProvider client={archives}>
				<ArchiveOpening location={location} fallback={<p>Opening…</p>} />
			</ArchiveClientProvider>,
		);
		act(() =>
			archives.slowListing({
				container: { display: '/other.tar.gz', uri: 'file:///other.tar.gz' },
				bytes: 1,
				format: 'tar.gz',
			}),
		);
		await waitFor(() => expect(screen.getByText('Opening…')).toBeVisible());
		expect(screen.queryByRole('status')).toBeNull();
	});

	it('shows the plain opening message where there is no archive client', () => {
		render(<ArchiveOpening location={location} fallback={<p>Opening…</p>} />);
		expect(screen.getByText('Opening…')).toBeVisible();
	});
});

describe('a damaged archive', () => {
	it('says so, with the archive named, and never an empty folder', async () => {
		const client = treeWithArchive();
		client.failOpening(PACK, { kind: 'corrupt', location: PACK });
		const tabs = new FakeTabsApi();
		await tabs.openTab(PACK);
		await renderWorkspace(client, tabs);
		expect(await screen.findByRole('heading', { name: 'This archive is damaged' })).toBeVisible();
	});

	it('says what cannot be opened when the format is not one it reads', async () => {
		const client = treeWithArchive();
		client.failOpening(PACK, { kind: 'unsupported', what: 'this kind of file as an archive' });
		const tabs = new FakeTabsApi();
		await tabs.openTab(PACK);
		await renderWorkspace(client, tabs);
		expect(
			await screen.findByRole('heading', { name: 'This cannot be shown as a folder' }),
		).toBeVisible();
		expect(screen.getByText(/this kind of file as an archive/)).toBeVisible();
	});
});
