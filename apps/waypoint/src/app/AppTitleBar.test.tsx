// Tests for the title bar with the application menu: where the menu sits against the window buttons on either side
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { TitlebarPreferences } from '@liminal-hq/plugin-system-appearance';
import { WindowChromeProvider } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import type { WindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/windowControls';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandBridgeProvider, createCommandBridge } from '../commands/commandBridge';
import { t } from '../i18n/messages';
import { AppMenu } from './AppMenu';
import { AppTitleBar } from './AppTitleBar';

let preferences: TitlebarPreferences | null = null;
vi.mock('../services/titlebarPreferences', () => ({ useTitlebarPreferences: () => preferences }));

afterEach(cleanup);

const controls: WindowControls = {
	minimize: vi.fn(),
	toggleMaximize: vi.fn(),
	close: vi.fn(),
	setAlwaysOnTop: vi.fn(),
	isMaximized: async () => false,
	onMaximizedChange: () => () => {},
};

function layout(
	start: string[],
	end: string[],
	desktop: TitlebarPreferences['desktopEnvironment'],
) {
	return {
		buttonLayout: { start, end },
		actions: { doubleClick: 'toggleMaximise', middleClick: 'none', rightClick: 'menu' },
		desktopEnvironment: desktop,
		source: 'portal',
	} as TitlebarPreferences;
}

function renderBar() {
	return render(
		<WindowChromeProvider controls={controls}>
			<CommandBridgeProvider value={createCommandBridge()}>
				<AppTitleBar title="Waypoint — Main" start={<AppMenu />} />
			</CommandBridgeProvider>
		</WindowChromeProvider>,
	);
}

const group = (name: string) => document.querySelector(`[data-group="${name}"]`) as HTMLElement;
const menuButton = () => screen.getByRole('button', { name: t('app.name') });

describe('the title bar with the application menu', () => {
	it('puts the menu in the start group with the window buttons on the right', () => {
		preferences = layout([], ['minimise', 'maximise', 'close'], 'gnome');
		renderBar();
		expect(group('start')).toContainElement(menuButton());
		expect(group('end').querySelectorAll('[data-wp-controls]')).toHaveLength(1);
		expect(group('start').querySelector('[data-wp-controls]')).toBeNull();
		// The menu never lands among the window buttons.
		expect(menuButton().closest('[data-wp-controls]')).toBeNull();
	});

	it('puts the menu after the window buttons when the desktop keeps them on the left', () => {
		preferences = layout(['close', 'minimise'], [], 'gnome');
		renderBar();
		const start = group('start');
		expect(start).toContainElement(menuButton());
		const children = [...start.children];
		const controlsAt = children.findIndex((child) => child.matches('[data-wp-controls]'));
		const menuAt = children.findIndex((child) => child.contains(menuButton()));
		expect(controlsAt).toBeGreaterThanOrEqual(0);
		expect(menuAt).toBeGreaterThan(controlsAt);
	});

	it('keeps the menu out of the end group on Windows, where the caption buttons are on the right', () => {
		preferences = layout([], ['minimise', 'maximise', 'close'], 'windows');
		renderBar();
		expect(group('end')).not.toContainElement(menuButton());
		expect(document.querySelector('[data-title-align="start"]')).not.toBeNull();
	});

	it('works with the platform default before the OS preferences have been read', () => {
		preferences = null;
		renderBar();
		expect(group('start')).toContainElement(menuButton());
	});

	it('is not a drag region, so a press on it opens the menu instead of moving the window', () => {
		preferences = null;
		renderBar();
		expect(menuButton()).not.toHaveAttribute('data-tauri-drag-region');
		expect(menuButton()).toHaveAttribute('data-window-menu-exclude');
	});
});
