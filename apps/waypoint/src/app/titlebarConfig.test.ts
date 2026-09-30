// Verifies the title bar configuration for OS preferences and for the platform fallback
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TitlebarPreferences } from '@liminal-hq/plugin-system-appearance';
import { describe, expect, it } from 'vitest';
import { titlebarConfigFor } from './titlebarConfig';

function preferences(overrides: Partial<TitlebarPreferences> = {}): TitlebarPreferences {
	return {
		buttonLayout: { start: [], end: ['minimise', 'maximise', 'close'] },
		actions: { doubleClick: 'toggleMaximise', middleClick: 'none', rightClick: 'menu' },
		desktopEnvironment: 'gnome',
		source: 'portal',
		...overrides,
	};
}

describe('titlebarConfigFor without a reading', () => {
	it('uses caption buttons and a left-aligned title on Windows', () => {
		const config = titlebarConfigFor(null, 'windows');
		expect(config.controlsStyle).toBe('win11');
		expect(config.titleAlign).toBe('start');
		expect(config.buttonLayout.end).toEqual(['minimise', 'maximise', 'close']);
	});

	it('uses the GNOME style and a centred title elsewhere', () => {
		for (const platform of ['linux', undefined, 'macos']) {
			const config = titlebarConfigFor(null, platform);
			expect(config.controlsStyle).toBe('gnome');
			expect(config.titleAlign).toBe('center');
		}
	});
});

describe('titlebarConfigFor with the OS preferences', () => {
	it('takes the layout and actions from the user settings', () => {
		const config = titlebarConfigFor(
			preferences({
				buttonLayout: { start: ['close'], end: ['minimise', 'maximise'] },
				actions: { doubleClick: 'minimise', middleClick: 'lower', rightClick: 'none' },
			}),
			'linux',
		);
		expect(config.buttonLayout).toEqual({ start: ['close'], end: ['minimise', 'maximise'] });
		expect(config.titlebarActions).toEqual({
			doubleClick: 'minimise',
			middleClick: 'lower',
			rightClick: 'none',
		});
	});

	it('picks the visual style from the desktop environment', () => {
		const style = (desktopEnvironment: TitlebarPreferences['desktopEnvironment']) =>
			titlebarConfigFor(preferences({ desktopEnvironment }), 'linux').controlsStyle;
		expect(style('gnome')).toBe('gnome');
		expect(style('kde')).toBe('kde');
		expect(style('cinnamon')).toBe('cinnamon');
		expect(style('windows')).toBe('win11');
		expect(style('xfce')).toBe('gnome');
		expect(style('unknown')).toBe('gnome');
	});

	it('left-aligns the title only on Windows', () => {
		expect(
			titlebarConfigFor(preferences({ desktopEnvironment: 'windows' }), 'windows').titleAlign,
		).toBe('start');
		expect(titlebarConfigFor(preferences({ desktopEnvironment: 'kde' }), 'linux').titleAlign).toBe(
			'center',
		);
	});
});
