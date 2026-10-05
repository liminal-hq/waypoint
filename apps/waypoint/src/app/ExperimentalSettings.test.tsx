// Verifies the Experimental page: one switch per remote protocol, each off by default and independent, with its badge, the build's missing protocols dimmed, and how a link opens the page
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { tauriWindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls';
import { WindowChromeProvider } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import { createFakeDefaultFileManagerClient } from '../services/fakeDefaultFileManagerClient';
import { createFakeIntegrationsClient } from '../services/fakeIntegrationsClient';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { createFakeSettingsClient, type FakeSettings } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS, type Settings } from '../services/settingsClient';
import type { ProtocolSupport } from '../settings/SettingsEditor';
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
const events = vi.hoisted(() => ({
	handlers: new Map<string, (event: { payload: string }) => void>(),
}));
vi.mock('@tauri-apps/api/event', () => ({
	listen: (name: string, handler: (event: { payload: string }) => void) => {
		events.handlers.set(name, handler);
		return Promise.resolve(() => events.handlers.delete(name));
	},
}));
vi.mock('../services/serviceStatuses', () => ({
	collectServiceStatuses: () => Promise.resolve({}),
}));

afterEach(cleanup);

/** Every remote protocol is in the build, and off (D167). */
const BUILD: ProtocolSupport = { schemes: [], off: ['sftp', 'smb', 'dav', 'davs', 's3'] };
/** A build without the S3 provider (its Cargo feature left out). */
const BUILD_WITHOUT_S3: ProtocolSupport = { schemes: [], off: ['sftp', 'smb', 'dav', 'davs'] };

async function open(
	options: {
		initial?: Settings;
		protocols?: () => Promise<ProtocolSupport>;
		viaLink?: boolean;
	} = {},
): Promise<{ settings: FakeSettings }> {
	const settings = createFakeSettingsClient(options.initial);
	render(
		<WindowChromeProvider controls={tauriWindowControls}>
			<SettingsScreen
				client={settings}
				ops={createFakeOpsClient({})}
				dndStatus={() => Promise.resolve({ outbound: { available: true, reason: null } })}
				integrations={createFakeIntegrationsClient()}
				fileManager={createFakeDefaultFileManagerClient()}
				protocolSupport={options.protocols ?? (() => Promise.resolve(BUILD))}
				{...(options.viaLink ? { initialSection: 'experimental' } : {})}
			/>
		</WindowChromeProvider>,
	);
	if (!options.viaLink) {
		await userEvent.click(
			within(await screen.findByRole('navigation', { name: 'Settings sections' })).getByRole(
				'button',
				{ name: 'Experimental' },
			),
		);
	}
	await screen.findByRole('heading', { level: 2, name: 'Experimental' });
	return { settings };
}

const lastExperimental = (settings: FakeSettings) =>
	settings.calls[settings.calls.length - 1]?.experimental;

