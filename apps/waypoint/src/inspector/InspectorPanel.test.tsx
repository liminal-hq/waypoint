// Verifies the Inspector panel: tabs, the subject following the selection, folder sizes and their cancelling, unavailable fields, previews and the keyboard
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { createFakeOpenWithClient, fakeApp } from '../openWith/fakeOpenWithClient';
import { fakeDetails, type FakeEntry } from '../services/fakeDetailsClient';
import { clickEntry, mountInspector } from './inspectorHarness';
import { DEFAULT_WIDTH } from './inspectorStore';

afterEach(cleanup);

const TOTALS = {
	files: 3,
	folders: 1,
	bytes: 3_000,
	allocatedBytes: null,
	symlinksSkipped: 0,
	mountsSkipped: 0,
	placeholders: 0,
	unreadable: 0,
};

/** docs (1), music (2), notes.txt (3) and photo.jpg (4): the order the home folder lists them in. */
function specs(): Record<number, FakeEntry> {
	return {
		1: {
			details: fakeDetails({ name: 'docs', kind: 'directory', size: null, mode: 0o755 }),
			folderTotals: TOTALS,
		},
		2: {
			details: fakeDetails({ name: 'music', kind: 'directory', size: null }),
			folderTotals: TOTALS,
		},
		3: {
			details: fakeDetails({
				name: 'notes.txt',
				size: 1027,
				mimeType: 'text/plain',
				owner: 'scott',
				group: 'users',
				mode: 0o644,
				createdMs: 1_700_000_000_000,
				unavailable: [],
			}),
			text: 'hello inspector',
		},
		4: {
			details: fakeDetails({ name: 'photo.jpg', size: 1028, mimeType: 'image/jpeg' }),
			text: null,
		},
	};
}

const properties = { state: { tab: 'properties' as const } };
const field = (label: string) => screen.getByText(label, { selector: 'dt' }).nextElementSibling;

describe('the tabs', () => {
	it('are a tablist of Preview and Properties with a labelled tabpanel each', () => {
		return mountInspector(specs()).then(() => {
			const tabs = screen.getByRole('tablist', { name: 'Inspector tabs' });
			const [preview, props] = within(tabs).getAllByRole('tab');
			expect(preview).toHaveAttribute('aria-selected', 'true');
			expect(props).toHaveAttribute('aria-selected', 'false');
			expect(preview).toHaveAttribute('tabindex', '0');
			expect(props).toHaveAttribute('tabindex', '-1');
			const panel = screen.getByRole('tabpanel', { name: 'Preview' });
			expect(preview).toHaveAttribute('aria-controls', panel.id);
			expect(panel).toHaveAttribute('aria-labelledby', preview!.id);
		});
	});

	it('move with the arrow keys, Home and End, and the focus follows the selection', async () => {
		const { store } = await mountInspector(specs());
		const preview = screen.getByRole('tab', { name: 'Preview' });
		preview.focus();
		fireEvent.keyDown(preview, { key: 'ArrowRight' });
		expect(store.getState().tab).toBe('properties');
		expect(screen.getByRole('tab', { name: 'Properties' })).toHaveFocus();
		expect(screen.getByRole('tab', { name: 'Properties' })).toHaveAttribute('tabindex', '0');
		fireEvent.keyDown(screen.getByRole('tab', { name: 'Properties' }), { key: 'ArrowRight' });
		expect(store.getState().tab).toBe('preview');
		fireEvent.keyDown(screen.getByRole('tab', { name: 'Preview' }), { key: 'End' });
		expect(store.getState().tab).toBe('properties');
		fireEvent.keyDown(screen.getByRole('tab', { name: 'Properties' }), { key: 'Home' });
		expect(store.getState().tab).toBe('preview');
		fireEvent.keyDown(screen.getByRole('tab', { name: 'Preview' }), { key: 'ArrowLeft' });
		expect(store.getState().tab).toBe('properties');
	});

	it('change on a click, and the close button hides the panel', async () => {
		const { store } = await mountInspector(specs());
		fireEvent.click(screen.getByRole('tab', { name: 'Properties' }));
		expect(store.getState().tab).toBe('properties');
		fireEvent.click(screen.getByRole('button', { name: 'Hide the Inspector' }));
		expect(store.getState().open).toBe(false);
	});
});

