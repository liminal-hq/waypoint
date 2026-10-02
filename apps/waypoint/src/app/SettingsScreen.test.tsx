// Verifies the Settings window: its pages and rows, saving through Rust's owners, refusals, unavailable states and live updates
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { tauriWindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls';
import { WindowChromeProvider } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { createFakeSettingsClient, type FakeSettings } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS, type Settings } from '../services/settingsClient';
import type { PluginStatus } from '@liminal-hq/plugin-thumbnails';
import { brokenStatus, workingStatus } from '../thumbnails/fakeThumbnailsClient';
import type { DndAvailability, OpsSettingsApi } from '../settings/SettingsEditor';
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

afterEach(cleanup);

const AVAILABLE: DndAvailability = { outbound: { available: true, reason: null } };

interface Rig {
	settings: FakeSettings;
	ops: FakeOpsClient;
	opsApi: OpsSettingsApi;
	opsSaves: unknown[];
}

async function open(
	options: {
		settings?: Settings;
		dnd?: DndAvailability | Error;
		thumbnails?: PluginStatus | Error;
		opsApi?: (ops: FakeOpsClient) => OpsSettingsApi;
	} = {},
): Promise<Rig> {
	const settings = createFakeSettingsClient(options.settings);
	const ops = createFakeOpsClient({});
	const opsSaves: unknown[] = [];
	const opsApi = options.opsApi?.(ops) ?? {
		getSettings: () => ops.getSettings(),
		setSettings: (next) => {
			opsSaves.push(next);
			return ops.setSettings(next);
		},
	};
	const dnd = options.dnd ?? AVAILABLE;
	const thumbnails = options.thumbnails ?? workingStatus();
	render(
		<WindowChromeProvider controls={tauriWindowControls}>
			<SettingsScreen
				client={settings}
				ops={opsApi}
				dndStatus={() => (dnd instanceof Error ? Promise.reject(dnd) : Promise.resolve(dnd))}
				thumbnailsStatus={() =>
					thumbnails instanceof Error ? Promise.reject(thumbnails) : Promise.resolve(thumbnails)
				}
			/>
		</WindowChromeProvider>,
	);
	await screen.findByRole('navigation', { name: 'Settings sections' });
	return { settings, ops, opsApi, opsSaves };
}

const goTo = async (name: string) => {
	await userEvent.click(within(screen.getByRole('navigation')).getByRole('button', { name }));
	await screen.findByRole('heading', { level: 2, name });
};

describe('SettingsScreen', () => {
	it('lists only the pages that exist, with the title bar, and no placeholder pages', async () => {
		await open();
		const nav = within(screen.getByRole('navigation', { name: 'Settings sections' }));
		expect(nav.getAllByRole('button').map((b) => b.textContent)).toEqual([
			'General',
			'Appearance',
			'Accessibility',
			'Previews & thumbnails',
			'Operations',
			'Drag & drop',
			'Integrations',
		]);
		expect(screen.getAllByText('Waypoint — Settings').length).toBeGreaterThan(0);
		expect(screen.queryByText(/coming soon/i)).toBeNull();
		expect(screen.getByRole('heading', { level: 2, name: 'General' })).toBeInTheDocument();
		expect(screen.queryByRole('button', { name: /save/i })).toBeNull();
	});

	it('says it is loading until both documents are read', async () => {
		const settings = createFakeSettingsClient();
		const gate = settings.holdSnapshot();
		render(
			<WindowChromeProvider controls={tauriWindowControls}>
				<SettingsScreen
					client={settings}
					ops={createFakeOpsClient({})}
					dndStatus={() => Promise.resolve(AVAILABLE)}
				/>
			</WindowChromeProvider>,
		);
		expect(await screen.findByText('Loading settings…')).toBeInTheDocument();
		expect(screen.queryByRole('navigation')).toBeNull();
		await act(async () => gate.release());
		expect(await screen.findByRole('navigation')).toBeInTheDocument();
		expect(screen.queryByText('Loading settings…')).toBeNull();
	});

	it('moves between pages from the keyboard, one arrow and Enter at a time', async () => {
		await open();
		const general = screen.getByRole('button', { name: 'General' });
		general.focus();
		await userEvent.keyboard('{ArrowDown}');
		expect(screen.getByRole('button', { name: 'Appearance' })).toHaveFocus();
		expect(screen.getByRole('heading', { level: 2, name: 'General' })).toBeInTheDocument();
		await userEvent.keyboard('{Enter}');
		expect(
			await screen.findByRole('heading', { level: 2, name: 'Appearance' }),
		).toBeInTheDocument();
	});
});

