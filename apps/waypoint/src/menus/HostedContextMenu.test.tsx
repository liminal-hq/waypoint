// Verifies the seam: the page's menu while the setting is off, the system's when it is on, and the page's again whenever the system's cannot be shown
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem, SelectableMenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { SettingsProvider, useSettingsReady } from '../settings/SettingsContext';
import { createFakeNativeMenuClient } from './fakeNativeMenuClient';
import { HostedContextMenu, NATIVE_PREPARE_MS, NATIVE_SETTLE_MS } from './HostedContextMenu';
import { NativeMenuProvider } from './NativeMenuContext';
import type { IconRasteriser } from './menuIconRaster';

afterEach(() => {
	cleanup();
	vi.useRealTimers();
	vi.restoreAllMocks();
});

const ITEMS: MenuItem[] = [
	{ type: 'action', id: 'open', label: 'Open' },
	{ type: 'separator' },
	{ type: 'checkbox', id: 'hidden', label: 'Show hidden', checked: false },
];

const noPictures: IconRasteriser = async () => null;

interface Setup {
	enabled?: boolean;
	items?: MenuItem[];
	answer?: Array<string | null | Error>;
	platform?: string;
	rasterise?: IconRasteriser;
	withClient?: boolean;
	settling?: boolean;
	position?: { x: number; y: number };
}

function Gate({ children }: { children: React.ReactNode }) {
	return useSettingsReady() ? <>{children}</> : null;
}

async function setup(options: Setup = {}) {
	const native = createFakeNativeMenuClient();
	native.answer(...(options.answer ?? [null]));
	const settings = createFakeSettingsClient({
		...DEFAULT_SETTINGS,
		experimental: { ...DEFAULT_SETTINGS.experimental, nativeContextMenus: options.enabled ?? true },
	});
	const onSelect = vi.fn<(item: SelectableMenuItem) => void>();
	const onClose = vi.fn();

	function Harness({
		position,
		settling,
	}: {
		position: { x: number; y: number };
		settling: boolean;
	}) {
		return (
			<SettingsProvider client={settings}>
				<NativeMenuProvider
					client={options.withClient === false ? undefined : native}
					platform={options.platform ?? 'linux'}
					rasterise={options.rasterise ?? noPictures}
				>
					<Gate>
						<button>Opener</button>
						<HostedContextMenu
							items={options.items ?? ITEMS}
							position={position}
							ariaLabel="Entry menu"
							settling={settling}
							onSelect={onSelect}
							onClose={onClose}
						/>
					</Gate>
				</NativeMenuProvider>
			</SettingsProvider>
		);
	}
	const initial = {
		position: options.position ?? { x: 120, y: 80 },
		settling: options.settling ?? false,
	};
	const view = render(<Harness {...initial} />);
	return {
		native,
		onSelect,
		onClose,
		settings,
		rerender: (next: Partial<typeof initial>) => view.rerender(<Harness {...initial} {...next} />),
	};
}

