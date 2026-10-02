// Verifies Quick Look end to end through the list: opening with Space, stepping, closing, each kind of content and its fallbacks
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ListView } from '../browse/ListView';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { SettingsProvider } from '../settings/SettingsContext';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { FakeDetailsClient, fakeDetails } from '../services/fakeDetailsClient';
import { FakeVfsClient, makeEntry } from '../services/fakeVfsClient';
import { FOLDER, stubLayout } from '../test/browseHarness';
import { createFakeThumbnailsClient } from '../thumbnails/fakeThumbnailsClient';
import { ThumbnailsProvider } from '../thumbnails/ThumbnailsContext';
import { DetailsClientProvider } from '../inspector/DetailsClientContext';
import { QuickLookHost } from './QuickLookHost';

// In listing order: the folder first, then by name.
const ENTRIES: Entry[] = [
	makeEntry(10, 'albums', { kind: 'directory' }),
	makeEntry(11, 'b-photo.png'),
	makeEntry(12, 'c-notes.txt'),
	makeEntry(13, 'd-song.mp3'),
	makeEntry(14, 'e-clip.mp4'),
	makeEntry(15, 'f-report.pdf'),
	makeEntry(16, 'g-pipe', { kind: 'other' }),
	makeEntry(17, 'h-last.txt'),
];

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
});

interface Setup {
	details?: FakeDetailsClient | undefined;
	onOpen?: (entry: Entry) => void;
	thumbnails?: ReturnType<typeof createFakeThumbnailsClient>;
}

function fakeDetailsFor(): FakeDetailsClient {
	const details = new FakeDetailsClient();
	for (const entry of ENTRIES) {
		details.setEntry(1, entry.id, {
			details: fakeDetails({
				name: entry.name,
				kind: entry.kind === 'other' ? 'other' : entry.kind,
			}),
			text: entry.name.endsWith('.txt') ? `contents of ${entry.name}` : null,
		});
	}
	return details;
}

function renderBrowser({ details = fakeDetailsFor(), onOpen, thumbnails }: Setup = {}) {
	const client = new FakeVfsClient();
	client.setFolder(FOLDER, ENTRIES);
	const settings = createFakeSettingsClient({
		...DEFAULT_SETTINGS,
		previews: { ...DEFAULT_SETTINGS.previews, thumbnails: true },
	});
	const view = render(
		<SettingsProvider client={settings}>
			<ThumbnailsProvider client={thumbnails}>
				<VfsClientProvider client={client}>
					<DetailsClientProvider client={details}>
						<ListView location={FOLDER} onOpen={onOpen} />
						<QuickLookHost />
					</DetailsClientProvider>
				</VfsClientProvider>
			</ThumbnailsProvider>
		</SettingsProvider>,
	);
	return { view, details };
}

const key = (target: Element, name: string, init: KeyboardEventInit = {}) =>
	fireEvent.keyDown(target, { key: name, ...init });

/** Focuses the list and moves to the entry at `position`. */
async function focusEntry(position: number): Promise<HTMLElement> {
	const list = await screen.findByRole('listbox', { name: 'Files' });
	await screen.findAllByRole('option');
	list.focus();
	for (let at = 0; at < position; at++) key(list, 'ArrowDown');
	return list;
}

const overlay = () => screen.queryByRole('dialog');
const settle = () => act(() => new Promise((resolve) => setTimeout(resolve, 20)));

describe('opening and closing', () => {
	it('opens on Space as a dialog named for the file, with its position', async () => {
		renderBrowser();
		const list = await focusEntry(2);
		const prevented = !key(list, ' ');
		expect(prevented).toBe(true);
		const dialog = await screen.findByRole('dialog', { name: 'c-notes.txt' });
		expect(within(dialog).getByText('3 of 8')).toBeInTheDocument();
	});

	it('closes on Esc and puts the focus back on the list', async () => {
		renderBrowser();
		const list = await focusEntry(1);
		key(list, ' ');
		await screen.findByRole('dialog');
		key(document.activeElement ?? document.body, 'Escape');
		await waitFor(() => expect(overlay()).toBeNull());
		expect(document.activeElement).toBe(list);
	});

	it('closes on Space inside the overlay, and the list keeps its focused entry', async () => {
		renderBrowser();
		const list = await focusEntry(1);
		key(list, ' ');
		const dialog = await screen.findByRole('dialog');
		key(dialog.querySelector('[class*="stage"]')!, ' ');
		await waitFor(() => expect(overlay()).toBeNull());
		expect(list.getAttribute('aria-activedescendant')).toMatch(/-row-1$/);
	});

	it('does not open with a held key or a modified Space', async () => {
		renderBrowser();
		const list = await focusEntry(1);
		key(list, ' ', { repeat: true });
		key(list, ' ', { ctrlKey: true });
		expect(overlay()).toBeNull();
	});

	it('does nothing where the window has no details client', async () => {
		const client = new FakeVfsClient();
		client.setFolder(FOLDER, ENTRIES);
		render(
			<VfsClientProvider client={client}>
				<ListView location={FOLDER} />
				<QuickLookHost />
			</VfsClientProvider>,
		);
		const list = await screen.findByRole('listbox', { name: 'Files' });
		await screen.findAllByRole('option');
		list.focus();
		key(list, 'ArrowDown');
		expect(key(list, ' ')).toBe(false);
		expect(overlay()).toBeNull();
	});

	it('leaves Space to a field being typed in', async () => {
		renderBrowser();
		const list = await focusEntry(1);
		const field = document.createElement('input');
		list.append(field);
		field.focus();
		fireEvent.keyDown(field, { key: ' ' });
		expect(overlay()).toBeNull();
	});
});