describe('the divider', () => {
	it('widens and narrows by the arrow keys, and restores the default on a double click', async () => {
		const { store } = await mountInspector(specs());
		const divider = screen.getByRole('separator', { name: 'Resize the Inspector' });
		fireEvent.keyDown(divider, { key: 'ArrowLeft' });
		expect(store.getState().width).toBe(DEFAULT_WIDTH + 16);
		fireEvent.keyDown(divider, { key: 'ArrowRight', shiftKey: true });
		expect(store.getState().width).toBe(DEFAULT_WIDTH + 16 - 64);
		fireEvent.doubleClick(divider);
		expect(store.getState().width).toBe(DEFAULT_WIDTH);
		expect(divider).toHaveAttribute('aria-valuenow', String(DEFAULT_WIDTH));
	});
});

describe('Properties', () => {
	it('shows the current folder when nothing is selected', async () => {
		await mountInspector(specs(), properties);
		expect(field('Name')).toHaveTextContent('test');
		expect(field('Kind')).toHaveTextContent('Folder');
		expect(field('Location')).toHaveTextContent('/home/test');
		expect(field('Contains')).toHaveTextContent('4 items');
		await waitFor(() => expect(field('Free space')).toHaveTextContent('120 GB free of 500 GB'));
	});

	it('shows what the listing knows at once, within the 50 ms budget, and the rest as it is read', async () => {
		const { session } = await mountInspector(specs(), properties);
		act(() => clickEntry(session, 2));
		// No timer has run: the name, the kind and the size come from the listing alone.
		expect(field('Name')).toHaveTextContent('notes.txt');
		expect(field('Kind')).toHaveTextContent('Document');
		expect(field('Size')).toHaveTextContent('1 kB');
		expect(screen.queryByText('Owner', { selector: 'dt' })).toBeNull();
		await waitFor(() => expect(field('Owner')).toHaveTextContent('scott'));
		expect(field('Group')).toHaveTextContent('users');
		expect(field('Permissions')).toHaveTextContent('rw-r--r-- (0644)');
		expect(field('Content type')).toHaveTextContent('text/plain');
		expect(field('Location')).toHaveTextContent('/home/test');
		expect(field('Created')).toBeTruthy();
	});

	it('follows the selection, and never shows one file’s facts under another’s name', async () => {
		const { session, details } = await mountInspector(specs(), properties);
		act(() => clickEntry(session, 2));
		await waitFor(() => expect(field('Owner')).toHaveTextContent('scott'));
		act(() => clickEntry(session, 3));
		expect(field('Name')).toHaveTextContent('photo.jpg');
		expect(screen.queryByText('Owner', { selector: 'dt' })).toBeNull();
		await waitFor(() => expect(field('Content type')).toHaveTextContent('image/jpeg'));
		expect(details.calls.filter((call) => call.startsWith('details'))).toHaveLength(2);
	});

	it('reads nothing while the selection keeps moving', async () => {
		const { session, details } = await mountInspector(specs(), properties);
		for (const position of [0, 1, 2, 3]) act(() => clickEntry(session, position));
		await waitFor(() => expect(field('Name')).toHaveTextContent('photo.jpg'));
		await waitFor(() => expect(field('Content type')).toBeTruthy());
		expect(details.calls.filter((call) => call.startsWith('details'))).toHaveLength(1);
		expect(details.started).toHaveLength(0);
	});

	it('says Unavailable for a field the provider cannot report, and leaves out one the entry does not have', async () => {
		const unavailable = specs();
		unavailable[3] = {
			details: fakeDetails({
				name: 'notes.txt',
				size: 1027,
				mimeType: null,
				owner: null,
				group: null,
				mode: null,
				accessedMs: null,
				createdMs: null,
				allocatedSize: null,
				unavailable: ['owner', 'permissions', 'created'],
			}),
		};
		const { session } = await mountInspector(unavailable, properties);
		act(() => clickEntry(session, 2));
		await waitFor(() => expect(field('Owner')).toHaveTextContent('Unavailable'));
		expect(field('Permissions')).toHaveTextContent('Unavailable');
		expect(field('Created')).toHaveTextContent('Unavailable');
		// Group and Accessed are empty and not listed as unavailable: the entry has none.
		expect(screen.queryByText('Group', { selector: 'dt' })).toBeNull();
		expect(screen.queryByText('Accessed', { selector: 'dt' })).toBeNull();
		expect(screen.queryByText('Content type', { selector: 'dt' })).toBeNull();
		expect(screen.queryByText('On disk', { selector: 'dt' })).toBeNull();
	});

	it('shows a link’s target, and a broken link as such', async () => {
		const links = specs();
		links[3] = {
			details: fakeDetails({
				name: 'notes.txt',
				kind: 'symlink',
				resolvesTo: 'file',
				symlinkTarget: '../real.txt',
			}),
		};
		links[4] = {
			details: fakeDetails({
				name: 'photo.jpg',
				kind: 'symlink',
				resolvesTo: null,
				symlinkTarget: null,
			}),
		};
		const { session } = await mountInspector(links, properties);
		act(() => clickEntry(session, 2));
		await waitFor(() => expect(field('Link target')).toHaveTextContent('../real.txt'));
		act(() => clickEntry(session, 3));
		await waitFor(() => expect(field('Link target')).toHaveTextContent('Broken link'));
	});

	it('names the default application, and says so when there is none', async () => {
		const withApp = createFakeOpenWithClient({
			handlers: { mime: 'text/plain', default: fakeApp('gedit', 'Text Editor') },
		});
		const first = await mountInspector(specs(), { ...properties, openWith: withApp });
		act(() => clickEntry(first.session, 2));
		await waitFor(() => expect(field('Opens with')).toHaveTextContent('Text Editor'));
		cleanup();
		const without = createFakeOpenWithClient({ handlers: { default: null } });
		const second = await mountInspector(specs(), { ...properties, openWith: without });
		act(() => clickEntry(second.session, 2));
		await waitFor(() => expect(field('Opens with')).toHaveTextContent('No application'));
	});

	it('does not ask about the default application where the plugin cannot list them', async () => {
		const none = createFakeOpenWithClient({ features: [] });
		const { session } = await mountInspector(specs(), { ...properties, openWith: none });
		act(() => clickEntry(session, 2));
		await waitFor(() => expect(field('Owner')).toBeTruthy());
		expect(none.asked).toEqual([]);
		expect(screen.queryByText('Opens with', { selector: 'dt' })).toBeNull();
	});

	it('gives a count and a total size for a selection of several, and walks no folder', async () => {
		const { session, details } = await mountInspector(specs(), properties);
		act(() => {
			session.store.getState().click(0, 1);
			session.store.getState().toggleAt(2, 3);
			session.store.getState().toggleAt(3, 4);
		});
		expect(field('Selected')).toHaveTextContent('3 items selected');
		await waitFor(() => expect(field('Total size')).toHaveTextContent('2.1 kB'));
		await new Promise((resolve) => setTimeout(resolve, 250));
		expect(details.started).toHaveLength(0);
		expect(screen.getByText(/folders are not included/)).toBeInTheDocument();
	});

	it('shows only what the listing knows where there is no details service', async () => {
		const { session } = await mountInspector(specs(), { ...properties, withDetails: false });
		act(() => clickEntry(session, 2));
		expect(field('Name')).toHaveTextContent('notes.txt');
		await new Promise((resolve) => setTimeout(resolve, 200));
		expect(screen.queryByText('Owner', { selector: 'dt' })).toBeNull();
		act(() => clickEntry(session, 0));
		expect(field('Kind')).toHaveTextContent('Folder');
		expect(screen.queryByText('Size', { selector: 'dt' })).toBeNull();
	});
});

