// Verifies the real SystemIconsClient over mocked plugins: the icon features and their reasons, the OS look and its changes, refreshing, addresses and the load probe
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createTauriSystemIconsClient } from './tauriSystemIconsClient';

const mime = vi.hoisted(() => ({
	getStatus: vi.fn(),
	refreshTypeIcons: vi.fn(),
}));
const appearance = vi.hoisted(() => ({
	getAppearance: vi.fn(),
	onAppearanceChanged: vi.fn(),
}));
vi.mock('@liminal-hq/plugin-mime-apps', async () => {
	const real = await vi.importActual<typeof import('@liminal-hq/plugin-mime-apps')>(
		'@liminal-hq/plugin-mime-apps',
	);
	return { ...real, ...mime };
});
vi.mock('@liminal-hq/plugin-system-appearance', () => appearance);

/** Lets the promises the client chains on the plugin's answer settle. */
async function flush(): Promise<void> {
	for (let turn = 0; turn < 5; turn += 1) await Promise.resolve();
}

const feature = (name: string, available: boolean, message: string | null = null) => ({
	name,
	available,
	reason: available ? null : 'no-icon-theme',
	message,
});

beforeEach(() => {
	for (const fn of [...Object.values(mime), ...Object.values(appearance)]) fn.mockReset();
});

describe('status', () => {
	it('reads the type and folder icon features, with the sentence for one that is missing', async () => {
		mime.getStatus.mockResolvedValue({
			features: [
				feature('typeIcons', true),
				feature('folderIcons', false, 'no icon theme is installed'),
			],
		});
		expect(await createTauriSystemIconsClient().status()).toEqual({
			typeIcons: { available: true, reason: null },
			folderIcons: { available: false, reason: 'no icon theme is installed' },
		});
	});

	it('rejects where the plugin is not there', async () => {
		mime.getStatus.mockRejectedValue(new Error('no plugin'));
		await expect(createTauriSystemIconsClient().status()).rejects.toThrow('no plugin');
	});
});

describe('the OS look', () => {
	it('is the icon theme and the colour scheme the appearance plugin reports', async () => {
		appearance.getAppearance.mockResolvedValue({ iconTheme: 'Papirus', colourScheme: 'dark' });
		expect(await createTauriSystemIconsClient().look()).toEqual({
			theme: 'Papirus',
			scheme: 'dark',
		});
	});

	it('is announced on each change until the listener is dropped, even one dropped before the plugin answered', async () => {
		let emit: (preferences: unknown) => void = () => undefined;
		const stop = vi.fn();
		appearance.onAppearanceChanged.mockImplementation(async (listener) => {
			emit = listener;
			return stop;
		});
		const client = createTauriSystemIconsClient();
		const seen: unknown[] = [];
		const unlisten = client.onLookChanged((look) => seen.push(look));
		await flush();
		emit({ iconTheme: 'breeze', colourScheme: 'light' });
		expect(seen).toEqual([{ theme: 'breeze', scheme: 'light' }]);
		unlisten();
		expect(stop).toHaveBeenCalledTimes(1);

		const early = client.onLookChanged(() => undefined);
		early();
		await flush();
		expect(stop).toHaveBeenCalledTimes(2);
	});
});

describe('refresh and addresses', () => {
	it('tells the plugin to forget its icons', async () => {
		mime.refreshTypeIcons.mockResolvedValue(undefined);
		await createTauriSystemIconsClient().refresh();
		expect(mime.refreshTypeIcons).toHaveBeenCalledTimes(1);
	});

	it('makes the typeicon address with the size, scale, theme and revision', () => {
		const url = createTauriSystemIconsClient().url(
			{ extension: 'pdf' },
			{ size: 24, scale: 2, theme: 'Breeze Dark', revision: 3 },
		);
		expect(url).toBe('typeicon://localhost/ext/pdf?size=24&scale=2&theme=Breeze%20Dark&v=3');
		expect(createTauriSystemIconsClient().url({ mime: 'application/pdf' }, { size: 16 })).toBe(
			'typeicon://localhost/mime/application%2Fpdf?size=16&scale=1',
		);
		expect(createTauriSystemIconsClient().url({ folder: 'home' }, {})).toBe(
			'typeicon://localhost/folder/home?size=32&scale=1',
		);
	});
});

describe('probe', () => {
	const images: Array<{ src: string; onload: (() => void) | null; onerror: (() => void) | null }> =
		[];
	beforeEach(() => {
		images.length = 0;
		vi.stubGlobal(
			'Image',
			class {
				src = '';
				onload: (() => void) | null = null;
				onerror: (() => void) | null = null;
				constructor() {
					images.push(this);
				}
			},
		);
	});
	afterEach(() => vi.unstubAllGlobals());

	it('says whether a picture came back, and says nothing once cancelled', () => {
		const client = createTauriSystemIconsClient();
		const done = vi.fn();
		client.probe('typeicon://localhost/ext/pdf?size=16', done);
		expect(images[0]!.src).toBe('typeicon://localhost/ext/pdf?size=16');
		images[0]!.onload?.();
		expect(done).toHaveBeenLastCalledWith(true);
		const missing = vi.fn();
		client.probe('typeicon://localhost/ext/zzz?size=16', missing);
		images[1]!.onerror?.();
		expect(missing).toHaveBeenLastCalledWith(false);
		const cancelled = vi.fn();
		const cancel = client.probe('typeicon://localhost/ext/png?size=16', cancelled);
		const late = images[2]!;
		const onload = late.onload;
		cancel();
		onload?.();
		expect(cancelled).not.toHaveBeenCalled();
	});
});