describe('stepping', () => {
	it('moves to the next and previous entry, with the list following and the change announced', async () => {
		renderBrowser();
		const list = await focusEntry(1);
		key(list, ' ');
		const dialog = await screen.findByRole('dialog', { name: 'b-photo.png' });
		const stage = dialog.querySelector<HTMLElement>('[tabindex="-1"][class*="stage"]')!;
		key(stage, 'ArrowRight');
		await screen.findByRole('dialog', { name: 'c-notes.txt' });
		expect(screen.getByText('3 of 8')).toBeInTheDocument();
		expect(screen.getByText('c-notes.txt, 3 of 8')).toBeInTheDocument();
		expect(screen.getByText('c-notes.txt, 3 of 8')).toHaveAttribute('aria-live', 'polite');
		expect(list.getAttribute('aria-activedescendant')).toMatch(/-row-2$/);
		key(screen.getByRole('dialog').querySelector('[class*="stage"]')!, 'ArrowLeft');
		await screen.findByRole('dialog', { name: 'b-photo.png' });
	});

	it('steps past an entry that cannot be previewed, and stops at the end', async () => {
		renderBrowser();
		await focusEntry(5);
		key(screen.getByRole('listbox'), ' ');
		await screen.findByRole('dialog', { name: 'f-report.pdf' });
		const stage = () => screen.getByRole('dialog').querySelector('[class*="stage"]')!;
		key(stage(), 'ArrowRight');
		await screen.findByRole('dialog', { name: 'h-last.txt' });
		key(stage(), 'ArrowRight');
		await settle();
		expect(screen.getByRole('dialog', { name: 'h-last.txt' })).toBeInTheDocument();
	});

	it('steps with the footer buttons', async () => {
		renderBrowser();
		await focusEntry(1);
		key(screen.getByRole('listbox'), ' ');
		await screen.findByRole('dialog', { name: 'b-photo.png' });
		fireEvent.click(screen.getByRole('button', { name: 'Next item' }));
		await screen.findByRole('dialog', { name: 'c-notes.txt' });
		fireEvent.click(screen.getByRole('button', { name: 'Previous item' }));
		await screen.findByRole('dialog', { name: 'b-photo.png' });
	});

	it('asks only for the neighbours’ thumbnails, ahead of a step', async () => {
		const thumbnails = createFakeThumbnailsClient();
		renderBrowser({ thumbnails });
		await focusEntry(1);
		key(screen.getByRole('listbox'), ' ');
		await screen.findByRole('dialog');
		await waitFor(() => expect(thumbnails.batches.some((b) => b.size === 'x-large')).toBe(true));
		const keys = thumbnails.batches.filter((b) => b.size === 'x-large').flatMap((b) => b.keys);
		// The current entry and the previous one (a folder, so none) and the next one.
		expect(keys.some((k) => k.startsWith('11:'))).toBe(true);
		expect(keys.some((k) => k.startsWith('12:'))).toBe(true);
		expect(keys.some((k) => k.startsWith('14:'))).toBe(false);
	});
});

