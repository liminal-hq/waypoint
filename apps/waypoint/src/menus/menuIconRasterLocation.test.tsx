// Verifies the rasteriser draws a location's icon (a component that reads the icon set and the window's contexts, and in the System set loads its picture) as the window's menu shows it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { act, cleanup, render } from '@testing-library/react';
import { createElement } from 'react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { ConnectionsProvider } from '../connections/ConnectionsContext';
import { FakeConnectionsClient, serverLocation } from '../connections/fakeConnectionsClient';
import { FOLDER_PALETTE } from '../icons/portage/portagePalette';
import { configureSystemIcons } from '../icons/systemIcons';
import { LocationIcon } from '../nav/LocationIcon';
import { createFakeSystemIconsClient } from '../services/fakeSystemIconsClient';
import { createTree } from '../test/workspaceHarness';
import { createIconStage, type HostedIconStage } from './iconStage';
import { IconStagePortal } from './MenuIconStageHost';
import {
	createIconRasteriser,
	ICON_SETTLE_MS,
	type IconRasteriser,
	type RasteriserEnvironment,
} from './menuIconRaster';

const root = document.documentElement;

const local: Location = { uri: 'file:///home/test/docs', display: '/home/test/docs' };
const other: Location = { uri: 'file:///home/test/music', display: '/home/test/music' };
const server = serverLocation('sftp://me@nas.lan', '/media');

const iconOf = (location: Location) => createElement(LocationIcon, { location });

interface Drawn {
	markup: string[];
	images: string[];
	environment: RasteriserEnvironment;
	/** What `drawImage` answers; `null` is a picture the canvas could not read. */
	imageAnswer: { value: 'pixels' | 'null' | 'throw' };
}

function environment(): Drawn {
	const markup: string[] = [];
	const images: string[] = [];
	const imageAnswer: Drawn['imageAnswer'] = { value: 'pixels' };
	return {
		markup,
		images,
		imageAnswer,
		environment: {
			scale: () => 1,
			draw: async (svg, px) => {
				markup.push(svg);
				return new Array<number>(px * px * 4).fill(7);
			},
			drawImage: async (url, px) => {
				images.push(url);
				if (imageAnswer.value === 'throw') throw new Error('tainted canvas');
				return imageAnswer.value === 'null' ? null : new Array<number>(px * px * 4).fill(9);
			},
		},
	};
}

/** A rasteriser whose stage is drawn from inside the providers a window has. */
function hosted(drawn: Drawn): { rasterise: IconRasteriser; stage: HostedIconStage } {
	const stage = createIconStage();
	render(
		<VfsClientProvider client={createTree()}>
			<ConnectionsProvider client={new FakeConnectionsClient({})}>
				<IconStagePortal stage={stage} />
			</ConnectionsProvider>
		</VfsClientProvider>,
	);
	return { rasterise: createIconRasteriser(drawn.environment, stage), stage };
}

/** Lets what the stage has asked of the plugin (its status, then the picture) come back. */
async function turns(count = 8): Promise<void> {
	await act(async () => {
		for (let turn = 0; turn < count; turn += 1) await Promise.resolve();
	});
}

beforeEach(() => {
	delete root.dataset.iconTheme;
});

afterEach(() => {
	cleanup();
	configureSystemIcons(null);
	for (const key of ['iconTheme', 'folderColour', 'theme']) delete root.dataset[key];
	document.body.replaceChildren();
	vi.restoreAllMocks();
});