describe('HostedContextMenu', () => {
	it('is the page’s menu while the setting is off, and never asks the system', async () => {
		const { native } = await setup({ enabled: false });
		expect(await screen.findByRole('menu', { name: 'Entry menu' })).toBeInTheDocument();
		expect(native.calls).toEqual([]);
	});

	it('is the page’s menu in a window without the service or on a platform not tried', async () => {
		await setup({ withClient: false });
		expect(await screen.findByRole('menu')).toBeInTheDocument();
		cleanup();
		const { native } = await setup({ platform: 'macos' });
		expect(await screen.findByRole('menu')).toBeInTheDocument();
		expect(native.calls).toEqual([]);
	});

	it('asks the system for the menu at the pointer and draws nothing of its own', async () => {
		const { native } = await setup({ answer: [null] });
		await waitFor(() => expect(native.calls).toHaveLength(1));
		expect(native.calls[0]?.at).toEqual({ x: 120, y: 80 });
		expect(native.calls[0]?.items.map((item) => item.kind)).toEqual([
			'action',
			'separator',
			'checkbox',
		]);
		expect(screen.queryByRole('menu')).toBeNull();
	});

	it('runs the same onSelect with the chosen item, then closes', async () => {
		const { onSelect, onClose } = await setup({ answer: ['hidden'] });
		await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
		expect(onSelect).toHaveBeenCalledTimes(1);
		expect(onSelect).toHaveBeenCalledWith(ITEMS[2]);
		expect(onSelect.mock.invocationCallOrder[0]).toBeLessThan(
			onClose.mock.invocationCallOrder[0] ?? 0,
		);
	});

	it('only closes when the menu is dismissed, and gives the focus back to what had it', async () => {
		const { onSelect, onClose } = await setup({ answer: [null] });
		await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
		expect(onSelect).not.toHaveBeenCalled();
		cleanup();
		const opener = document.createElement('button');
		document.body.append(opener);
		opener.focus();
		const again = await setup({ answer: [null] });
		await waitFor(() => expect(again.onClose).toHaveBeenCalled());
		cleanup();
		expect(document.activeElement).toBe(opener);
		opener.remove();
	});

	it('ignores an answer that names no item of the menu', async () => {
		const { onSelect, onClose } = await setup({ answer: ['not-an-item'] });
		await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
		expect(onSelect).not.toHaveBeenCalled();
	});

	it('shows the page’s menu when the system could not show one, and keeps to the page’s from then on', async () => {
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		const { native } = await setup({ answer: [new Error('closed after 3 ms without a choice')] });
		expect(await screen.findByRole('menu', { name: 'Entry menu' })).toBeInTheDocument();
		expect(native.calls).toHaveLength(1);
	});

	it('does not ask the system again in a window where it has failed', async () => {
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		const native = createFakeNativeMenuClient();
		native.answer(new Error('no'));
		const settings = createFakeSettingsClient({
			...DEFAULT_SETTINGS,
			experimental: { ...DEFAULT_SETTINGS.experimental, nativeContextMenus: true },
		});
		function Menu({ n }: { n: number }) {
			return (
				<HostedContextMenu
					key={n}
					items={ITEMS}
					position={{ x: n, y: n }}
					ariaLabel={`Menu ${n}`}
					onSelect={() => {}}
					onClose={() => {}}
				/>
			);
		}
		function Harness() {
			const [n, setN] = useState(1);
			return (
				<SettingsProvider client={settings}>
					<NativeMenuProvider client={native} platform="windows" rasterise={noPictures}>
						<Gate>
							<button onClick={() => setN(n + 1)}>Next</button>
							<Menu n={n} />
						</Gate>
					</NativeMenuProvider>
				</SettingsProvider>
			);
		}
		render(<Harness />);
		expect(await screen.findByRole('menu', { name: 'Menu 1' })).toBeInTheDocument();
		fireEvent.click(screen.getByRole('button', { name: 'Next' }));
		expect(await screen.findByRole('menu', { name: 'Menu 2' })).toBeInTheDocument();
		expect(native.calls).toHaveLength(1);
	});

	it('keeps the page’s menu for items the system’s menu cannot show', async () => {
		vi.spyOn(console, 'debug').mockImplementation(() => {});
		const { native } = await setup({ items: [{ type: 'section', label: 'Only a heading' }] });
		expect(await screen.findByRole('menu')).toBeInTheDocument();
		expect(native.calls).toEqual([]);
	});

	it('passes the converted ids, labels and the pictures made for the icons', async () => {
		const icon = <svg data-testid="open-icon" />;
		const made = { width: 16, height: 16, rgba: new Array<number>(16 * 16 * 4).fill(1) };
		const { native } = await setup({
			items: [{ type: 'action', id: 'open', label: 'Open', icon }],
			rasterise: async (node) => (node === icon ? made : null),
		});
		await waitFor(() => expect(native.calls).toHaveLength(1));
		expect(native.calls[0]?.items[0]).toMatchObject({ kind: 'action', id: 'open', icon: made });
	});

	it('shows the page’s menu when the icons take too long, so no menu is lost', async () => {
		vi.spyOn(console, 'debug').mockImplementation(() => {});
		vi.useFakeTimers({ shouldAdvanceTime: true });
		const { native } = await setup({
			items: [{ type: 'action', id: 'open', label: 'Open', icon: <svg /> }],
			rasterise: () => new Promise(() => {}),
		});
		expect(screen.queryByRole('menu')).toBeNull();
		await act(async () => {
			await vi.advanceTimersByTimeAsync(NATIVE_PREPARE_MS + 10);
		});
		expect(screen.getByRole('menu')).toBeInTheDocument();
		expect(native.calls).toEqual([]);
	});

	it('waits for items that are still arriving, and gives up on them after a moment', async () => {
		vi.useFakeTimers({ shouldAdvanceTime: true });
		const { native, rerender } = await setup({ settling: true, answer: [null] });
		expect(native.calls).toEqual([]);
		expect(screen.queryByRole('menu')).toBeNull();
		rerender({ settling: false });
		await waitFor(() => expect(native.calls).toHaveLength(1));
		cleanup();

		const held = await setup({ settling: true });
		await act(async () => {
			await vi.advanceTimersByTimeAsync(NATIVE_SETTLE_MS + 10);
		});
		expect(screen.getByRole('menu')).toBeInTheDocument();
		expect(held.native.calls).toEqual([]);
	});

	it('shows the menu again from the new place when it is opened elsewhere', async () => {
		const { native, rerender } = await setup({ answer: [null] });
		await waitFor(() => expect(native.calls).toHaveLength(1));
		rerender({ position: { x: 300, y: 40 } });
		await waitFor(() => expect(native.calls).toHaveLength(2));
		expect(native.calls[1]?.at).toEqual({ x: 300, y: 40 });
	});
});
