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