describe('a folder’s size', () => {
	it('is Calculating… until the stream reports, updates as it progresses and ends on the total', async () => {
		const { session, details } = await mountInspector(specs(), properties);
		act(() => clickEntry(session, 0));
		expect(field('Size')).toHaveTextContent('Calculating…');
		await waitFor(() => expect(details.started).toEqual([1]));
		act(() => details.advance(1, { ...TOTALS, bytes: 1_500 }));
		expect(field('Size')).toHaveTextContent('Calculating… 1.5 kB so far');
		act(() => details.finish(1));
		expect(field('Size')).toHaveTextContent('3 kB');
		expect(field('Size')).toHaveTextContent('3 files, 1 folder');
		// The completion, and only the completion, is announced.
		const live = screen.getByRole('status');
		expect(live).toHaveTextContent('docs is 3 kB');
	});

	it('does not announce progress', async () => {
		const { session, details } = await mountInspector(specs(), properties);
		act(() => clickEntry(session, 0));
		await waitFor(() => expect(details.started).toEqual([1]));
		act(() => details.advance(1, { ...TOTALS, bytes: 10 }));
		expect(screen.getByRole('status')).toBeEmptyDOMElement();
	});

	it('is cancelled when the selection changes, and the next folder is counted afresh', async () => {
		const { session, details } = await mountInspector(specs(), properties);
		act(() => clickEntry(session, 0));
		await waitFor(() => expect(details.started).toEqual([1]));
		act(() => clickEntry(session, 1));
		await waitFor(() => expect(details.cancelled).toEqual([1]));
		await waitFor(() => expect(details.started).toEqual([1, 2]));
		act(() => details.finish(2));
		expect(field('Size')).toHaveTextContent('3 kB');
		// The first run's late events do not reach the second folder.
		act(() => details.advance(1, { ...TOTALS, bytes: 99 }));
		expect(field('Size')).toHaveTextContent('3 kB');
	});

	it('is cancelled when the panel is closed', async () => {
		const { session, details, view } = await mountInspector(specs(), properties);
		act(() => clickEntry(session, 0));
		await waitFor(() => expect(details.started).toEqual([1]));
		view.unmount();
		expect(details.cancelled).toEqual([1]);
	});

	it('is cancelled when the panel moves to the Preview tab', async () => {
		const { session, details } = await mountInspector(specs(), properties);
		act(() => clickEntry(session, 0));
		await waitFor(() => expect(details.started).toEqual([1]));
		fireEvent.click(screen.getByRole('tab', { name: 'Preview' }));
		await waitFor(() => expect(details.cancelled).toEqual([1]));
	});

	it('is never started for a selection that moves on before it settles', async () => {
		const { session, details } = await mountInspector(specs(), properties);
		act(() => clickEntry(session, 0));
		act(() => clickEntry(session, 2));
		await new Promise((resolve) => setTimeout(resolve, 300));
		expect(details.started).toEqual([]);
		expect(details.cancelled).toEqual([]);
	});

	it('reads Unavailable when the count fails', async () => {
		const { session, details } = await mountInspector(specs(), properties);
		act(() => clickEntry(session, 0));
		await waitFor(() => expect(details.started).toEqual([1]));
		act(() => details.failRun(1));
		expect(field('Size')).toHaveTextContent('Unavailable');
	});
});