describe('the General page', () => {
	it('shows every row with its description, named by its label', async () => {
		await open();
		for (const name of ['When Waypoint starts', 'Default view', 'Open items with']) {
			expect(screen.getByRole('radiogroup', { name })).toBeInTheDocument();
		}
		expect(screen.getByRole('switch', { name: 'Show hidden files by default' })).not.toBeChecked();
		expect(
			screen.getByRole('switch', { name: 'Confirm before moving to the Trash' }),
		).not.toBeChecked();
		expect(screen.getByRole('radio', { name: 'Restore session' })).toBeChecked();
		expect(screen.getByRole('radio', { name: 'List' })).toBeChecked();
		expect(screen.getByRole('radio', { name: 'Double click' })).toBeChecked();
		expect(
			screen.getByRole('switch', { name: 'Show hidden files by default' }),
		).toHaveAccessibleDescription(/New windows|A new window lists hidden files/);
		expect(
			screen.getByRole('radiogroup', { name: 'When Waypoint starts' }),
		).toHaveAccessibleDescription(/Takes effect the next time Waypoint starts/);
	});

	it('saves a segmented choice through the settings plugin and keeps the rest of the document', async () => {
		const { settings } = await open();
		await userEvent.click(screen.getByRole('radio', { name: 'Grid' }));
		await waitFor(() => expect(screen.getByRole('radio', { name: 'Grid' })).toBeChecked());
		expect(settings.calls).toHaveLength(1);
		expect(settings.current().settings).toEqual({
			...DEFAULT_SETTINGS,
			general: { ...DEFAULT_SETTINGS.general, defaultView: 'grid' },
		});
		await userEvent.click(screen.getByRole('radio', { name: 'Single click' }));
		await userEvent.click(screen.getByRole('radio', { name: 'Open Home' }));
		await userEvent.click(screen.getByRole('switch', { name: 'Show hidden files by default' }));
		await waitFor(() => expect(settings.current().revision).toBe(4));
		expect(settings.current().settings.general).toEqual({
			defaultView: 'grid',
			clickMode: 'single',
			startup: 'home',
			showHiddenDefault: true,
		});
	});

	it('shows the menu bar row under Title bar, off by default, and saves it', async () => {
		const { settings } = await open();
		const row = screen.getByRole('switch', { name: 'Show a menu bar under the title bar' });
		expect(row).not.toBeChecked();
		expect(row).toHaveAccessibleDescription(/File, Edit, View and Window/);
		await userEvent.click(row);
		await waitFor(() => expect(row).toBeChecked());
		expect(settings.current().settings.ui.menuBar).toBe(true);
		expect(settings.current().settings).toEqual({
			...DEFAULT_SETTINGS,
			ui: { ...DEFAULT_SETTINGS.ui, menuBar: true },
		});
	});

	it('edits the confirm-before-Trash row through the operations plugin, not the settings plugin', async () => {
		const { settings, ops, opsSaves } = await open();
		await userEvent.click(
			screen.getByRole('switch', { name: 'Confirm before moving to the Trash' }),
		);
		await waitFor(() =>
			expect(
				screen.getByRole('switch', { name: 'Confirm before moving to the Trash' }),
			).toBeChecked(),
		);
		expect(opsSaves).toHaveLength(1);
		expect((await ops.getSettings()).confirmTrash).toBe(true);
		// The rest of the operations settings go back unchanged.
		expect((await ops.getSettings()).concurrency).toBe(2);
		expect(settings.calls).toHaveLength(0);
	});

	it('shows a refusal under the row and keeps the value that is in force', async () => {
		const { settings } = await open();
		settings.failNext({
			kind: 'storage',
			message: 'could not save the settings: the disk is full',
		});
		await userEvent.click(screen.getByRole('radio', { name: 'Grid' }));
		const alert = await screen.findByRole('alert');
		expect(alert).toHaveTextContent('Could not save the settings: the disk is full');
		expect(screen.getByRole('radio', { name: 'List' })).toBeChecked();
		expect(screen.getByRole('radio', { name: 'Grid' })).not.toBeChecked();
		// The next change clears it.
		await userEvent.click(screen.getByRole('radio', { name: 'Grid' }));
		await waitFor(() => expect(screen.queryByRole('alert')).toBeNull());
	});

	it('follows a change another window makes while it is open', async () => {
		const { settings } = await open();
		act(() => {
			settings.change({
				...DEFAULT_SETTINGS,
				general: { ...DEFAULT_SETTINGS.general, clickMode: 'single' },
			});
		});
		expect(screen.getByRole('radio', { name: 'Single click' })).toBeChecked();
	});

	it('disables the Trash row and explains when the operations settings cannot be read', async () => {
		await open({
			opsApi: () => ({
				getSettings: () => Promise.reject({ kind: 'storage', message: 'no ops' }),
				setSettings: () => Promise.reject(new Error('unreachable')),
			}),
		});
		const row = screen.getByRole('switch', { name: 'Confirm before moving to the Trash' });
		expect(row).toBeDisabled();
		expect(row).toHaveAccessibleDescription(/could not be read.*no ops/);
	});
});

