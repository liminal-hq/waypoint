// Verifies the Integrations page: every switch off by default, the Services reason where one cannot work, the shortcut field and the default file manager action
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { tauriWindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls';
import { WindowChromeProvider } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import { createFakeDefaultFileManagerClient } from '../services/fakeDefaultFileManagerClient';
import {
	createFakeIntegrationsClient,
	everythingWorks,
	without,
} from '../services/fakeIntegrationsClient';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { createFakeSettingsClient, type FakeSettings } from '../services/fakeSettingsClient';
import type { DefaultFileManagerClient } from '../services/defaultFileManagerClient';
import type { IntegrationsClient } from '../services/integrationsClient';
import { SettingsScreen } from './SettingsScreen';

vi.mock('@tauri-apps/api/window', () => ({
	getCurrentWindow: () => ({
		minimize: vi.fn(),
		toggleMaximize: vi.fn(),
		close: vi.fn(),
		startDragging: vi.fn(),
		setAlwaysOnTop: vi.fn(),
		isAlwaysOnTop: vi.fn().mockResolvedValue(false),
		isMaximized: vi.fn().mockResolvedValue(false),
		onResized: vi.fn().mockResolvedValue(() => {}),
		isFocused: vi.fn().mockResolvedValue(true),
		onFocusChanged: vi.fn().mockResolvedValue(() => {}),
	}),
}));
// The Services panel asks every plugin; it is not what these tests are about.
vi.mock('../services/serviceStatuses', () => ({
	collectServiceStatuses: () => Promise.resolve({}),
}));

afterEach(cleanup);

async function open(
	options: {
		integrations?: IntegrationsClient;
		fileManager?: DefaultFileManagerClient;
	} = {},
): Promise<{ settings: FakeSettings }> {
	const settings = createFakeSettingsClient();
	render(
		<WindowChromeProvider controls={tauriWindowControls}>
			<SettingsScreen
				client={settings}
				ops={createFakeOpsClient({})}
				dndStatus={() => Promise.resolve({ outbound: { available: true, reason: null } })}
				integrations={options.integrations ?? createFakeIntegrationsClient()}
				fileManager={options.fileManager ?? createFakeDefaultFileManagerClient()}
			/>
		</WindowChromeProvider>,
	);
	await userEvent.click(
		within(await screen.findByRole('navigation', { name: 'Settings sections' })).getByRole(
			'button',
			{ name: 'Integrations' },
		),
	);
	await screen.findByRole('heading', { level: 2, name: 'Integrations' });
	return { settings };
}

const NOTIFY = 'Notify when a job finishes';
const PROGRESS = 'Show progress on the app icon';
const SLEEP = 'Keep the computer awake during jobs';
const SERVICE = 'Open folders other applications ask for';
const SHORTCUT_ON = 'Bring Waypoint forward with a shortcut';

const lastIntegrations = (settings: FakeSettings) =>
	settings.calls[settings.calls.length - 1]?.integrations;

describe('the Integrations page', () => {
	it('shows every integration off, as D118 asks, and enabled to switch where the system can do it', async () => {
		await open();
		for (const name of [NOTIFY, PROGRESS, SLEEP, SERVICE, SHORTCUT_ON]) {
			const row = await screen.findByRole('switch', { name });
			expect(row).not.toBeChecked();
			await waitFor(() => expect(row).toBeEnabled());
		}
	});

	it('saves a switch through Rust and shows what is in force', async () => {
		const { settings } = await open();
		await userEvent.click(await screen.findByRole('switch', { name: NOTIFY }));
		await waitFor(() => expect(screen.getByRole('switch', { name: NOTIFY })).toBeChecked());
		expect(lastIntegrations(settings)).toMatchObject({ notifications: true, preventSleep: false });
		await userEvent.click(screen.getByRole('switch', { name: SLEEP }));
		await userEvent.click(screen.getByRole('switch', { name: PROGRESS }));
		await userEvent.click(screen.getByRole('switch', { name: SERVICE }));
		await waitFor(() =>
			expect(lastIntegrations(settings)).toEqual({
				notifications: true,
				preventSleep: true,
				launcherProgress: true,
				defaultFileManager: true,
				globalShortcutEnabled: false,
				globalShortcut: null,
			}),
		);
	});

	it('dims a switch the system cannot do and gives the Services reason, and will not change it', async () => {
		const integrations = createFakeIntegrationsClient(
			without({
				notifications: 'No notification server is running.',
				preventSleep: 'systemd-logind is not running.',
			}),
		);
		const { settings } = await open({ integrations });
		const notify = await screen.findByRole('switch', { name: NOTIFY });
		await waitFor(() => expect(notify).toBeDisabled());
		expect(notify).toHaveAccessibleDescription(/No notification server is running\./);
		expect(screen.getByRole('switch', { name: SLEEP })).toBeDisabled();
		expect(screen.getByRole('switch', { name: SLEEP })).toHaveAccessibleDescription(
			/systemd-logind is not running\./,
		);
		// The others still work.
		expect(screen.getByRole('switch', { name: PROGRESS })).toBeEnabled();
		await userEvent.click(notify);
		expect(settings.calls).toHaveLength(0);
	});

	it('leaves every switch off and says so when what the system can do cannot be read', async () => {
		const integrations = createFakeIntegrationsClient(everythingWorks(), {}, 'no command');
		await open({ integrations });
		const notify = await screen.findByRole('switch', { name: NOTIFY });
		await waitFor(() =>
			expect(notify).toHaveAccessibleDescription(/could not check what this system can do/),
		);
		expect(notify).toBeDisabled();
		expect(screen.getByRole('switch', { name: SHORTCUT_ON })).toBeDisabled();
	});

	it('enables the shortcut field only with the shortcut, showing the default, and saves a valid one', async () => {
		const { settings } = await open();
		const field = await screen.findByRole('textbox', { name: 'Shortcut' });
		expect(field).toHaveValue('Ctrl+Alt+W');
		expect(field).toBeDisabled();
		await userEvent.click(screen.getByRole('switch', { name: SHORTCUT_ON }));
		await waitFor(() => expect(field).toBeEnabled());
		await userEvent.clear(field);
		await userEvent.type(field, 'Ctrl+Shift+K{Enter}');
		await waitFor(() =>
			expect(lastIntegrations(settings)).toMatchObject({
				globalShortcutEnabled: true,
				globalShortcut: 'Ctrl+Shift+K',
			}),
		);
	});

	it("shows Rust's refusal under the field and keeps the value in force", async () => {
		const { settings } = await open();
		const field = await screen.findByRole('textbox', { name: 'Shortcut' });
		await userEvent.click(screen.getByRole('switch', { name: SHORTCUT_ON }));
		await waitFor(() => expect(field).toBeEnabled());
		await userEvent.clear(field);
		await userEvent.type(field, 'W{Enter}');
		expect(await screen.findByRole('alert')).toHaveTextContent(/write it like Ctrl\+Alt\+W/);
		expect(field).toHaveAttribute('aria-invalid', 'true');
		expect(settings.current().settings.integrations.globalShortcut).toBeNull();
		expect(field).toHaveValue('Ctrl+Alt+W');
	});

	it('goes back to the default accelerator when the field is emptied', async () => {
		const { settings } = await open();
		const field = await screen.findByRole('textbox', { name: 'Shortcut' });
		await userEvent.click(screen.getByRole('switch', { name: SHORTCUT_ON }));
		await waitFor(() => expect(field).toBeEnabled());
		await userEvent.clear(field);
		await userEvent.type(field, 'Alt+Q{Enter}');
		await waitFor(() => expect(lastIntegrations(settings)?.globalShortcut).toBe('Alt+Q'));
		await userEvent.clear(field);
		await userEvent.type(field, '{Enter}');
		await waitFor(() => expect(lastIntegrations(settings)?.globalShortcut).toBeNull());
		await waitFor(() => expect(field).toHaveValue('Ctrl+Alt+W'));
	});
});

describe('the default file manager row', () => {
	it('makes Waypoint the default and then shows that it is', async () => {
		const fileManager = createFakeDefaultFileManagerClient({ kind: 'set' });
		await open({ fileManager });
		expect(await screen.findByText('The default file manager is Files.')).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Make default' }));
		expect(fileManager.made).toBe(1);
		expect(await screen.findByText('Waypoint is the default file manager.')).toBeInTheDocument();
		expect(screen.queryByText('The default file manager is Files.')).toBeNull();
	});

	it('says why when the system refused, such as a development run with no desktop file', async () => {
		const fileManager = createFakeDefaultFileManagerClient(
			{ kind: 'set' },
			{ isWaypoint: false, name: null },
			'Waypoint is not installed as an application on this system.',
		);
		await open({ fileManager });
		await userEvent.click(await screen.findByRole('button', { name: 'Make default' }));
		expect(
			await screen.findByText(
				'Could not make Waypoint the default: Waypoint is not installed as an application on this system.',
			),
		).toBeInTheDocument();
	});

	it('opens the system’s Default apps page with instructions on Windows, and offers no name to own', async () => {
		const fileManager = createFakeDefaultFileManagerClient({ kind: 'settings' }, null);
		await open({ fileManager });
		expect(await screen.findByText('Choose Waypoint in your system settings')).toBeInTheDocument();
		expect(screen.getByText(/Windows lets only you change the default/)).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Open Default apps' }));
		expect(fileManager.made).toBe(1);
		expect(screen.queryByRole('switch', { name: SERVICE })).toBeNull();
	});

	it('is dimmed with the reason where the system does not offer it', async () => {
		const fileManager = createFakeDefaultFileManagerClient(
			{ kind: 'unavailable', reason: 'A Flatpak sandbox cannot change the default.' },
			null,
		);
		await open({ fileManager });
		const button = await screen.findByRole('button', { name: 'Make default' });
		expect(button).toBeDisabled();
		expect(button).toHaveAccessibleDescription(/A Flatpak sandbox cannot change the default\./);
	});

	it('saves the switch that makes Waypoint answer other applications’ folder requests', async () => {
		const { settings } = await open();
		await userEvent.click(await screen.findByRole('switch', { name: SERVICE }));
		await waitFor(() => expect(lastIntegrations(settings)?.defaultFileManager).toBe(true));
	});
});
