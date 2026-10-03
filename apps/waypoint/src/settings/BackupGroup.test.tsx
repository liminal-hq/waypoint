// Verifies Settings → General → Back up and restore: export, the import plan and its confirmation, and each refusal in plain words
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { tauriWindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls';
import { WindowChromeProvider } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import { SettingsScreen } from '../app/SettingsScreen';
import { dismissNotice } from '../app/notices';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import {
	createFakeSettingsTransferClient,
	type FakeSettingsTransfer,
} from '../services/fakeSettingsTransferClient';
import { DEFAULT_SETTINGS, type SettingsCommandError } from '../services/settingsClient';
import type { ImportPreview } from '../services/settingsTransferClient';

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

afterEach(() => {
	cleanup();
	act(() => dismissNotice());
});

const AVAILABLE = { outbound: { available: true, reason: null } };

function preview(overrides: Partial<ImportPreview['plan']> = {}): ImportPreview {
	return {
		planId: 7,
		plan: {
			kind: 'zip',
			appVersion: '0.1.0',
			exportedAt: '2026-10-03T02:30:00Z',
			files: ['settings', 'ops'],
			changes: [
				{ file: 'settings', group: 'general', count: 2 },
				{ file: 'settings', group: 'dnd', count: 1 },
				{ file: 'ops', group: 'ops', count: 3 },
			],
			warnings: [],
			...overrides,
		},
	};
}

async function open(): Promise<{ transfer: FakeSettingsTransfer; opsReads: () => number }> {
	const transfer = createFakeSettingsTransferClient();
	const ops = createFakeOpsClient({});
	let reads = 0;
	render(
		<WindowChromeProvider controls={tauriWindowControls}>
			<SettingsScreen
				client={createFakeSettingsClient(DEFAULT_SETTINGS)}
				ops={{
					getSettings: () => {
						reads += 1;
						return ops.getSettings();
					},
					setSettings: (next) => ops.setSettings(next),
				}}
				dndStatus={() => Promise.resolve(AVAILABLE)}
				transfer={transfer}
			/>
		</WindowChromeProvider>,
	);
	await screen.findByRole('button', { name: 'Export settings…' });
	return { transfer, opsReads: () => reads };
}

const exportButton = () => screen.getByRole('button', { name: 'Export settings…' });
const importButton = () => screen.getByRole('button', { name: 'Import settings…' });
const refusal = (kind: SettingsCommandError['kind'], extra: Partial<SettingsCommandError> = {}) =>
	({ kind, message: 'a reason in English', ...extra }) as SettingsCommandError;

describe('Back up and restore', () => {
	it('is a group on the General page with Export and Import buttons', async () => {
		await open();
		expect(screen.getByRole('heading', { name: 'Back up and restore' })).toBeInTheDocument();
		expect(exportButton()).toBeEnabled();
		expect(importButton()).toBeEnabled();
	});

	it('exports, then confirms with the path in a status message', async () => {
		const { transfer } = await open();
		transfer.nextExport({
			path: '/home/me/waypoint-settings-2026-10-03.zip',
			kind: 'zip',
			files: [],
		});
		await userEvent.click(exportButton());
		expect(
			await screen.findByText(/Settings exported to \/home\/me\/waypoint-settings-2026-10-03\.zip/),
		).toBeInTheDocument();
		expect(screen.getByRole('status')).toHaveTextContent('waypoint-settings-2026-10-03.zip');
		expect(transfer.calls.export).toBe(1);
		expect(screen.queryByRole('alert')).toBeNull();
	});

	it('says nothing when the save dialog is closed', async () => {
		const { transfer } = await open();
		transfer.nextExport(null);
		await userEvent.click(exportButton());
		await waitFor(() => expect(transfer.calls.export).toBe(1));
		expect(screen.getByRole('status')).toBeEmptyDOMElement();
		expect(screen.queryByRole('alert')).toBeNull();
	});

	it('shows what a failed export could not do under its row, and clears it on the next try', async () => {
		const { transfer } = await open();
		transfer.nextExport(refusal('io', { message: 'could not write the file: disk full' }));
		await userEvent.click(exportButton());
		const alert = await screen.findByRole('alert');
		expect(alert).toHaveTextContent(
			'The file could not be read or written: could not write the file: disk full',
		);
		transfer.nextExport(null);
		await userEvent.click(exportButton());
		await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
	});

	it('shows the plan before anything changes, with Cancel as the default', async () => {
		const { transfer } = await open();
		transfer.nextPlan(
			preview({
				warnings: [
					{ kind: 'unknownKeys', file: 'settings', keys: ['general.newThing', 'future'] },
					{ kind: 'unknownFiles', ids: ['later'] },
				],
			}),
		);
		await userEvent.click(importButton());
		const dialog = await screen.findByRole('dialog', { name: 'Replace your settings?' });
		expect(dialog).toHaveAccessibleDescription(
			'Your current settings will be replaced by the ones in this file.',
		);
		const list = within(dialog)
			.getAllByRole('listitem')
			.map((item) => item.textContent);
		expect(list).toEqual([
			'General2 changes',
			'Drag & drop1 change',
			'Operations3 changes',
			'2 settings in the file are not in this version of Waypoint, so they will be left out.',
			'1 part of the file is not in this version of Waypoint, so it will be left out.',
		]);
		expect(within(dialog).getByText('The file was made by Waypoint 0.1.0.')).toBeInTheDocument();
		// Opening the plan applied nothing, and focus starts on the safe choice.
		expect(transfer.calls.apply).toEqual([]);
		await waitFor(() =>
			expect(within(dialog).getByRole('button', { name: 'Cancel' })).toHaveFocus(),
		);
	});

	it('Cancel and Escape leave the settings as they are', async () => {
		const { transfer } = await open();
		transfer.nextPlan(preview());
		await userEvent.click(importButton());
		await screen.findByRole('dialog');
		await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());

		transfer.nextPlan(preview());
		await userEvent.click(importButton());
		await screen.findByRole('dialog');
		await userEvent.keyboard('{Escape}');
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(transfer.calls.apply).toEqual([]);
		expect(screen.queryByText('Settings imported.')).toBeNull();
	});

	it('replaces on confirmation by the plan number alone, then confirms and reads the operations settings again', async () => {
		const { transfer, opsReads } = await open();
		const before = opsReads();
		transfer.nextPlan(preview());
		transfer.nextApply({ revision: 5, settings: DEFAULT_SETTINGS });
		await userEvent.click(importButton());
		await screen.findByRole('dialog');
		await userEvent.click(screen.getByRole('button', { name: 'Replace my settings' }));
		expect(await screen.findByText('Settings imported.')).toBeInTheDocument();
		expect(transfer.calls.apply).toEqual([7]);
		expect(opsReads()).toBeGreaterThan(before);
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(screen.queryByRole('alert')).toBeNull();
	});

	it('is operable from the keyboard alone', async () => {
		const { transfer } = await open();
		transfer.nextPlan(preview());
		transfer.nextApply({ revision: 5, settings: DEFAULT_SETTINGS });
		importButton().focus();
		await userEvent.keyboard('{Enter}');
		await screen.findByRole('dialog');
		await userEvent.keyboard('{Tab}');
		expect(screen.getByRole('button', { name: 'Replace my settings' })).toHaveFocus();
		await userEvent.keyboard('{Enter}');
		expect(await screen.findByText('Settings imported.')).toBeInTheDocument();
	});

	it('does not ask when the file matches the settings in force', async () => {
		const { transfer } = await open();
		transfer.nextPlan(preview({ changes: [] }));
		await userEvent.click(importButton());
		expect(
			await screen.findByText('That file matches your current settings, so nothing was changed.'),
		).toBeInTheDocument();
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(transfer.calls.apply).toEqual([]);
	});

	it('does nothing when the open dialog is closed', async () => {
		const { transfer } = await open();
		transfer.nextPlan(null);
		await userEvent.click(importButton());
		await waitFor(() => expect(transfer.calls.plan).toBe(1));
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(screen.queryByRole('alert')).toBeNull();
	});

	it.each([
		['not-a-bundle', 'That file is not a Waypoint settings file.'],
		['corrupt', 'That file is damaged or incomplete, so it cannot be read.'],
		[
			'newer-format',
			'That file was made by a newer version of Waypoint. Update Waypoint to import it.',
		],
		['too-large', 'That file is too large to be a settings file.'],
		['unsafe', 'That archive holds files Waypoint will not read, so nothing was imported.'],
	])('says plainly, under the row, that a file is refused as %s', async (reason, sentence) => {
		const { transfer } = await open();
		transfer.nextPlan(refusal('transfer', { reason }));
		await userEvent.click(importButton());
		expect(await screen.findByRole('alert')).toHaveTextContent(sentence);
		expect(screen.queryByRole('dialog')).toBeNull();
		// The alert belongs to the Import row, tied to its button.
		const alert = screen.getByRole('alert');
		expect(importButton().getAttribute('aria-describedby') ?? '').toContain(alert.id);
	});

	it('names the setting a file holds that is not allowed', async () => {
		const { transfer } = await open();
		transfer.nextPlan(
			refusal('transfer', {
				reason: 'invalid',
				message: 'dnd.springLoadMs must be between 200 and 2000',
			}),
		);
		await userEvent.click(importButton());
		expect(await screen.findByRole('alert')).toHaveTextContent(
			'That file holds a setting that is not allowed: dnd.springLoadMs must be between 200 and 2000',
		);
	});

	it('says an import that failed left the settings as they were, and one that is no longer ready', async () => {
		const { transfer } = await open();
		transfer.nextPlan(preview());
		transfer.nextApply(refusal('apply', { message: 'could not apply `ops`: no room' }));
		await userEvent.click(importButton());
		await userEvent.click(await screen.findByRole('button', { name: 'Replace my settings' }));
		expect(await screen.findByRole('alert')).toHaveTextContent(
			'The import failed, so your settings were left as they were: could not apply `ops`: no room',
		);

		transfer.nextPlan(preview());
		await userEvent.click(importButton());
		await userEvent.click(await screen.findByRole('button', { name: 'Replace my settings' }));
		expect(await screen.findByRole('alert')).toHaveTextContent(
			'That import is no longer ready. Choose the file again.',
		);
	});

	it('says when the system has no file dialog', async () => {
		const { transfer } = await open();
		transfer.nextExport(refusal('unavailable'));
		await userEvent.click(exportButton());
		expect(await screen.findByRole('alert')).toHaveTextContent(
			'This system has no file dialog Waypoint can use.',
		);
	});
});