describe('the Experimental page', () => {
	it('says in two plain sentences what the page is, and lists one switch per remote protocol', async () => {
		await open();
		expect(
			screen.getByText(/built but not yet proven in daily use, so they may misbehave/),
		).toBeInTheDocument();
		const switches = (await screen.findAllByRole('switch')).map((s) =>
			s.getAttribute('aria-checked'),
		);
		expect(switches).toEqual(['false', 'false', 'false', 'false']);
		for (const name of ['SFTP', 'SMB', 'WebDAV', 'S3']) {
			expect(screen.getByRole('switch', { name: `${name} Experimental` })).toBeInTheDocument();
		}
	});

	it('marks every row Experimental, so the badge is part of the switch’s name for a screen reader', async () => {
		await open();
		const group = await screen.findByRole('group', { name: 'Remote protocols' });
		expect(within(group).getAllByText('Experimental')).toHaveLength(4);
	});

	it('is off by default and opens on the page a link asks for', async () => {
		await open({ viaLink: true });
		expect(DEFAULT_SETTINGS.experimental).toEqual({
			sftp: false,
			smb: false,
			webdav: false,
			s3: false,
		});
		expect(await screen.findByRole('switch', { name: 'SFTP Experimental' })).not.toBeChecked();
	});

	it('turns each switch on by itself and nothing else', async () => {
		const { settings } = await open();
		const sftp = await screen.findByRole('switch', { name: 'SFTP Experimental' });
		await waitFor(() => expect(sftp).toBeEnabled());
		await userEvent.click(sftp);
		await waitFor(() =>
			expect(lastExperimental(settings)).toEqual({
				sftp: true,
				smb: false,
				webdav: false,
				s3: false,
			}),
		);
		await userEvent.click(screen.getByRole('switch', { name: 'WebDAV Experimental' }));
		await waitFor(() =>
			expect(lastExperimental(settings)).toEqual({
				sftp: true,
				smb: false,
				webdav: true,
				s3: false,
			}),
		);
		await userEvent.click(screen.getByRole('switch', { name: 'SFTP Experimental' }));
		await waitFor(() => expect(lastExperimental(settings)?.sftp).toBe(false));
		expect(lastExperimental(settings)?.webdav).toBe(true);
	});

	it('shows what Rust says is in force, so a stored switch reads on', async () => {
		await open({
			initial: {
				...DEFAULT_SETTINGS,
				experimental: { ...DEFAULT_SETTINGS.experimental, smb: true },
			},
		});
		expect(await screen.findByRole('switch', { name: 'SMB Experimental' })).toBeChecked();
		expect(screen.getByRole('switch', { name: 'SFTP Experimental' })).not.toBeChecked();
	});

	it('turns S3 on like the other protocols, now that its provider is in the build', async () => {
		const { settings } = await open();
		const s3 = await screen.findByRole('switch', { name: 'S3 Experimental' });
		await waitFor(() => expect(s3).toBeEnabled());
		await userEvent.click(s3);
		await waitFor(() => expect(lastExperimental(settings)?.s3).toBe(true));
		expect(lastExperimental(settings)?.sftp).toBe(false);
	});

	it('dims the switch of a protocol this build does not have, with the reason', async () => {
		await open({ protocols: () => Promise.resolve(BUILD_WITHOUT_S3) });
		const s3 = await screen.findByRole('switch', { name: 'S3 Experimental' });
		await waitFor(() => expect(s3).toBeDisabled());
		expect(screen.getByText(/Not in this build yet/)).toBeInTheDocument();
		expect(screen.getByRole('switch', { name: 'SFTP Experimental' })).toBeEnabled();
	});

	it('leaves every switch usable when what the build has cannot be read', async () => {
		await open({ protocols: () => Promise.reject(new Error('no plugin')) });
		const s3 = await screen.findByRole('switch', { name: 'S3 Experimental' });
		expect(s3).toBeEnabled();
	});

	it('is reached from the section list like the other pages, with the keyboard', async () => {
		await open();
		const nav = screen.getByRole('navigation', { name: 'Settings sections' });
		const names = within(nav)
			.getAllByRole('button')
			.map((b) => b.textContent);
		expect(names[names.length - 1]).toBe('Experimental');
	});

	it('switches an open Settings window to the page the app asks for, such as from the link on a location that is off', async () => {
		await open({ viaLink: false });
		await userEvent.click(
			within(screen.getByRole('navigation', { name: 'Settings sections' })).getByRole('button', {
				name: 'General',
			}),
		);
		await screen.findByRole('heading', { level: 2, name: 'General' });
		await waitFor(() => expect(events.handlers.has('waypoint://settings-section')).toBe(true));
		act(() => events.handlers.get('waypoint://settings-section')?.({ payload: 'experimental' }));
		expect(await screen.findByRole('heading', { level: 2, name: 'Experimental' })).toBeVisible();
	});
});
