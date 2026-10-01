// Verifies a cut dims its rows in the list and the grid, in step with the clipboard and in any window that shares it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { ClipboardProvider } from '../ops/ClipboardContext';
import { createClipboardService, type ClipboardService } from '../ops/clipboardService';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import { FOLDER, stubLayout } from '../test/browseHarness';
import { GridView } from './GridView';
import { ListingView } from './ListView';
import { useListingSession } from './useListingSession';
import { useVfsClient, VfsClientProvider } from './VfsClientContext';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(400, 600);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

function Host({ grid }: { grid: boolean }) {
	const state = useListingSession(useVfsClient(), FOLDER);
	return grid ? <GridView state={state} size={96} /> : <ListingView state={state} />;
}

function client() {
	const vfs = new FakeVfsClient();
	vfs.setFolder(FOLDER, [
		makeEntry(1, 'alpha.txt'),
		makeEntry(2, 'beta.jpg'),
		makeEntry(3, 'my file.txt'),
	]);
	return vfs;
}

async function mount(grid: boolean, ops: FakeOpsClient = createFakeOpsClient()) {
	const service: ClipboardService = createClipboardService({
		client: ops,
		vfs: client(),
		os: null,
		focusTarget: null,
	});
	await service.ready;
	render(
		<VfsClientProvider client={client()}>
			<ClipboardProvider value={service}>
				<Host grid={grid} />
			</ClipboardProvider>
		</VfsClientProvider>,
	);
	await screen.findByRole('listbox');
	await waitFor(() => expect(screen.getAllByRole('option').length).toBe(3));
	return { ops, service };
}

const row = (name: string) =>
	screen.getAllByRole('option').find((option) => option.textContent?.includes(name))!;

describe.each([
	['the list', false],
	['the grid', true],
])('%s', (_name, grid) => {
	it('dims the rows a cut holds in this folder, whatever window cut them', async () => {
		const { ops } = await mount(grid);
		expect(screen.getAllByRole('option').some((r) => r.hasAttribute('data-cut'))).toBe(false);
		await act(async () => {
			await ops.setClipboard('cut', [
				fileLocation('/home/test/alpha.txt'),
				fileLocation('/home/test/my file.txt'),
				fileLocation('/home/elsewhere/beta.jpg'),
			]);
		});
		await waitFor(() => expect(row('alpha.txt')).toHaveAttribute('data-cut'));
		expect(row('my file.txt')).toHaveAttribute('data-cut');
		// An item of the same name in another folder does not dim this folder's.
		expect(row('beta.jpg')).not.toHaveAttribute('data-cut');
	});

	it('stops dimming when the cut is spent, replaced by a copy or cleared', async () => {
		const { ops } = await mount(grid);
		const cut = [fileLocation('/home/test/alpha.txt')];
		await act(async () => void (await ops.setClipboard('cut', cut)));
		await waitFor(() => expect(row('alpha.txt')).toHaveAttribute('data-cut'));
		await act(async () => void (await ops.setClipboard('copy', cut)));
		await waitFor(() => expect(row('alpha.txt')).not.toHaveAttribute('data-cut'));
		await act(async () => void (await ops.setClipboard('cut', cut)));
		await waitFor(() => expect(row('alpha.txt')).toHaveAttribute('data-cut'));
		await act(async () => void (await ops.setClipboard('copy', [])));
		await waitFor(() => expect(row('alpha.txt')).not.toHaveAttribute('data-cut'));
	});
});

describe('a window with no clipboard', () => {
	it('dims nothing', async () => {
		render(
			<VfsClientProvider client={client()}>
				<Host grid={false} />
			</VfsClientProvider>,
		);
		await waitFor(() => expect(screen.getAllByRole('option').length).toBe(3));
		expect(screen.getAllByRole('option').some((r) => r.hasAttribute('data-cut'))).toBe(false);
	});
});