describe('the Operations page', () => {
	it('shows each row and keeps the checksum choice off until verification is on', async () => {
		const { opsSaves } = await open();
		await goTo('Operations');
		expect(screen.getByRole('switch', { name: 'Verify copies after writing' })).not.toBeChecked();
		const algorithm = screen.getByRole('combobox', { name: 'Checksum algorithm' });
		expect(algorithm).toBeDisabled();
		expect(algorithm).toHaveAccessibleDescription(/Turn on “Verify copies after writing”/);
		expect(screen.queryByText(/^Unavailable/)).toBeNull();
		await userEvent.click(screen.getByRole('switch', { name: 'Verify copies after writing' }));
		await waitFor(() => expect(algorithm).toBeEnabled());
		expect(algorithm).not.toHaveAccessibleDescription(/Turn on/);
		await userEvent.selectOptions(algorithm, 'sha256');
		await waitFor(() => expect(opsSaves).toHaveLength(2));
		expect(opsSaves[1]).toMatchObject({ verifyAfterCopy: true, verifyAlgorithm: 'sha256' });
	});

	it('takes a number only when it is finished, clamped to its range', async () => {
		const { opsSaves } = await open();
		await goTo('Operations');
		const depth = screen.getByRole('spinbutton', { name: 'Undo history depth' });
		expect(depth).toHaveValue(50);
		fireEvent.change(depth, { target: { value: '5' } });
		fireEvent.change(depth, { target: { value: '90' } });
		expect(opsSaves).toHaveLength(0);
		await userEvent.type(depth, '{Enter}');
		await waitFor(() => expect(opsSaves).toHaveLength(1));
		expect(opsSaves[0]).toMatchObject({ undoDepth: 90 });

		const running = screen.getByRole('spinbutton', { name: 'Operations running at once' });
		expect(running).toHaveAttribute('min', '1');
		expect(running).toHaveAttribute('max', '8');
		fireEvent.change(running, { target: { value: '40' } });
		fireEvent.blur(running);
		await waitFor(() => expect(opsSaves).toHaveLength(2));
		expect(opsSaves[1]).toMatchObject({ concurrency: 8 });
	});

	it('turns the Trash sweep on with a default and off again, and only then offers the days', async () => {
		const { opsSaves, ops } = await open();
		await goTo('Operations');
		const days = screen.getByRole('spinbutton', { name: 'Delete Trash items older than' });
		const sweep = screen.getByRole('switch', { name: 'Empty old items from the Trash' });
		expect(sweep).not.toBeChecked();
		expect(days).toBeDisabled();
		expect(sweep).toHaveAccessibleDescription(/runs when Waypoint starts/);
		await userEvent.click(sweep);
		await waitFor(() => expect(days).toBeEnabled());
		expect(days).toHaveValue(30);
		expect(await ops.getSettings()).toMatchObject({ trashExpiryDays: 30 });
		fireEvent.change(days, { target: { value: '7' } });
		fireEvent.blur(days);
		await waitFor(() => expect(opsSaves).toHaveLength(2));
		expect(opsSaves[1]).toMatchObject({ trashExpiryDays: 7 });
		await userEvent.click(sweep);
		await waitFor(() => expect(days).toBeDisabled());
		expect(await ops.getSettings()).toMatchObject({ trashExpiryDays: null });
	});

	it('shows the operations plugin’s refusal under the row and keeps the value in force', async () => {
		await open({
			opsApi: (ops) => ({
				getSettings: () => ops.getSettings(),
				setSettings: () =>
					Promise.reject({
						kind: 'invalid',
						message: 'the undo history can keep at most 500 entries',
					}),
			}),
		});
		await goTo('Operations');
		fireEvent.change(screen.getByRole('spinbutton', { name: 'Undo history depth' }), {
			target: { value: '120' },
		});
		fireEvent.blur(screen.getByRole('spinbutton', { name: 'Undo history depth' }));
		expect(await screen.findByRole('alert')).toHaveTextContent(
			'The change could not be saved: the undo history can keep at most 500 entries',
		);
		expect(screen.getByRole('spinbutton', { name: 'Undo history depth' })).toHaveValue(50);
		expect(screen.getByRole('spinbutton', { name: 'Undo history depth' })).toHaveAttribute(
			'aria-invalid',
			'true',
		);
	});

	it('shows the rows disabled, with the reason, when the operations settings cannot be read', async () => {
		await open({
			opsApi: () => ({
				getSettings: () => Promise.reject({ kind: 'storage', message: 'no ops' }),
				setSettings: () => Promise.reject(new Error('unreachable')),
			}),
		});
		await goTo('Operations');
		expect(screen.getByRole('switch', { name: 'Verify copies after writing' })).toBeDisabled();
		expect(screen.getByRole('spinbutton', { name: 'Undo history depth' })).toBeDisabled();
		expect(
			screen.getByText(/could not be read, so they cannot be changed right now: no ops/),
		).toBeVisible();
	});
});