describe('Preview', () => {
	it('is the folder for nothing selected, and a count for several', async () => {
		const { session } = await mountInspector(specs());
		expect(screen.getByText('test')).toBeInTheDocument();
		expect(screen.getByText('4 items')).toBeInTheDocument();
		act(() => {
			session.store.getState().click(2, 3);
			session.store.getState().toggleAt(3, 4);
		});
		expect(screen.getByText('2 items selected')).toBeInTheDocument();
	});

	it('draws an image from its wpfile address once the selection settles, and not before', async () => {
		const { session } = await mountInspector(specs());
		act(() => clickEntry(session, 3));
		expect(screen.queryByRole('img', { name: 'Preview of photo.jpg' })).toBeNull();
		const image = await screen.findByRole('img', { name: 'Preview of photo.jpg' });
		expect(image.getAttribute('src')).toMatch(/^wpfile:\/\/localhost\//);
	});

	it('keeps the icon and loads nothing for an image too large to be worth it', async () => {
		const big = specs();
		big[4] = {
			details: fakeDetails({ name: 'photo.jpg', size: 50_000_000, mimeType: 'image/jpeg' }),
		};
		const { session } = await mountInspector(big);
		act(() => clickEntry(session, 3));
		await waitFor(() => expect(screen.getByText('photo.jpg')).toBeInTheDocument());
		await new Promise((resolve) => setTimeout(resolve, 300));
		expect(screen.queryByRole('img', { name: 'Preview of photo.jpg' })).toBeNull();
	});

	it('falls back to the icon when the image cannot be drawn', async () => {
		const { session } = await mountInspector(specs());
		act(() => clickEntry(session, 3));
		const image = await screen.findByRole('img', { name: 'Preview of photo.jpg' });
		fireEvent.error(image);
		await waitFor(() =>
			expect(screen.queryByRole('img', { name: 'Preview of photo.jpg' })).toBeNull(),
		);
		expect(screen.getByText('photo.jpg')).toBeInTheDocument();
	});

	it('shows the start of a text file in a monospace block', async () => {
		const { session, details } = await mountInspector(specs());
		act(() => clickEntry(session, 2));
		const block = await screen.findByLabelText('Start of notes.txt');
		expect(block).toHaveTextContent('hello inspector');
		expect(block.tagName).toBe('PRE');
		expect(details.calls.filter((call) => call.startsWith('textHead'))).toHaveLength(1);
		expect(screen.queryByText(/Showing the first/)).toBeNull();
	});

	it('says when only the first part of a long text file is shown, and reads no more than that', async () => {
		const long = specs();
		long[3] = {
			details: fakeDetails({ name: 'notes.txt', size: 100_000, mimeType: 'text/plain' }),
			text: 'x'.repeat(100_000),
		};
		const { session } = await mountInspector(long);
		act(() => clickEntry(session, 2));
		const block = await screen.findByLabelText('Start of notes.txt');
		expect(block.textContent!.length).toBe(16 * 1024);
		expect(screen.getByText('Showing the first 16 kB of the file')).toBeInTheDocument();
	});

	it('keeps the icon for a file that is not text although its type said it was', async () => {
		const binary = specs();
		binary[3] = {
			details: fakeDetails({ name: 'notes.txt', mimeType: 'text/plain' }),
			text: null,
		};
		const { session, details } = await mountInspector(binary);
		act(() => clickEntry(session, 2));
		await waitFor(() =>
			expect(details.calls.filter((call) => call.startsWith('textHead'))).toHaveLength(1),
		);
		expect(screen.queryByLabelText('Start of notes.txt')).toBeNull();
	});

	it('uses an audio element on the wpfile address, loading only its metadata', async () => {
		const audio = specs();
		audio[3] = { details: fakeDetails({ name: 'song.mp3', mimeType: 'audio/mpeg' }) };
		const { session } = await mountInspector(audio);
		act(() => clickEntry(session, 2));
		const player = await screen.findByLabelText('Audio player for notes.txt');
		expect(player.tagName).toBe('AUDIO');
		expect(player).toHaveAttribute('preload', 'metadata');
		expect(player.getAttribute('src')).toMatch(/^wpfile:\/\//);
		expect(player).not.toHaveAttribute('autoplay');
	});

	it('uses a video element for a video', async () => {
		const video = specs();
		video[3] = { details: fakeDetails({ name: 'clip.mp4', mimeType: 'video/mp4' }) };
		const { session } = await mountInspector(video);
		act(() => clickEntry(session, 2));
		const player = await screen.findByLabelText('Video player for notes.txt');
		expect(player.tagName).toBe('VIDEO');
		expect(player).toHaveAttribute('preload', 'metadata');
	});

	it('is the icon with the name and kind for anything else, and reads no content', async () => {
		const other = specs();
		other[3] = { details: fakeDetails({ name: 'notes.txt', mimeType: 'application/pdf' }) };
		const { session, details } = await mountInspector(other);
		act(() => clickEntry(session, 2));
		await waitFor(() =>
			expect(details.calls.filter((call) => call.startsWith('details'))).toHaveLength(1),
		);
		await new Promise((resolve) => setTimeout(resolve, 250));
		expect(screen.getByText('notes.txt')).toBeInTheDocument();
		expect(screen.getByText(/^Document/)).toBeInTheDocument();
		expect(details.calls.filter((call) => call.startsWith('textHead'))).toEqual([]);
		expect(screen.queryByRole('img')).toBeNull();
	});

	it('loads no player or text for a selection that moves on before it settles', async () => {
		const { session, details } = await mountInspector(specs());
		act(() => clickEntry(session, 2));
		act(() => clickEntry(session, 3));
		act(() => clickEntry(session, 0));
		await new Promise((resolve) => setTimeout(resolve, 300));
		expect(details.calls.filter((call) => call.startsWith('textHead'))).toEqual([]);
	});
});
