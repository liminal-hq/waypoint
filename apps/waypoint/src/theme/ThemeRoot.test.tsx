// Verifies the root element follows the settings and the OS preferences live
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { NO_OS_APPEARANCE, type OsAppearance, type OsAppearanceSource } from './appearance';
import { prefersReducedMotion } from './motion';
import { ThemeRoot } from './ThemeRoot';

function fakeOs(initial: Partial<OsAppearance> = {}) {
	let value: OsAppearance = { ...NO_OS_APPEARANCE, ...initial };
	const listeners = new Set<() => void>();
	const source: OsAppearanceSource = {
		read: () => value,
		subscribe(listener) {
			listeners.add(listener);
			return () => listeners.delete(listener);
		},
	};
	return {
		source,
		change(next: Partial<OsAppearance>) {
			value = { ...value, ...next };
			for (const listener of listeners) listener();
		},
	};
}

afterEach(() => {
	const root = document.documentElement;
	for (const key of Object.keys(root.dataset)) delete root.dataset[key];
	root.removeAttribute('style');
});

describe('ThemeRoot', () => {
	it('sets the OS scheme on the root and follows it when it changes', async () => {
		const os = fakeOs({ scheme: 'light' });
		await act(async () => {
			render(
				<ThemeRoot client={createFakeSettingsClient()} os={os.source}>
					<p>content</p>
				</ThemeRoot>,
			);
		});
		expect(document.documentElement.dataset.theme).toBe('light');
		await act(async () => os.change({ scheme: 'dark', reducedMotion: true }));
		expect(document.documentElement.dataset.theme).toBe('dark');
		expect(document.documentElement.dataset.motion).toBe('reduce');
		expect(prefersReducedMotion()).toBe(true);
	});

	it('follows a setting the moment Rust announces it', async () => {
		const fake = createFakeSettingsClient();
		await act(async () => {
			render(
				<ThemeRoot client={fake} os={fakeOs({ scheme: 'light' }).source}>
					<p>content</p>
				</ThemeRoot>,
			);
		});
		await act(async () => {
			await fake.set({
				...DEFAULT_SETTINGS,
				appearance: { ...DEFAULT_SETTINGS.appearance, mode: 'dark', density: 'spacious' },
				accessibility: { ...DEFAULT_SETTINGS.accessibility, textSize: 130 },
			});
		});
		expect(document.documentElement.dataset).toMatchObject({ theme: 'dark', density: 'spacious' });
		expect(document.documentElement.style.getPropertyValue('--wp-text-scale')).toBe('1.3');
	});
});

describe('ThemeRoot transparency', () => {
	const lit = {
		...DEFAULT_SETTINGS,
		transparency: { ...DEFAULT_SETTINGS.transparency, enabled: true, solidWhenUnfocused: false },
	};
	const root = document.documentElement;

	async function mount(
		settings: typeof DEFAULT_SETTINGS,
		options: { available?: boolean; os?: Partial<OsAppearance> } = {},
	) {
		const os = fakeOs(options.os);
		await act(async () => {
			render(
				<ThemeRoot
					client={createFakeSettingsClient(settings)}
					os={os.source}
					opacityAvailable={() => Promise.resolve(options.available ?? true)}
				>
					<p>content</p>
				</ThemeRoot>,
			);
		});
		return os;
	}

	it('stays solid until the platform says windows can be see-through', async () => {
		await mount(lit, { available: false });
		expect(root.dataset.transparency).toBe('off');
		expect(root.dataset.transparencyReason).toBe('unavailable');
		expect(root.style.getPropertyValue('--wp-alpha-rows')).toBe('');
	});

	it('draws translucent when asked for, writing the opacity of every part on the root', async () => {
		await mount(lit);
		expect(root.dataset.transparency).toBe('on');
		expect(root.dataset.transparencyReason).toBeUndefined();
		// No theme colours in a test, so every part is at least the 70 % fallback.
		const read = (name: string) => Number(root.style.getPropertyValue(name));
		expect(read('--wp-alpha-title-bar')).toBe(0.82);
		expect(read('--wp-alpha-rows')).toBe(0.9);
		expect(read('--wp-alpha-sidebar')).toBe(0.94);
		expect(read('--wp-alpha-content')).toBe(1);
		expect(read('--wp-alpha-menu')).toBe(1);
		expect(root.dataset.menus).toBe('solid');
	});

	it('goes solid and takes the opacity off under high contrast and under reduced transparency', async () => {
		const os = await mount(lit, { os: { highContrast: true } });
		expect(root.dataset.transparency).toBe('off');
		expect(root.dataset.transparencyReason).toBe('high-contrast');
		expect(root.style.getPropertyValue('--wp-alpha-title-bar')).toBe('');
		await act(async () => os.change({ highContrast: false, reducedTransparency: true }));
		expect(root.dataset.transparencyReason).toBe('reduced-transparency');
		await act(async () => os.change({ reducedTransparency: false }));
		expect(root.dataset.transparency).toBe('on');
	});

	it('marks menus translucent when they are, and follows the settings live', async () => {
		const client = createFakeSettingsClient(lit);
		await act(async () => {
			render(
				<ThemeRoot
					client={client}
					os={fakeOs().source}
					opacityAvailable={() => Promise.resolve(true)}
				>
					<p>content</p>
				</ThemeRoot>,
			);
		});
		await act(async () =>
			client.change({
				...lit,
				transparency: { ...lit.transparency, menus: true, menuOpacity: 80, opacity: 60 },
			}),
		);
		expect(root.dataset.menus).toBe('translucent');
		expect(Number(root.style.getPropertyValue('--wp-alpha-menu'))).toBe(0.8);
		expect(Number(root.style.getPropertyValue('--wp-alpha-title-bar'))).toBe(0.7);
		await act(async () =>
			client.change({ ...lit, transparency: { ...lit.transparency, enabled: false } }),
		);
		expect(root.dataset.transparency).toBe('off');
		expect(root.dataset.menus).toBeUndefined();
	});

	it('goes solid when the window loses focus and back when it returns, if asked to', async () => {
		await mount({ ...lit, transparency: { ...lit.transparency, solidWhenUnfocused: true } });
		await act(async () => {
			globalThis.dispatchEvent(new Event('focus'));
		});
		expect(root.dataset.transparency).toBe('on');
		await act(async () => {
			globalThis.dispatchEvent(new Event('blur'));
		});
		expect(root.dataset.transparency).toBe('off');
		expect(root.dataset.transparencyReason).toBe('unfocused');
		await act(async () => {
			globalThis.dispatchEvent(new Event('focus'));
		});
		expect(root.dataset.transparency).toBe('on');
	});
});
