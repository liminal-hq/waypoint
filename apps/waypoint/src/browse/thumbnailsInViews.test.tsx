// Verifies thumbnails in the grid and the list: what is requested, scrolling, leaving, and the fall back to icons
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { SettingsProvider } from '../settings/SettingsContext';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { syntheticEntries } from '../services/fakeVfsClient';
import { clientWith, FOLDER, stubLayout } from '../test/browseHarness';
import {
	brokenStatus,
	createFakeThumbnailsClient,
	type FakeThumbnailsClient,
} from '../thumbnails/fakeThumbnailsClient';
import { ThumbnailsProvider } from '../thumbnails/ThumbnailsContext';
import type { PluginStatus } from '../thumbnails/thumbnailsClient';
import type { VfsClient } from '../services/vfsClient';
import { GridView } from './GridView';
import { ListView } from './ListView';
import { useListingSession } from './useListingSession';
import { useVfsClient, VfsClientProvider } from './VfsClientContext';

let restoreLayout: () => void;
beforeEach(() => {
	// 400 px tall, 600 px wide: at 96 px icons, four columns and 2.5 rows.
	restoreLayout = stubLayout(400, 600);
});
afterEach(() => {
	cleanup();
	restoreLayout();
	vi.restoreAllMocks();
});

function GridHost() {
	const state = useListingSession(useVfsClient(), FOLDER);
	return <GridView state={state} size={96} />;
}

interface Options {
	status?: PluginStatus;
	thumbnails?: boolean;
	withClient?: boolean;
}

function renderWith(
	view: 'grid' | 'list',
	client: VfsClient,
	thumbs: FakeThumbnailsClient,
	o: Options = {},
) {
	const settings = createFakeSettingsClient({
		...DEFAULT_SETTINGS,
		previews: { ...DEFAULT_SETTINGS.previews, thumbnails: o.thumbnails ?? true },
	});
	return render(
		<SettingsProvider client={settings}>
			<ThumbnailsProvider client={o.withClient === false ? undefined : thumbs}>
				<VfsClientProvider client={client}>
					{view === 'grid' ? <GridHost /> : <ListView location={FOLDER} />}
				</VfsClientProvider>
			</ThumbnailsProvider>
		</SettingsProvider>,
	);
}

const settle = () => act(() => new Promise((resolve) => setTimeout(resolve, 20)));
const pictures = () => document.querySelectorAll('img[src^="thumb://"]');
const options = () => screen.queryAllByRole('option');