describe('the Previews & thumbnails page', () => {
	it('shows the thumbnails switch and the size limit with their values', async () => {
		await open();
		await goTo('Previews & thumbnails');
		expect(screen.getByRole('switch', { name: 'Show thumbnails' })).toBeChecked();
		const limit = screen.getByRole('spinbutton', { name: 'Largest file to make a thumbnail of' });
		expect(limit).toHaveValue(50);
		expect(limit).toHaveAttribute('min', '1');
		expect(limit).toHaveAttribute('max', '2048');
		expect(screen.getByText('MB')).toBeInTheDocument();
		expect(screen.queryByText(/unavailable on this system/)).toBeNull();
		// Folder peeks and Quick Look's hover are not built yet, so there is no switch for them.
		expect(screen.queryByRole('switch', { name: /peek/i })).toBeNull();
	});

	it('saves the switch and the limit, and disables the limit while thumbnails are off', async () => {
		const { settings } = await open();
		await goTo('Previews & thumbnails');
		const limit = screen.getByRole('spinbutton', { name: 'Largest file to make a thumbnail of' });
		fireEvent.change(limit, { target: { value: '200' } });
		fireEvent.blur(limit);
		await waitFor(() => expect(settings.current().settings.previews.maxFileMb).toBe(200));
		await userEvent.click(screen.getByRole('switch', { name: 'Show thumbnails' }));
		await waitFor(() => expect(settings.current().settings.previews.thumbnails).toBe(false));
		expect(limit).toBeDisabled();
	});

	it('shows Rust’s range refusal under the limit', async () => {
		const { settings } = await open();
		await goTo('Previews & thumbnails');
		settings.failNext({
			kind: 'invalid',
			message: 'previews.maxFileMb must be between 1 and 2048',
			field: 'previews.maxFileMb',
			min: 1,
			max: 2048,
		});
		const limit = screen.getByRole('spinbutton', { name: 'Largest file to make a thumbnail of' });
		fireEvent.change(limit, { target: { value: '900' } });
		fireEvent.blur(limit);
		expect(await screen.findByRole('alert')).toHaveTextContent(
			'Choose a value between 1 and 2048.',
		);
		expect(limit).toHaveValue(50);
	});

	it('hides the options and gives the plugin’s reason where thumbnails are unavailable', async () => {
		await open({ thumbnails: brokenStatus('No thumbnail cache folder could be found.') });
		await goTo('Previews & thumbnails');
		expect(
			await screen.findByText(
				'Thumbnails are unavailable on this system: No thumbnail cache folder could be found.',
			),
		).toBeInTheDocument();
		expect(screen.queryByRole('switch', { name: 'Show thumbnails' })).toBeNull();
		expect(screen.queryByRole('spinbutton')).toBeNull();
	});

	it('keeps the options when the status cannot be read', async () => {
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		await open({ thumbnails: new Error('no plugin') });
		await goTo('Previews & thumbnails');
		expect(screen.getByRole('switch', { name: 'Show thumbnails' })).toBeInTheDocument();
	});
});

