// Tests for the store that opens the batch rename dialog and for the host that shows it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Sources } from '@liminal-hq/waypoint-protocol/generated/Sources';
import { cleanup, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { act } from 'react';
import { afterEach, describe, expect, it } from 'vitest';
import { BatchRenameHost } from './BatchRenameHost';
import { batchRenameStore, createBatchRenameStore, openBatchRename } from './batchRenameStore';
import { FakeBatchRenameApi, previewOf } from './fakeBatchRenameApi';

const sources: Sources = { kind: 'locations', locations: [] };

afterEach(() => {
	cleanup();
	batchRenameStore.getState().close();
	document.documentElement.removeAttribute('style');
});

describe('the store', () => {
	it('opens for a selection and closes', () => {
		const store = createBatchRenameStore();
		expect(store.getState().selection).toBeNull();
		store.getState().open({ sources, count: 3 });
		expect(store.getState().selection).toEqual({ sources, count: 3 });
		store.getState().close();
		expect(store.getState().selection).toBeNull();
	});

	it('is opened by openBatchRename, which the menus and the shortcut call', () => {
		openBatchRename({ sources });
		expect(batchRenameStore.getState().selection?.sources).toBe(sources);
	});
});

describe('BatchRenameHost', () => {
	it('shows nothing until something asks, then shows the dialog for that selection', async () => {
		const api = new FakeBatchRenameApi();
		render(<BatchRenameHost api={api} debounceMs={0} />);
		expect(document.querySelector('dialog')).toBeNull();
		act(() => openBatchRename({ sources, count: 4 }));
		const dialog = await screen.findByRole('dialog', { name: 'Batch rename' });
		expect(dialog).toHaveAccessibleDescription(/Rename 4 items/);
		await waitFor(() => expect(api.previews).toHaveLength(1));
		expect(api.previews[0]!.sources).toBe(sources);
	});

	it('closes on Escape and forgets the selection, so the next opening starts fresh', async () => {
		const api = new FakeBatchRenameApi();
		const user = userEvent.setup();
		render(<BatchRenameHost api={api} debounceMs={0} />);
		act(() => openBatchRename({ sources }));
		await screen.findByRole('dialog');
		await user.type(screen.getByLabelText('Find'), 'typed');
		await user.keyboard('{Escape}');
		expect(document.querySelector('dialog')).toBeNull();
		expect(batchRenameStore.getState().selection).toBeNull();
		act(() => openBatchRename({ sources }));
		expect(await screen.findByLabelText('Find')).toHaveValue('');
	});

	it('closes after Apply and passes the announcement on', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = () => previewOf([{ from: 'a', to: 'b' }]);
		const messages: string[] = [];
		const user = userEvent.setup();
		render(<BatchRenameHost api={api} announce={(m) => messages.push(m)} debounceMs={0} />);
		act(() => openBatchRename({ sources }));
		const apply = await screen.findByRole('button', { name: 'Apply' });
		await waitFor(() => expect(apply).toBeEnabled());
		await user.click(apply);
		await waitFor(() => expect(document.querySelector('dialog')).toBeNull());
		expect(messages).toEqual(['Renaming 1 item']);
		expect(api.applied).toHaveLength(1);
	});

	it('follows a store of its own', async () => {
		const store = createBatchRenameStore();
		render(<BatchRenameHost api={new FakeBatchRenameApi()} store={store} debounceMs={0} />);
		act(() => store.getState().open({ sources }));
		expect(await screen.findByRole('dialog')).toBeInTheDocument();
		expect(batchRenameStore.getState().selection).toBeNull();
	});
});
