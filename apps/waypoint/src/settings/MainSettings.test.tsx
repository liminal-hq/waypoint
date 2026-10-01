// Verifies a Main window follows the settings and opens the Settings window on Ctrl+,
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn().mockResolvedValue(undefined);
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));

import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { MainSettings } from './MainSettings';
import { useSettings } from './SettingsContext';

afterEach(() => {
	cleanup();
	invoke.mockClear();
});

function Mode() {
	return <output aria-label="mode">{useSettings((s) => s.general.clickMode)}</output>;
}

describe('MainSettings', () => {
	it('gives the window the settings and follows a change made in the Settings window', async () => {
		const fake = createFakeSettingsClient();
		render(
			<MainSettings client={fake}>
				<Mode />
			</MainSettings>,
		);
		expect(screen.getByLabelText('mode')).toHaveTextContent('double');
		await act(async () => {
			await fake.snapshot();
		});
		act(() => {
			fake.change({
				...DEFAULT_SETTINGS,
				general: { ...DEFAULT_SETTINGS.general, clickMode: 'single' },
			});
		});
		expect(screen.getByLabelText('mode')).toHaveTextContent('single');
	});

	it('opens the Settings window on Ctrl+,', () => {
		render(
			<MainSettings client={createFakeSettingsClient()}>
				<Mode />
			</MainSettings>,
		);
		fireEvent.keyDown(document.body, { key: ',', ctrlKey: true });
		expect(invoke).toHaveBeenCalledWith('open_settings_window');
	});
});