describe('the Drag & drop page', () => {
	it('shows the default rule, delay and Shelf rows with their values', async () => {
		await open();
		await goTo('Drag & drop');
		expect(screen.getByRole('combobox', { name: 'Default drop action' })).toHaveValue('byVolume');
		expect(
			screen.getByRole('option', { name: 'Move on the same volume, copy otherwise' }),
		).toBeInTheDocument();
		expect(screen.getByRole('option', { name: 'Always copy' })).toBeInTheDocument();
		expect(screen.getByRole('option', { name: 'Always ask' })).toBeInTheDocument();
		const delay = screen.getByRole('spinbutton', { name: 'Spring-load delay' });
		expect(delay).toHaveValue(600);
		expect(delay).toHaveAttribute('min', '200');
		expect(delay).toHaveAttribute('max', '2000');
		expect(screen.getByText('ms')).toBeInTheDocument();
		expect(screen.getByRole('switch', { name: 'Keep the Shelf between sessions' })).toBeChecked();
		expect(screen.queryByText(/Dragging files out/)).toBeNull();
	});

	it('saves the rule, the delay and the Shelf choice', async () => {
		const { settings } = await open();
		await goTo('Drag & drop');
		await userEvent.selectOptions(
			screen.getByRole('combobox', { name: 'Default drop action' }),
			'alwaysAsk',
		);
		await waitFor(() =>
			expect(settings.current().settings.dnd.defaultActionRule).toBe('alwaysAsk'),
		);
		const delay = screen.getByRole('spinbutton', { name: 'Spring-load delay' });
		fireEvent.change(delay, { target: { value: '900' } });
		fireEvent.blur(delay);
		await waitFor(() => expect(settings.current().settings.dnd.springLoadMs).toBe(900));
		await userEvent.click(screen.getByRole('switch', { name: 'Keep the Shelf between sessions' }));
		await waitFor(() => expect(settings.current().settings.dnd.shelfPersist).toBe(false));
	});

	it('shows Rust’s range refusal under the delay and goes back to the value in force', async () => {
		const { settings } = await open();
		await goTo('Drag & drop');
		settings.failNext({
			kind: 'invalid',
			message: 'dnd.springLoadMs must be between 200 and 2000',
			field: 'dnd.springLoadMs',
			min: 200,
			max: 2000,
		});
		const delay = screen.getByRole('spinbutton', { name: 'Spring-load delay' });
		fireEvent.change(delay, { target: { value: '900' } });
		fireEvent.blur(delay);
		expect(await screen.findByRole('alert')).toHaveTextContent(
			'Choose a value between 200 and 2000.',
		);
		expect(delay).toHaveValue(600);
		expect(delay).toHaveAccessibleDescription(/Choose a value between 200 and 2000/);
	});

	it('explains, instead of hiding, that dragging out to other applications is unavailable', async () => {
		await open({
			dnd: { outbound: { available: false, reason: 'the compositor has no drag support' } },
		});
		await goTo('Drag & drop');
		expect(
			await screen.findByText(
				'Dragging files out to other applications is unavailable: the compositor has no drag support',
			),
		).toBeInTheDocument();
		// The in-app rows still work.
		expect(screen.getByRole('combobox', { name: 'Default drop action' })).toBeEnabled();
	});

	it('says nothing about availability when the status cannot be read', async () => {
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		await open({ dnd: new Error('no plugin') });
		await goTo('Drag & drop');
		expect(screen.queryByText(/Dragging files out/)).toBeNull();
		expect(screen.getByRole('combobox', { name: 'Default drop action' })).toBeEnabled();
		warn.mockRestore();
	});
});