describe('content by kind', () => {
	const open = async (position: number) => {
		await focusEntry(position);
		key(screen.getByRole('listbox'), ' ');
		return screen.findByRole('dialog');
	};

	it('shows an image from the preview address, scaled to fit, over its thumbnail', async () => {
		const thumbnails = createFakeThumbnailsClient();
		renderBrowser({ thumbnails });
		const dialog = await open(1);
		const image = within(dialog).getByRole('img', { name: 'b-photo.png' });
		expect(image).toHaveAttribute('src', 'wpfile://localhost/1-11');
		await waitFor(() => expect(thumbnails.batches.length).toBeGreaterThan(0));
		act(() => thumbnails.ready(thumbnails.batches[0]!.keys[0]!, 'thumb://localhost/x/a.png'));
		await waitFor(() =>
			expect(dialog.querySelector('img[src="thumb://localhost/x/a.png"]')).not.toBeNull(),
		);
	});

	it('shows text in monospace, with a notice when it was cut', async () => {
		const details = fakeDetailsFor();
		details.setEntry(1, 12, {
			details: fakeDetails({ name: 'c-notes.txt' }),
			text: 'x'.repeat(300 * 1024),
		});
		renderBrowser({ details });
		const dialog = await open(2);
		await waitFor(() => expect(dialog.querySelector('pre')).not.toBeNull());
		expect(dialog.querySelector('pre')?.textContent?.length).toBe(256 * 1024);
		expect(within(dialog).getByTestId('quicklook-truncated')).toHaveTextContent(
			'Showing the start of the file only.',
		);
	});

	it('shows a short text without a notice', async () => {
		renderBrowser();
		const dialog = await open(2);
		await waitFor(() =>
			expect(dialog.querySelector('pre')).toHaveTextContent('contents of c-notes.txt'),
		);
		expect(within(dialog).queryByTestId('quicklook-truncated')).toBeNull();
	});

	it('plays audio and video natively over the preview address', async () => {
		renderBrowser();
		let dialog = await open(3);
		const audio = dialog.querySelector('audio')!;
		expect(audio).toHaveAttribute('src', 'wpfile://localhost/1-13');
		expect(audio.hasAttribute('controls')).toBe(true);
		key(dialog.querySelector('[class*="stage"]')!, 'ArrowRight');
		await screen.findByRole('dialog', { name: 'e-clip.mp4' });
		dialog = screen.getByRole('dialog');
		const video = dialog.querySelector('video')!;
		expect(video).toHaveAttribute('src', 'wpfile://localhost/1-14');
		expect(video.hasAttribute('controls')).toBe(true);
	});

	it('shows a PDF as its thumbnail with the name, kind and size', async () => {
		const thumbnails = createFakeThumbnailsClient();
		renderBrowser({ thumbnails });
		const dialog = await open(5);
		expect(within(dialog).getByText('PDF document')).toBeInTheDocument();
		expect(within(dialog).getByText('f-report.pdf', { selector: 'p' })).toBeInTheDocument();
		expect(within(dialog).getByText('Size')).toBeInTheDocument();
		expect(dialog.querySelector('video, audio, pre')).toBeNull();
	});

	it('shows a folder as its name and kind', async () => {
		renderBrowser();
		const dialog = await open(0);
		expect(within(dialog).getByText('Folder', { selector: 'dd' })).toBeInTheDocument();
		expect(within(dialog).queryByText('Size')).toBeNull();
	});
});

describe('when a preview fails', () => {
	const open = async (position: number) => {
		await focusEntry(position);
		key(screen.getByRole('listbox'), ' ');
		return screen.findByRole('dialog');
	};

	it('falls back to the facts when an image cannot be decoded', async () => {
		renderBrowser();
		const dialog = await open(1);
		fireEvent.error(within(dialog).getByRole('img', { name: 'b-photo.png' }));
		expect(await within(dialog).findByText('This item cannot be previewed.')).toBeInTheDocument();
		expect(within(dialog).getByText('Image')).toBeInTheDocument();
	});

	it('falls back when media cannot be played', async () => {
		renderBrowser();
		const dialog = await open(3);
		fireEvent.error(dialog.querySelector('audio')!);
		expect(
			await within(dialog).findByText('This media cannot be played here.'),
		).toBeInTheDocument();
	});

	it('says there is no preview for a file that is not text, and when the text cannot be read', async () => {
		const details = fakeDetailsFor();
		details.setEntry(1, 17, { details: fakeDetails({ name: 'h-last.txt' }), text: null });
		renderBrowser({ details });
		const dialog = await open(7);
		expect(
			await within(dialog).findByText('No preview is available for this type of item.'),
		).toBeInTheDocument();

		const broken = fakeDetailsFor();
		vi.spyOn(broken, 'readTextHead').mockRejectedValue({
			kind: 'io',
			message: 'x',
			location: null,
		});
		cleanup();
		renderBrowser({ details: broken });
		const again = await open(2);
		expect(await within(again).findByText('The text could not be read.')).toBeInTheDocument();
	});
});

describe('the Open button', () => {
	it('opens the entry as the view does and closes the overlay', async () => {
		const onOpen = vi.fn();
		renderBrowser({ onOpen });
		await focusEntry(2);
		key(screen.getByRole('listbox'), ' ');
		await screen.findByRole('dialog');
		fireEvent.click(screen.getByRole('button', { name: 'Open' }));
		expect(onOpen).toHaveBeenCalledWith(expect.objectContaining({ name: 'c-notes.txt' }), 1);
		await waitFor(() => expect(overlay()).toBeNull());
	});

	it('offers no Open With where the window has no Open With client', async () => {
		renderBrowser();
		await focusEntry(2);
		key(screen.getByRole('listbox'), ' ');
		await screen.findByRole('dialog');
		expect(screen.queryByRole('button', { name: 'Open With…' })).toBeNull();
	});
});