describe('the grid', () => {
	it('asks for the pictures in view and a screen around them, never for a folder', async () => {
		const { client, entries } = clientWith(30);
		const thumbs = createFakeThumbnailsClient();
		renderWith('grid', client, thumbs);
		await waitFor(() => expect(thumbs.batches.length).toBeGreaterThan(0));
		const asked = thumbs.batches.flatMap((batch) => batch.keys);
		expect(asked.length).toBeGreaterThan(0);
		expect(asked.length).toBeLessThan(30);
		const folders = new Set(
			entries.filter((entry) => entry.kind === 'directory').map((entry) => String(entry.id)),
		);
		expect(asked.some((key) => folders.has(key.split(':')[0]!))).toBe(false);
		expect(thumbs.batches[0]?.handle).toBe(1);
		// Large enough for a 96 px icon at a pixel ratio of 1.
		expect(thumbs.batches[0]?.size).toBe('normal');
	});

	it('shows a picture when it is ready, with no alternative text, and keeps the name as the label', async () => {
		const { client } = clientWith(100);
		const thumbs = createFakeThumbnailsClient();
		renderWith('grid', client, thumbs);
		await waitFor(() => expect(thumbs.batches.length).toBeGreaterThan(0));
		const key = thumbs.batches[0]!.keys[0]!;
		act(() => thumbs.ready(key, 'thumb://localhost/normal/abc.png'));
		await waitFor(() => expect(pictures()).toHaveLength(1));
		const image = pictures()[0] as HTMLImageElement;
		expect(image.getAttribute('alt')).toBe('');
		expect(image.getAttribute('src')).toBe('thumb://localhost/normal/abc.png');
		expect(image.closest('[role="option"]')).toHaveTextContent(/\w/);
	});

	it('keeps the icon where a thumbnail failed, was skipped or cannot be loaded', async () => {
		const { client } = clientWith(100);
		const thumbs = createFakeThumbnailsClient();
		renderWith('grid', client, thumbs);
		await waitFor(() => expect(thumbs.batches.length).toBeGreaterThan(0));
		const [failed, skipped, broken] = thumbs.batches[0]!.keys;
		act(() => {
			thumbs.fail(failed!);
			thumbs.skip(skipped!, 'remote');
			thumbs.ready(broken!, 'thumb://localhost/normal/bad.png');
		});
		await waitFor(() => expect(pictures()).toHaveLength(1));
		fireEvent.error(pictures()[0]!);
		await waitFor(() => expect(pictures()).toHaveLength(0));
		// Every item still has its icon (the grid draws it as a picture).
		for (const option of options()) {
			if (option.getAttribute('data-placeholder') === null) {
				expect(option.querySelector('[data-icon-picture][data-group]')).not.toBeNull();
			}
		}
	});

	it('asks for the pictures that scrolled into view and withdraws the ones it left', async () => {
		// Folders sort first and have no picture, so a listing of files only is scrolled through.
		const { client } = clientWith(0);
		client.setFolder(
			FOLDER,
			syntheticEntries(2000).filter((entry) => entry.kind !== 'directory'),
		);
		const thumbs = createFakeThumbnailsClient();
		renderWith('grid', client, thumbs);
		await waitFor(() => expect(thumbs.batches.length).toBeGreaterThan(0));
		const first = thumbs.batches[0]!;
		const scroller = screen.getByRole('listbox').parentElement!;
		scroller.scrollTop = 160 * 100;
		fireEvent.scroll(scroller);
		await waitFor(() => expect(thumbs.batches.length).toBeGreaterThan(1));
		await waitFor(() => expect(first.cancelled).toBe(true));
		expect(thumbs.batches.at(-1)!.cancelled).toBe(false);
	});

	it('withdraws everything when the view goes away', async () => {
		const { client } = clientWith(100);
		const thumbs = createFakeThumbnailsClient();
		const view = renderWith('grid', client, thumbs);
		await waitFor(() => expect(thumbs.batches.length).toBeGreaterThan(0));
		view.unmount();
		await settle();
		expect(thumbs.batches.every((batch) => batch.cancelled)).toBe(true);
	});

	it('asks for nothing when "Show thumbnails" is off, the plugin is unavailable or there is no client', async () => {
		for (const options of [
			{ thumbnails: false },
			{ status: brokenStatus() },
			{ withClient: false },
		] satisfies Options[]) {
			const { client } = clientWith(100);
			const thumbs = createFakeThumbnailsClient(options.status);
			const view = renderWith('grid', client, thumbs, options);
			await screen.findByRole('listbox');
			await settle();
			expect(thumbs.batches).toHaveLength(0);
			expect(screen.getAllByRole('option').length).toBeGreaterThan(0);
			view.unmount();
		}
	});
});

describe('the list', () => {
	function withRowHeight(height: string) {
		const real = window.getComputedStyle.bind(window);
		vi.spyOn(window, 'getComputedStyle').mockImplementation((element, pseudo) => {
			const style = real(element, pseudo);
			return new Proxy(style, {
				get(target, property) {
					if (property === 'getPropertyValue') {
						return (name: string) =>
							name === '--wp-row-height' ? height : target.getPropertyValue(name);
					}
					const value = Reflect.get(target, property, target) as unknown;
					return typeof value === 'function' ? value.bind(target) : value;
				},
			});
		});
	}

	it('keeps icons at the usual row height', async () => {
		withRowHeight('28px');
		const { client } = clientWith(100);
		const thumbs = createFakeThumbnailsClient();
		renderWith('list', client, thumbs);
		await waitFor(() => expect(options().length).toBeGreaterThan(0));
		await settle();
		expect(thumbs.batches).toHaveLength(0);
	});

	it('shows thumbnails at a large row height', async () => {
		withRowHeight('44px');
		const { client } = clientWith(100);
		const thumbs = createFakeThumbnailsClient();
		renderWith('list', client, thumbs);
		await waitFor(() => expect(thumbs.batches.length).toBeGreaterThan(0));
		expect(thumbs.batches[0]?.size).toBe('normal');
		act(() => thumbs.ready(thumbs.batches[0]!.keys[0]!, 'thumb://localhost/normal/a.png'));
		await waitFor(() => expect(pictures()).toHaveLength(1));
		expect(pictures()[0]!.getAttribute('alt')).toBe('');
	});
});
