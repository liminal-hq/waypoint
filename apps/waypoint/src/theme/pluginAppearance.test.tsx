// Verifies the plugin-backed OS appearance source, and that the root follows it under the settings
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, render } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';
import {
	createFakeOsAppearanceClient,
	NEUTRAL_PREFERENCES,
} from '../services/fakeOsAppearanceClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS, type Settings } from '../services/settingsClient';
import { accentFor } from './accent';
import { NO_OS_APPEARANCE, type OsAppearance, type OsAppearanceSource } from './appearance';
import { clampTextScale, pluginOsAppearance } from './pluginAppearance';
import { ThemeRoot } from './ThemeRoot';

const SOURCES = {
	colourScheme: 'portal',
	accent: 'portal',
	contrast: 'portal',
	reducedMotion: 'portal',
	reducedTransparency: 'portal',
	textScale: 'portal',
	iconTheme: null,
} as const;

/** A media-query stand-in whose answer the test sets. */
function fakeMedia(initial: Partial<OsAppearance> = {}) {
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
		get listenerCount() {
			return listeners.size;
		},
		change(next: Partial<OsAppearance>) {
			value = { ...value, ...next };
			for (const listener of listeners) listener();
		},
	};
}

const settle = () => act(async () => undefined);

describe('pluginOsAppearance', () => {
	it('reads the media queries until the plugin has answered', () => {
		const media = fakeMedia({ scheme: 'dark' });
		const source = pluginOsAppearance(createFakeOsAppearanceClient(), media.source);
		expect(source.read()).toMatchObject({ scheme: 'dark', textScale: 1, accent: null });
	});

	it('applies the plugin initial answer and tells the listener', async () => {
		const source = pluginOsAppearance(
			createFakeOsAppearanceClient({
				colourScheme: 'dark',
				textScale: 1.5,
				accent: '#3584e4',
				reducedMotion: true,
				contrast: 'more',
				reducedTransparency: true,
				sources: SOURCES,
			}),
			fakeMedia({ scheme: 'light' }).source,
		);
		let calls = 0;
		const stop = source.subscribe(() => (calls += 1));
		await settle();
		expect(calls).toBe(1);
		expect(source.read()).toEqual({
			scheme: 'dark',
			highContrast: true,
			reducedMotion: true,
			reducedTransparency: true,
			textScale: 1.5,
			accent: '#3584e4',
		});
		stop();
	});

	it('follows a live change, and ignores an older revision', async () => {
		const client = createFakeOsAppearanceClient({ textScale: 1.25, sources: SOURCES });
		const source = pluginOsAppearance(client, fakeMedia().source);
		source.subscribe(() => undefined);
		await settle();
		const newer = client.change({ textScale: 1.5 });
		expect(source.read().textScale).toBe(1.5);
		client.emit({ ...newer, revision: newer.revision - 1, textScale: 1.1 });
		expect(source.read().textScale).toBe(1.5);
	});

	it('keeps the media query for a preference the plugin has no source for', async () => {
		const client = createFakeOsAppearanceClient({
			reducedMotion: false,
			sources: { ...NEUTRAL_PREFERENCES.sources, textScale: 'portal' },
			textScale: 1.3,
		});
		const source = pluginOsAppearance(
			client,
			fakeMedia({ scheme: 'dark', reducedMotion: true }).source,
		);
		source.subscribe(() => undefined);
		await settle();
		expect(source.read()).toMatchObject({ scheme: 'dark', reducedMotion: true, textScale: 1.3 });
	});

	it('falls back to the media queries when the read fails, and still hears events', async () => {
		const client = createFakeOsAppearanceClient();
		client.failGet();
		const media = fakeMedia({ scheme: 'light' });
		const source = pluginOsAppearance(client, media.source);
		let calls = 0;
		source.subscribe(() => (calls += 1));
		await settle();
		expect(source.read().scheme).toBe('light');
		media.change({ scheme: 'dark' });
		expect(source.read().scheme).toBe('dark');
		client.change({ textScale: 1.4, sources: SOURCES });
		expect(source.read().textScale).toBe(1.4);
		expect(calls).toBe(2);
	});

	it('clamps a text scale that is out of range or not a number', () => {
		expect(clampTextScale(0.5)).toBe(1);
		expect(clampTextScale(1.5)).toBe(1.5);
		expect(clampTextScale(99)).toBe(3);
		expect(clampTextScale(Number.NaN)).toBe(1);
		expect(clampTextScale(Number.POSITIVE_INFINITY)).toBe(1);
	});

	it('lets go of both listeners when the last one stops', async () => {
		const client = createFakeOsAppearanceClient();
		const media = fakeMedia();
		const source = pluginOsAppearance(client, media.source);
		const a = source.subscribe(() => undefined);
		const b = source.subscribe(() => undefined);
		expect(client.listenerCount).toBe(1);
		a();
		expect(client.listenerCount).toBe(1);
		b();
		expect(client.listenerCount).toBe(0);
		expect(media.listenerCount).toBe(0);
	});
});

