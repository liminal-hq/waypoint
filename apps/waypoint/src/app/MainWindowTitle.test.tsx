// Verifies the Main window's title bar follows the title the active tab sets, with the app name when asked, and the OS title stays plain
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { useWindowTitle } from '@liminal-hq/waypoint-chrome/WindowTitle/useWindowTitle';
import { cleanup, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { t, tf } from '../i18n/messages';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { SettingsProvider } from '../settings/SettingsContext';

const host = vi.hoisted(() => ({ setTitle: vi.fn() }));
vi.mock('@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls', () => ({
	tauriWindowControls: {
		minimize: () => {},
		toggleMaximize: () => {},
		close: () => {},
		setAlwaysOnTop: () => {},
		isMaximized: async () => false,
		onMaximizedChange: () => () => {},
		setTitle: host.setTitle,
	},
}));
vi.mock('@liminal-hq/plugin-window-manager', () => ({
	getCapabilities: vi.fn().mockResolvedValue(null),
	getAlwaysOnTop: vi.fn().mockResolvedValue(null),
	onAlwaysOnTopChanged: vi.fn().mockResolvedValue(() => {}),
	showSystemWindowMenu: vi.fn(),
}));
vi.mock('../services/titlebarPreferences', () => ({ useTitlebarPreferences: () => null }));

import { MainWindowChrome } from './AppWindowChrome';
import { AppTitleBar } from './AppTitleBar';

afterEach(() => {
	cleanup();
	host.setTitle.mockClear();
});

function ActiveFolder({ name }: { name: string }) {
	useWindowTitle(name);
	return null;
}

function renderMain(appNameInTitle: boolean, folder?: string) {
	const client = createFakeSettingsClient({
		...DEFAULT_SETTINGS,
		ui: { ...DEFAULT_SETTINGS.ui, appNameInTitle },
	});
	return render(
		<SettingsProvider client={client}>
			<MainWindowChrome>
				<AppTitleBar fallbackTitle={t('window.main.title')} />
				{folder && <ActiveFolder name={folder} />}
			</MainWindowChrome>
		</SettingsProvider>,
	);
}

describe('the Main window title bar', () => {
	it('shows the active tab’s folder, and the OS title is the same plain name', async () => {
		renderMain(false, 'Trash');
		expect(await screen.findByText('Trash')).toBeInTheDocument();
		expect(screen.queryByText(t('window.main.title'))).toBeNull();
		expect(host.setTitle).toHaveBeenLastCalledWith('Trash');
	});

	it('prefixes the app name in the title bar only when the setting is on', async () => {
		renderMain(true, 'Trash');
		const shown = tf('window.main.titleWithApp', { title: 'Trash' });
		await screen.findByText(shown);
		expect(shown).toContain('Trash');
		expect(host.setTitle).toHaveBeenLastCalledWith('Trash');
		expect(document.title).toBe('Trash');
	});

	it('shows the Main name until a folder is set', async () => {
		renderMain(false);
		expect(await screen.findByText(t('window.main.title'))).toBeInTheDocument();
	});
});