describe('the Appearance page', () => {
	it('shows the current choices and saves a change through Rust', async () => {
		const { settings } = await open();
		await goTo('Appearance');
		const mode = within(screen.getByRole('radiogroup', { name: 'Colour mode' }));
		expect(mode.getByRole('radio', { name: 'System' })).toBeChecked();
		expect(screen.getByRole('radio', { name: 'Comfortable' })).toBeChecked();
		await userEvent.click(screen.getByRole('radio', { name: 'Dark' }));
		await waitFor(() => expect(settings.calls.at(-1)?.appearance.mode).toBe('dark'));
		await userEvent.click(screen.getByRole('radio', { name: 'Compact' }));
		await waitFor(() => expect(settings.calls.at(-1)?.appearance.density).toBe('compact'));
	});

	it('offers a colour picker only for a custom accent, and saves what is picked', async () => {
		const { settings } = await open();
		await goTo('Appearance');
		expect(screen.queryByLabelText('Custom accent')).toBeNull();
		await userEvent.selectOptions(screen.getByLabelText('Accent colour'), 'custom');
		const picker = await screen.findByLabelText('Custom accent');
		expect(settings.calls.at(-1)?.appearance.accent).toMatchObject({ kind: 'custom' });
		fireEvent.change(picker, { target: { value: '#0f766e' } });
		await waitFor(() =>
			expect(settings.calls.at(-1)?.appearance.accent).toEqual({ kind: 'custom', hex: '#0f766e' }),
		);
		await userEvent.selectOptions(screen.getByLabelText('Accent colour'), 'ember');
		await waitFor(() => expect(screen.queryByLabelText('Custom accent')).toBeNull());
	});
});

describe('the Accessibility page', () => {
	it('lists each preference and every one starts by following the system', async () => {
		await open();
		await goTo('Accessibility');
		for (const name of ['High contrast', 'Reduce motion', 'Reduce transparency']) {
			expect(screen.getByLabelText(name)).toHaveValue('follow');
		}
		expect(screen.getByRole('radio', { name: '100%' })).toBeChecked();
		expect(screen.getByRole('radio', { name: 'Automatic' })).toBeChecked();
		expect(screen.getByRole('switch', { name: 'Stronger focus ring' })).not.toBeChecked();
	});

	it('saves the text size, forced choices and the focus ring', async () => {
		const { settings } = await open();
		await goTo('Accessibility');
		await userEvent.click(screen.getByRole('radio', { name: '130%' }));
		await waitFor(() => expect(settings.calls.at(-1)?.accessibility.textSize).toBe(130));
		await userEvent.selectOptions(screen.getByLabelText('High contrast'), 'on');
		await waitFor(() => expect(settings.calls.at(-1)?.accessibility.highContrast).toBe('on'));
		await userEvent.click(screen.getByRole('switch', { name: 'Stronger focus ring' }));
		await waitFor(() => expect(settings.calls.at(-1)?.accessibility.strongFocusRing).toBe(true));
		await userEvent.click(screen.getByRole('radio', { name: 'On' }));
		await waitFor(() => expect(settings.calls.at(-1)?.accessibility.touchMode).toBe('on'));
	});
});
