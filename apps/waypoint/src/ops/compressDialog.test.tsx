// Verifies the compress dialog, its host and the commands that open it from the window
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { FakeArchiveClient } from '../archives/fakeArchiveClient';
import { FakeConnectionsClient } from '../connections/fakeConnectionsClient';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { makeEntry } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { createTree, HOME, renderWorkspace } from '../test/workspaceHarness';
import { CompressDialog } from './CompressDialog';
import { CompressHost } from './CompressHost';
import { createCompressStore, pickCompression } from './compressStore';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

describe('the dialog', () => {
	const open = (name = 'photos', format?: 'zip' | 'tarXz') => {
		const onConfirm = vi.fn();
		const onCancel = vi.fn();
		render(
			<CompressDialog
				options={{ name, count: 3, ...(format ? { format } : {}) }}
				onConfirm={onConfirm}
				onCancel={onCancel}
			/>,
		);
		return { onConfirm, onCancel };
	};

	it('starts on the name, selected, and compresses with Enter into a zip', async () => {
		const { onConfirm } = open();
		const name = screen.getByRole('textbox', { name: 'Archive name' }) as HTMLInputElement;
		await waitFor(() => expect(name).toHaveFocus());
		expect(name.value).toBe('photos');
		fireEvent.submit(name.closest('form')!);
		expect(onConfirm).toHaveBeenCalledWith({ name: 'photos.zip', format: 'zip' });
	});

	it('puts the extension of the chosen format on the name, once', () => {
		const { onConfirm } = open('src.tar.gz');
		const name = screen.getByRole('textbox', { name: 'Archive name' }) as HTMLInputElement;
		// The old archive extension is taken off, so choosing another format changes the end only.
		expect(name.value).toBe('src');
		fireEvent.change(screen.getByRole('combobox', { name: 'Format' }), {
			target: { value: 'sevenZ' },
		});
		fireEvent.click(screen.getByRole('button', { name: 'Compress' }));
		expect(onConfirm).toHaveBeenCalledWith({ name: 'src.7z', format: 'sevenZ' });
	});

	it('lists every format by its words, with the extension', () => {
		open();
		const options = screen.getAllByRole('option').map((option) => option.textContent);
		expect(options).toEqual([
			'Zip (.zip)',
			'Tar, gzip (.tar.gz)',
			'Tar, xz (.tar.xz)',
			'Tar, bzip2 (.tar.bz2)',
			'Tar, uncompressed (.tar)',
			'7z (.7z)',
		]);
	});

	it('refuses an empty name or one with a slash where it is typed, in words read out', () => {
		const { onConfirm } = open();
		const name = screen.getByRole('textbox', { name: 'Archive name' });
		fireEvent.change(name, { target: { value: '' } });
		expect(screen.getByRole('alert')).toHaveTextContent('Give the archive a name.');
		expect(name).toHaveAttribute('aria-invalid', 'true');
		fireEvent.click(screen.getByRole('button', { name: 'Compress' }));
		fireEvent.change(name, { target: { value: 'a/b' } });
		expect(screen.getByRole('alert')).toHaveTextContent('A name cannot hold a slash.');
		fireEvent.click(screen.getByRole('button', { name: 'Compress' }));
		expect(onConfirm).not.toHaveBeenCalled();
	});

	it('cancels with the button and with Escape', () => {
		const { onCancel } = open();
		fireEvent.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(onCancel).toHaveBeenCalledTimes(1);
		fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
		expect(onCancel.mock.calls.length).toBeGreaterThanOrEqual(1);
	});
});

describe('asking from a command', () => {
	it('resolves to the choice, or to nothing where no dialog is mounted', async () => {
		const store = createCompressStore();
		expect(await pickCompression({ name: 'x', count: 1 }, store)).toBeNull();
		render(<CompressHost store={store} />);
		let answer: Awaited<ReturnType<typeof pickCompression>> = null;
		void pickCompression({ name: 'x', count: 1 }, store).then((choice) => (answer = choice));
		const box = await screen.findByRole('textbox', { name: 'Archive name' });
		fireEvent.change(box, { target: { value: 'bundle' } });
		fireEvent.click(screen.getByRole('button', { name: 'Compress' }));
		await waitFor(() => expect(answer).toEqual({ name: 'bundle.zip', format: 'zip' }));
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('answers the first question with nothing when a second is asked, and when the host goes', async () => {
		const store = createCompressStore();
		const view = render(<CompressHost store={store} />);
		const first = pickCompression({ name: 'a', count: 1 }, store);
		const second = pickCompression({ name: 'b', count: 1 }, store);
		expect(await first).toBeNull();
		view.unmount();
		expect(await second).toBeNull();
	});
});

describe('in the window', () => {
	it('opens from the item menu, and sends a compress job when the dialog is confirmed', async () => {
		const client = createTree();
		client.setFolder(HOME, [makeEntry(3, 'notes.txt'), makeEntry(4, 'photo.jpg')]);
		const ops = createFakeOpsClient();
		await renderWorkspace(client, undefined, undefined, {
			ops,
			archives: new FakeArchiveClient(),
			connections: new FakeConnectionsClient(),
		});
		fireEvent.contextMenu(await screen.findByText('notes.txt'));
		fireEvent.click(await screen.findByRole('menuitem', { name: 'Compress…' }));
		const name = await screen.findByRole('textbox', { name: 'Archive name' });
		expect((name as HTMLInputElement).value).toBe('notes');
		fireEvent.change(screen.getByRole('combobox', { name: 'Format' }), {
			target: { value: 'tarGz' },
		});
		act(() => {
			fireEvent.click(screen.getByRole('button', { name: 'Compress' }));
		});
		await waitFor(() => expect(ops.calls.some((call) => call[0] === 'submit')).toBe(true));
		const submit = ops.calls.find((call) => call[0] === 'submit')![1] as {
			kind: { kind: string };
			name: string;
			destination: { uri: string };
			archive: unknown;
		};
		expect(submit.kind).toEqual({ kind: 'compress' });
		expect(submit.name).toBe('notes.tar.gz');
		expect(submit.destination.uri).toBe(HOME.uri);
		expect(submit.archive).toEqual({ kind: 'compress', format: 'tarGz' });
	});
});