describe('a location’s icon in the native menu', () => {
	it('is the Waypoint folder glyph in the Waypoint set, drawn as markup', async () => {
		root.dataset.iconTheme = 'waypoint';
		const drawn = environment();
		const { rasterise } = hosted(drawn);
		const picture = await rasterise(iconOf(local));
		expect(picture).toMatchObject({ width: 16, height: 16 });
		expect(drawn.markup).toHaveLength(1);
		expect(drawn.markup[0]).toContain('viewBox="0 0 16 16"');
		expect(drawn.images).toEqual([]);
	});

	it('is the Portage folder in the chosen colour, and draws again for another colour', async () => {
		root.dataset.iconTheme = 'portage';
		root.dataset.folderColour = 'red';
		const drawn = environment();
		const { rasterise } = hosted(drawn);
		await rasterise(iconOf(local));
		expect(drawn.markup[0]).toContain('viewBox="0 0 64 64"');
		expect(drawn.markup[0]?.toLowerCase()).toContain(FOLDER_PALETTE.red.light as string);
		await act(async () => {
			root.dataset.folderColour = 'purple';
		});
		await rasterise(iconOf(local));
		expect(drawn.markup).toHaveLength(2);
		expect(drawn.markup[1]?.toLowerCase()).toContain(FOLDER_PALETTE.purple.light as string);
		expect(drawn.markup[1]?.toLowerCase()).not.toContain(FOLDER_PALETTE.red.light as string);
	});

	it('is drawn in the dark tone on a dark window', async () => {
		root.dataset.iconTheme = 'portage';
		root.dataset.folderColour = 'red';
		root.dataset.theme = 'dark';
		const drawn = environment();
		const { rasterise } = hosted(drawn);
		await rasterise(iconOf(local));
		expect(drawn.markup[0]?.toLowerCase()).toContain(FOLDER_PALETTE.red.dark as string);
	});

	describe('in the System set', () => {
		it('waits for the system’s picture and draws it from its address', async () => {
			root.dataset.iconTheme = 'system';
			const fake = createFakeSystemIconsClient();
			configureSystemIcons(fake);
			const drawn = environment();
			const { rasterise } = hosted(drawn);
			const pending = rasterise(iconOf(local));
			await turns();
			expect(fake.probed).toEqual(['fake://folder/plain?size=16&scale=1&theme=Adwaita&tone=light']);
			await act(async () => fake.settle(true));
			const picture = await pending;
			expect(drawn.images).toEqual(fake.probed);
			expect(drawn.markup).toEqual([]);
			expect(picture).toMatchObject({ width: 16, height: 16 });
			expect(picture?.rgba[0]).toBe(9);
		});

		it('keeps the picture for the next folder of the look, and asks again when the OS theme changes', async () => {
			root.dataset.iconTheme = 'system';
			const fake = createFakeSystemIconsClient();
			configureSystemIcons(fake);
			const drawn = environment();
			const { rasterise } = hosted(drawn);
			const first = rasterise(iconOf(local));
			await turns();
			await act(async () => fake.settle(true));
			const picture = await first;
			expect(await rasterise(iconOf(other))).toBe(picture);
			expect(drawn.images).toHaveLength(1);
			await act(async () => fake.changeLook({ theme: 'Papirus' }));
			await turns();
			const again = rasterise(iconOf(local));
			await turns();
			await act(async () => fake.settle(true));
			expect(await again).not.toBe(picture);
			expect(drawn.images).toHaveLength(2);
			expect(drawn.images[1]).toContain('theme=Papirus');
		});

		it('draws the Waypoint glyph when the picture does not come in time, and does not hold the next icon', async () => {
			root.dataset.iconTheme = 'system';
			configureSystemIcons(createFakeSystemIconsClient());
			const drawn = environment();
			const { rasterise } = hosted(drawn);
			const started = Date.now();
			const picture = await rasterise(iconOf(local));
			expect(Date.now() - started).toBeGreaterThanOrEqual(ICON_SETTLE_MS - 50);
			expect(picture).toMatchObject({ width: 16, height: 16 });
			expect(drawn.images).toEqual([]);
			expect(drawn.markup).toHaveLength(1);
			expect(drawn.markup[0]).toContain('viewBox="0 0 16 16"');
			const next = Date.now();
			await rasterise(iconOf(other));
			expect(Date.now() - next).toBeLessThan(ICON_SETTLE_MS / 2);
		});

		it('draws the Waypoint glyph where the system has no icon for the folder', async () => {
			root.dataset.iconTheme = 'system';
			const fake = createFakeSystemIconsClient();
			configureSystemIcons(fake);
			const drawn = environment();
			const { rasterise } = hosted(drawn);
			const pending = rasterise(iconOf(local));
			await turns();
			await act(async () => fake.settle(false));
			expect(await pending).toMatchObject({ width: 16, height: 16 });
			expect(drawn.images).toEqual([]);
			expect(drawn.markup).toHaveLength(1);
		});

		it('draws the glyph at once where the system offers no folder icons at all', async () => {
			root.dataset.iconTheme = 'system';
			configureSystemIcons(
				createFakeSystemIconsClient({
					status: { folderIcons: { available: false, reason: 'no' } },
				}),
			);
			const drawn = environment();
			const { rasterise } = hosted(drawn);
			const started = Date.now();
			expect(await rasterise(iconOf(local))).not.toBeNull();
			expect(Date.now() - started).toBeLessThan(ICON_SETTLE_MS / 2);
			expect(drawn.markup).toHaveLength(1);
		});

		it.each(['null', 'throw'] as const)(
			'falls back to the glyph it replaced, and tries the picture again next time, when the canvas gives %s',
			async (answer) => {
				root.dataset.iconTheme = 'system';
				const fake = createFakeSystemIconsClient();
				configureSystemIcons(fake);
				const drawn = environment();
				drawn.imageAnswer.value = answer;
				vi.spyOn(console, 'debug').mockImplementation(() => {});
				const { rasterise } = hosted(drawn);
				const pending = rasterise(iconOf(local));
				await turns();
				await act(async () => fake.settle(true));
				expect(await pending).toMatchObject({ width: 16, height: 16 });
				expect(drawn.images).toHaveLength(1);
				expect(drawn.markup).toHaveLength(1);
				drawn.imageAnswer.value = 'pixels';
				const again = await rasterise(iconOf(local));
				expect(drawn.images).toHaveLength(2);
				expect(again?.rgba[0]).toBe(9);
			},
		);
	});

	it('draws again when the icon set changes, and reuses what it drew for a set it has seen', async () => {
		const drawn = environment();
		const { rasterise } = hosted(drawn);
		root.dataset.iconTheme = 'waypoint';
		const waypoint = await rasterise(iconOf(local));
		await act(async () => {
			root.dataset.iconTheme = 'portage';
		});
		const portage = await rasterise(iconOf(local));
		expect(portage).not.toBe(waypoint);
		expect(drawn.markup).toHaveLength(2);
		await act(async () => {
			root.dataset.iconTheme = 'waypoint';
		});
		expect(await rasterise(iconOf(other))).toBe(waypoint);
		expect(drawn.markup).toHaveLength(2);
	});

	it('is the server glyph for a location on a server, with the dot for its state', async () => {
		root.dataset.iconTheme = 'portage';
		const drawn = environment();
		const { rasterise, stage } = hosted(drawn);
		const folder = await rasterise(iconOf(local));
		const remote = await rasterise(iconOf(server));
		expect(remote).not.toBeNull();
		expect(remote).not.toBe(folder);
		expect(drawn.markup).toHaveLength(2);
		expect(stage.element.querySelector('[data-tone] svg')).not.toBeNull();
		expect(stage.element.querySelector('svg[data-group]')).toBeNull();
	});

	it('is the plain folder for a server location where the stage is not in the window’s tree', async () => {
		root.dataset.iconTheme = 'waypoint';
		const drawn = environment();
		const stage = createIconStage();
		const rasterise = createIconRasteriser(drawn.environment, stage);
		expect(await rasterise(iconOf(server))).not.toBeNull();
		expect(stage.element.querySelector('svg[data-group="folder"]')).not.toBeNull();
		stage.dispose();
	});
});