afterEach(() => {
	const root = document.documentElement;
	for (const key of Object.keys(root.dataset)) delete root.dataset[key];
	root.removeAttribute('style');
});

async function mount(settings: Settings, client: ReturnType<typeof createFakeOsAppearanceClient>) {
	const fake = createFakeSettingsClient();
	fake.change(settings);
	await act(async () => {
		render(
			<ThemeRoot client={fake} os={pluginOsAppearance(client, fakeMedia().source)}>
				<p>content</p>
			</ThemeRoot>,
		);
	});
	await settle();
}

const withAccess = (change: Partial<Settings['accessibility']>): Settings => ({
	...DEFAULT_SETTINGS,
	accessibility: { ...DEFAULT_SETTINGS.accessibility, ...change },
});

describe('ThemeRoot with the plugin source', () => {
	const root = () => document.documentElement;

	it('scales the text by the OS scale, live', async () => {
		const client = createFakeOsAppearanceClient({ textScale: 1.5, sources: SOURCES });
		await mount(DEFAULT_SETTINGS, client);
		expect(root().style.getPropertyValue('--wp-text-scale')).toBe('1.5');
		await act(async () => void client.change({ textScale: 2 }));
		expect(root().style.getPropertyValue('--wp-text-scale')).toBe('2');
		await act(async () => void client.change({ textScale: 40 }));
		expect(root().style.getPropertyValue('--wp-text-scale')).toBe('3');
	});

	it('uses the larger of the Settings text size and the OS scale', async () => {
		const client = createFakeOsAppearanceClient({ textScale: 1.15, sources: SOURCES });
		await mount(withAccess({ textSize: 130 }), client);
		expect(root().style.getPropertyValue('--wp-text-scale')).toBe('1.3');
	});

	it('applies the OS accent, with the text colour that keeps contrast, when the setting follows the system', async () => {
		const client = createFakeOsAppearanceClient({ accent: '#3584e4', sources: SOURCES });
		await mount(
			{
				...DEFAULT_SETTINGS,
				appearance: { ...DEFAULT_SETTINGS.appearance, accent: { kind: 'os' } },
			},
			client,
		);
		const expected = accentFor('#3584e4')!;
		expect(root().style.getPropertyValue('--wp-accent')).toBe(expected.fill);
		expect(root().style.getPropertyValue('--wp-accent-contrast')).toBe(expected.text);
		await act(async () => void client.change({ accent: '#f6d32d' }));
		const yellow = accentFor('#f6d32d')!;
		expect(root().style.getPropertyValue('--wp-accent')).toBe(yellow.fill);
		expect(root().style.getPropertyValue('--wp-accent-contrast')).toBe(yellow.text);
	});

	it('keeps the brand accent when the setting does not follow the system', async () => {
		const client = createFakeOsAppearanceClient({ accent: '#3584e4', sources: SOURCES });
		await mount(DEFAULT_SETTINGS, client);
		expect(root().style.getPropertyValue('--wp-accent')).toBe('');
	});

	it('lets an explicit setting win over the OS for scheme and motion', async () => {
		const client = createFakeOsAppearanceClient({
			colourScheme: 'dark',
			reducedMotion: true,
			sources: SOURCES,
		});
		await mount(
			{
				...withAccess({ reducedMotion: 'off' }),
				appearance: { ...DEFAULT_SETTINGS.appearance, mode: 'light' },
			},
			client,
		);
		expect(root().dataset.theme).toBe('light');
		expect(root().dataset.motion).toBe('full');
	});

	it('follows the OS scheme, contrast and motion by default', async () => {
		const client = createFakeOsAppearanceClient({
			colourScheme: 'dark',
			reducedMotion: true,
			contrast: 'more',
			sources: SOURCES,
		});
		await mount(DEFAULT_SETTINGS, client);
		expect(root().dataset.theme).toBe('dark');
		expect(root().dataset.motion).toBe('reduce');
		expect(root().dataset.contrast).toBe('high');
	});

	it('stays at the defaults when the plugin fails', async () => {
		const client = createFakeOsAppearanceClient();
		client.failGet();
		await mount(DEFAULT_SETTINGS, client);
		expect(root().style.getPropertyValue('--wp-text-scale')).toBe('1');
		expect(root().dataset.motion).toBe('full');
	});
});
