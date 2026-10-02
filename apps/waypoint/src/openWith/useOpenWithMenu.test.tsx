// Verifies the entry menu's Open With: what it reads, when it hides, and what choosing a row starts
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { entryMenuItems } from '../browse/EntryContextMenu';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import {
	createFakeOpenWithClient,
	fakeApp,
	type FakeOpenWithClient,
	type FakeOpenWithOptions,
} from './fakeOpenWithClient';
import { OpenWithProvider } from './OpenWithContext';
import { createOpenWithChooserStore } from './openWithChooserStore';
import { useOpenWithMenu } from './useOpenWithMenu';

const home = fileLocation('/home/scott');

async function setup(
	options: FakeOpenWithOptions = {},
	file: ReturnType<typeof makeEntry> = makeEntry(1, 'a.png'),
	withClient = true,
) {
	const vfs = new FakeVfsClient();
	vfs.setFolder(home, [file]);
	const { handle } = await vfs.openListing(home);
	const client: FakeOpenWithClient = createFakeOpenWithClient({
		handlers: {
			mime: 'image/png',
			default: fakeApp('viewer.desktop', 'Image Viewer'),
			recommended: [fakeApp('editor.desktop', 'Image Editor')],
			others: [fakeApp('text.desktop', 'Text Editor')],
		},
		...options,
	});
	const chooser = createOpenWithChooserStore();
	const wrapper = ({ children }: { children: ReactNode }) => (
		<VfsClientProvider client={vfs}>
			<OpenWithProvider client={withClient ? client : undefined}>{children}</OpenWithProvider>
		</VfsClientProvider>
	);
	const hook = renderHook(() => useOpenWithMenu({ session: null, entry: file, handle }, chooser), {
		wrapper,
	});
	return { client, chooser, hook };
}

const labelsOf = (item: { items: Array<{ type: string; label?: string }> } | null) =>
	item?.items.flatMap((row) => (row.label ? [row.label] : []));

describe('useOpenWithMenu', () => {
	it('holds a disabled row at first and then lists the applications for the file', async () => {
		const { client, hook } = await setup();
		await waitFor(() => expect(labelsOf(hook.result.current.item)).toContain('Image Editor'));
		expect(labelsOf(hook.result.current.item)).toEqual([
			'Image Viewer (default)',
			'Image Editor',
			'Other Application…',
		]);
		expect(client.asked).toEqual([['file:///home/scott/a.png']]);
	});

	it('is in the entry menu at the end of its first section, and not when it is hidden', async () => {
		const { hook } = await setup();
		await waitFor(() => expect(hook.result.current.item?.items.length).toBeGreaterThan(1));
		const file = makeEntry(1, 'a.png');
		const ids = entryMenuItems(file, undefined, false, hook.result.current.item).map((item) =>
			'id' in item ? item.id : '|',
		);
		expect(ids).toEqual(['open', 'openWith', '|', 'addToShelf', 'copyPath']);
		expect(
			entryMenuItems(file, undefined, false, null).some(
				(item) => 'id' in item && item.id === 'openWith',
			),
		).toBe(false);
	});

	it('opens in the default application, and in an application that is chosen', async () => {
		const { client, hook } = await setup();
		await waitFor(() => expect(hook.result.current.item?.items.length).toBeGreaterThan(1));
		act(() => void hook.result.current.select('openWith:default'));
		act(() => void hook.result.current.select('openWith:app:editor.desktop'));
		await waitFor(() => expect(client.calls).toHaveLength(2));
		expect(client.calls).toEqual([
			['openDefault', ['file:///home/scott/a.png']],
			['openWith', ['file:///home/scott/a.png'], 'editor.desktop'],
		]);
	});

	it('hands Other Application… to the chooser dialog with the type’s applications', async () => {
		const { chooser, hook } = await setup();
		await waitFor(() => expect(hook.result.current.item?.items.length).toBeGreaterThan(1));
		act(() => void hook.result.current.select('openWith:other'));
		expect(chooser.getState().request).toMatchObject({
			scope: 'others',
			uris: ['file:///home/scott/a.png'],
		});
		expect(chooser.getState().request?.handlers.others.map((app) => app.id)).toEqual([
			'text.desktop',
		]);
	});

	it('does not claim a row that is not its own', async () => {
		const { hook } = await setup();
		await waitFor(() => expect(hook.result.current.item?.items.length).toBeGreaterThan(1));
		expect(hook.result.current.select('copyPath')).toBe(false);
	});

	it('is hidden when the plugin reports the feature unavailable, and when there is no client', async () => {
		const off = await setup({ features: [] });
		await waitFor(() => expect(off.client.asked).toEqual([]));
		expect(off.hook.result.current.item).toBeNull();
		const none = await setup({}, makeEntry(1, 'a.png'), false);
		expect(none.hook.result.current.item).toBeNull();
	});

	it('is hidden for a selection of more than one type', async () => {
		const { client, hook } = await setup({ handlers: { mixed: true } });
		await waitFor(() => expect(client.asked).toHaveLength(1));
		await waitFor(() => expect(hook.result.current.item).toBeNull());
	});

	it('is hidden when the applications cannot be read', async () => {
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		const vfs = new FakeVfsClient();
		vfs.setFolder(home, [makeEntry(1, 'a.png')]);
		const { handle } = await vfs.openListing(home);
		const client = createFakeOpenWithClient();
		client.handlers = async () => {
			throw { kind: 'failed', message: 'boom' };
		};
		const entry = makeEntry(1, 'a.png');
		const hook = renderHook(() => useOpenWithMenu({ session: null, entry, handle }), {
			wrapper: ({ children }: { children: ReactNode }) => (
				<VfsClientProvider client={vfs}>
					<OpenWithProvider client={client}>{children}</OpenWithProvider>
				</VfsClientProvider>
			),
		});
		await waitFor(() => expect(hook.result.current.item).toBeNull());
		warn.mockRestore();
	});
});
