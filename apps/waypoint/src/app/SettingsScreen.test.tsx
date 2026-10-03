// Verifies the Settings window: its pages and rows, saving through Rust's owners, refusals, unavailable states and live updates
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { tauriWindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls';
import { WindowChromeProvider } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import { configureSystemIcons } from '../icons/systemIcons';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { createFakeSystemIconsClient } from '../services/fakeSystemIconsClient';
import { createFakeSettingsClient, type FakeSettings } from '../services/fakeSettingsClient';
import { DEFAULT_SETTINGS, type Settings } from '../services/settingsClient';
import type { PluginStatus } from '@liminal-hq/plugin-thumbnails';
import type { PluginStatus as WindowEffectsStatus } from '@liminal-hq/plugin-window-effects';
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

/** What the window effects plugin reports on GNOME Wayland: windows can be see-through, nothing can blur behind them. */
function gnomeEffects(): WindowEffectsStatus {
	const feature = (name: string, available: boolean, reason: string | null = null) => ({
		name,
		available,
		reason: (reason ? 'compositor-has-no-blur' : null) as 'compositor-has-no-blur' | null,
		message: reason,
	});
	return {
		available: true,
		reason: 'compositor-has-no-blur',
		message: 'GNOME does not let apps blur behind their windows',
		flavour: 'wayland',
		features: [
			feature('opacity', true),
			feature('blur', false, 'GNOME does not let apps blur behind their windows'),
			feature('mica', false),
			feature('acrylic', false),
			feature('shadowInset', true),
		],
	};
}

/** KDE Wayland: blur works too. */
function kdeEffects(): WindowEffectsStatus {
	const status = gnomeEffects();
	return {
		...status,
		reason: null,
		message: null,
		features: status.features.map((f) =>
			f.name === 'blur' ? { ...f, available: true, reason: null, message: null } : f,
		),
	};
}

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
		effects?: WindowEffectsStatus | Error;
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
	const effects = options.effects ?? gnomeEffects();
	render(
		<WindowChromeProvider controls={tauriWindowControls}>
			<SettingsScreen
				client={settings}
				ops={opsApi}
				dndStatus={() => (dnd instanceof Error ? Promise.reject(dnd) : Promise.resolve(dnd))}
				thumbnailsStatus={() =>
					thumbnails instanceof Error ? Promise.reject(thumbnails) : Promise.resolve(thumbnails)
				}
				windowEffectsStatus={() =>
					effects instanceof Error ? Promise.reject(effects) : Promise.resolve(effects)
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
			'Transparency',
			'Accessibility',
			'Language & region',
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
			rememberFolderViews: true,
		});
	});

	it('shows the remember-each-folder switch on, and turns it off and on again', async () => {
		const { settings } = await open();
		const row = screen.getByRole('switch', { name: 'Remember each folder’s view' });
		expect(row).toBeChecked();
		expect(row).toHaveAccessibleDescription(/up to 1,000 folders/);
		await userEvent.click(row);
		await waitFor(() => expect(row).not.toBeChecked());
		expect(settings.current().settings.general.rememberFolderViews).toBe(false);
		expect(settings.current().settings).toEqual({
			...DEFAULT_SETTINGS,
			general: { ...DEFAULT_SETTINGS.general, rememberFolderViews: false },
		});
		await userEvent.click(row);
		await waitFor(() => expect(row).toBeChecked());
		expect(settings.current().settings.general.rememberFolderViews).toBe(true);
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

	it('saves "Measure Home when Overview opens", on by default, and keeps it where thumbnails are unavailable', async () => {
		const { settings } = await open({ thumbnails: brokenStatus('No cache.') });
		await goTo('Previews & thumbnails');
		const row = await screen.findByRole('switch', { name: 'Measure Home when Overview opens' });
		expect(row).toBeChecked();
		await userEvent.click(row);
		await waitFor(() => expect(settings.current().settings.previews.measureHomeOnOpen).toBe(false));
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

/** The system supplies no icons, as on a headless session. */
function noSystemIcons() {
	const unavailable = { available: false, reason: 'no icon theme is installed' };
	return createFakeSystemIconsClient({
		status: { typeIcons: unavailable, folderIcons: unavailable },
	});
}

describe('the Appearance page icon rows', () => {
	beforeEach(() => configureSystemIcons(noSystemIcons()));
	afterEach(() => configureSystemIcons(null));

	it('offers the Waypoint and Portage themes, not System, with a preview of each', async () => {
		await open();
		await goTo('Appearance');
		const themes = screen.getByRole('radiogroup', { name: 'Icon theme' });
		const radios = within(themes).getAllByRole('radio');
		expect(radios.map((radio) => radio.title)).toEqual(['Waypoint', 'Portage']);
		expect(within(themes).getByRole('radio', { name: 'Waypoint' })).toBeChecked();
		expect(within(themes).queryByRole('radio', { name: 'System' })).toBeNull();
		for (const radio of radios) {
			expect(radio.querySelectorAll('svg[data-group]')).toHaveLength(5);
		}
		const [waypoint, portage] = radios.map((radio) => radio.querySelector('svg[data-group]')!);
		expect(waypoint!.getAttribute('viewBox')).toBe('0 0 16 16');
		expect(portage!.getAttribute('viewBox')).toBe('0 0 64 64');
	});

	it('hides the folder colours under the Waypoint theme, saying why, and greys the icon style only for Portage', async () => {
		await open();
		await goTo('Appearance');
		expect(screen.queryByRole('radiogroup', { name: 'Folder colour' })).toBeNull();
		expect(screen.getByText(/Folder colours apply to the Portage icon theme/)).toBeInTheDocument();
		const style = screen.getByRole('radiogroup', { name: 'Icon style' });
		expect(style).not.toHaveAttribute('aria-disabled');
		expect(
			within(style)
				.getAllByRole('radio')
				.map((radio) => radio.title),
		).toEqual(['Light', 'Regular', 'Bold', 'Filled']);
		await userEvent.click(screen.getByRole('radio', { name: 'Portage' }));
		await screen.findByRole('radiogroup', { name: 'Folder colour' });
		expect(screen.queryByText(/Folder colours apply to the Portage icon theme/)).toBeNull();
		const greyed = screen.getByRole('radiogroup', { name: 'Icon style' });
		expect(greyed).toHaveAttribute('aria-disabled', 'true');
		expect(
			within(greyed)
				.getAllByRole('radio')
				.every((radio) => (radio as HTMLButtonElement).disabled),
		).toBe(true);
		expect(screen.getByText(/does not apply to Portage/)).toBeInTheDocument();
	});

	it('saves the icon style through Rust and shows each style in a sample strip', async () => {
		const { settings } = await open();
		await goTo('Appearance');
		const style = screen.getByRole('radiogroup', { name: 'Icon style' });
		expect(within(style).getByRole('radio', { name: 'Regular' })).toBeChecked();
		expect(style.querySelectorAll('[data-icon-style-preview]')).toHaveLength(4);
		await userEvent.click(within(style).getByRole('radio', { name: 'Bold' }));
		await waitFor(() => expect(settings.calls.at(-1)?.appearance.iconStyle).toBe('bold'));
		await waitFor(() => expect(within(style).getByRole('radio', { name: 'Bold' })).toBeChecked());
	});

	it('saves the theme and the colour through Rust, and shows all ten named swatches', async () => {
		const { settings } = await open();
		await goTo('Appearance');
		await userEvent.click(screen.getByRole('radio', { name: 'Portage' }));
		await waitFor(() => expect(settings.calls.at(-1)?.appearance.iconTheme).toBe('portage'));
		const colours = screen.getByRole('radiogroup', { name: 'Folder colour' });
		expect(
			within(colours)
				.getAllByRole('radio')
				.map((radio) => radio.title),
		).toEqual([
			'Liminal',
			'GNOME',
			'Cinnamon',
			'KDE',
			'Windows 11',
			'Red',
			'Pink',
			'Orange',
			'Purple',
			'Rainbow',
		]);
		expect(within(colours).getByRole('radio', { name: 'Liminal' })).toBeChecked();
		await userEvent.click(within(colours).getByRole('radio', { name: 'Purple' }));
		await waitFor(() => expect(settings.calls.at(-1)?.appearance.folderColour).toBe('purple'));
		expect(settings.calls.at(-1)?.appearance.iconTheme).toBe('portage');
		await waitFor(() =>
			expect(within(colours).getByRole('radio', { name: 'Purple' })).toBeChecked(),
		);
	});

	it('moves between the swatches with the arrow keys, keeping one tab stop', async () => {
		const { settings } = await open({
			settings: {
				...DEFAULT_SETTINGS,
				appearance: { ...DEFAULT_SETTINGS.appearance, iconTheme: 'portage' },
			},
		});
		await goTo('Appearance');
		const colours = screen.getByRole('radiogroup', { name: 'Folder colour' });
		const radios = within(colours).getAllByRole('radio');
		expect(radios.filter((radio) => radio.tabIndex === 0)).toHaveLength(1);
		radios[0]!.focus();
		await userEvent.keyboard('{ArrowRight}');
		await waitFor(() => expect(settings.calls.at(-1)?.appearance.folderColour).toBe('gnome'));
		await userEvent.keyboard('{End}');
		await waitFor(() => expect(settings.calls.at(-1)?.appearance.folderColour).toBe('rainbow'));
		await userEvent.keyboard('{ArrowRight}');
		await waitFor(() => expect(settings.calls.at(-1)?.appearance.folderColour).toBe('liminal'));
	});

	it('shows a System theme the system cannot supply as the Waypoint set it draws, and says why System is not offered', async () => {
		await open({
			settings: {
				...DEFAULT_SETTINGS,
				appearance: { ...DEFAULT_SETTINGS.appearance, iconTheme: 'system' },
			},
		});
		await goTo('Appearance');
		const themes = within(screen.getByRole('radiogroup', { name: 'Icon theme' }));
		await waitFor(() => expect(themes.getByRole('radio', { name: 'Waypoint' })).toBeChecked());
		expect(themes.queryByRole('radio', { name: 'System' })).toBeNull();
		expect(
			screen.getByText(
				'The System icon theme is not available on this system: no icon theme is installed',
			),
		).toBeInTheDocument();
		// The Waypoint set is what is drawn, so its rows are not greyed.
		expect(screen.getByRole('radiogroup', { name: 'Icon style' })).not.toHaveAttribute(
			'aria-disabled',
		);
	});

	it('says nothing about System while the plugin has not answered, and a plain note when it gave no reason', async () => {
		configureSystemIcons(
			createFakeSystemIconsClient({
				status: {
					typeIcons: { available: false, reason: null },
					folderIcons: { available: false, reason: null },
				},
			}),
		);
		await open();
		await goTo('Appearance');
		expect(
			await screen.findByText('The System icon theme is not available on this system.'),
		).toBeInTheDocument();
	});
});

describe('the Appearance page icon rows where the system supplies icons', () => {
	let fake = createFakeSystemIconsClient();
	beforeEach(() => {
		fake = createFakeSystemIconsClient();
		configureSystemIcons(fake);
	});
	afterEach(() => configureSystemIcons(null));

	it('offers System as a third theme, with a strip of the system’s own icons for a folder, an image, a PDF, a document and an archive', async () => {
		await open();
		await goTo('Appearance');
		const themes = screen.getByRole('radiogroup', { name: 'Icon theme' });
		const system = await within(themes).findByRole('radio', { name: 'System' });
		expect(
			within(themes)
				.getAllByRole('radio')
				.map((radio) => radio.title),
		).toEqual(['Waypoint', 'Portage', 'System']);
		expect(screen.queryByText(/The System icon theme is not available/)).toBeNull();
		await waitFor(() => expect(fake.probed.length).toBeGreaterThan(0));
		// One picture per type, drawn at the preview size.
		expect(fake.probed.map((url) => url.split('?')[0]).sort()).toEqual([
			'fake://ext/odt',
			'fake://ext/pdf',
			'fake://ext/png',
			'fake://ext/zip',
			'fake://folder/plain',
		]);
		expect(fake.probed.every((url) => url.includes('size=24'))).toBe(true);
		await act(async () => fake.settle(true));
		expect(system.querySelectorAll('svg[data-system]')).toHaveLength(5);
	});

	it('saves System, then explains that the icon style and folder colours apply to other sets, and greys the style', async () => {
		const { settings } = await open();
		await goTo('Appearance');
		const themes = within(screen.getByRole('radiogroup', { name: 'Icon theme' }));
		await userEvent.click(await themes.findByRole('radio', { name: 'System' }));
		await waitFor(() => expect(settings.calls.at(-1)?.appearance.iconTheme).toBe('system'));
		await waitFor(() => expect(themes.getByRole('radio', { name: 'System' })).toBeChecked());
		expect(
			screen.getByText(/The System theme draws folders as your system does/),
		).toBeInTheDocument();
		expect(screen.queryByRole('radiogroup', { name: 'Folder colour' })).toBeNull();
		const style = screen.getByRole('radiogroup', { name: 'Icon style' });
		expect(style).toHaveAttribute('aria-disabled', 'true');
		expect(
			screen.getByText(/The System theme draws the icons your system draws/),
		).toBeInTheDocument();
	});

	it('shows the saved System choice as checked', async () => {
		await open({
			settings: {
				...DEFAULT_SETTINGS,
				appearance: { ...DEFAULT_SETTINGS.appearance, iconTheme: 'system' },
			},
		});
		await goTo('Appearance');
		const themes = within(screen.getByRole('radiogroup', { name: 'Icon theme' }));
		await waitFor(() => expect(themes.getByRole('radio', { name: 'System' })).toBeChecked());
	});
});

describe('the Language & region page', () => {
	it('offers the languages that ship, the pseudo-locales in a developer build, and the direction', async () => {
		await open();
		await goTo('Language & region');
		const language = screen.getByLabelText('Language');
		expect(language).toHaveValue('system');
		expect([...language.querySelectorAll('option')].map((o) => o.textContent)).toEqual([
			'System default',
			'English (Canada)',
			'Français (Canada)',
			'English with accents (en-XA, for developers)',
			'Mirrored, right to left (ar-XB, for developers)',
		]);
		const direction = within(screen.getByRole('radiogroup', { name: 'Direction' }));
		expect(direction.getByRole('radio', { name: 'Automatic' })).toBeChecked();
	});

	it('saves the language and the direction through Rust', async () => {
		const { settings } = await open();
		await goTo('Language & region');
		await userEvent.selectOptions(screen.getByLabelText('Language'), 'fr-CA');
		await waitFor(() => expect(settings.calls.at(-1)?.locale.language).toBe('fr-CA'));
		await userEvent.click(screen.getByRole('radio', { name: 'Right to left' }));
		await waitFor(() => expect(settings.calls.at(-1)?.locale.direction).toBe('rtl'));
		expect(settings.calls.at(-1)?.locale.language).toBe('fr-CA');
	});

	it('shows a language this build does not offer as the system default', async () => {
		await open({
			settings: {
				...DEFAULT_SETTINGS,
				locale: { ...DEFAULT_SETTINGS.locale, language: 'xx' },
			},
		});
		await goTo('Language & region');
		expect(screen.getByLabelText('Language')).toHaveValue('system');
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

describe('the Transparency page', () => {
	const slider = (name: string) => screen.getByRole('slider', { name });

	it('is off by default, labelled Experimental on Linux, with every other control dimmed', async () => {
		await open();
		await goTo('Transparency');
		const master = screen.getByRole('switch', { name: /Transparent window/ });
		expect(master).toHaveAttribute('aria-checked', 'false');
		expect(screen.getByText('Experimental')).toBeInTheDocument();
		expect(slider('Title bar and menu bar opacity')).toBeDisabled();
		expect(screen.getByRole('switch', { name: /Solid when not in front/ })).toBeDisabled();
	});

	it('hides the blur row where the compositor cannot blur and says why', async () => {
		await open();
		await goTo('Transparency');
		expect(screen.queryByRole('radiogroup', { name: /Blur behind the window/ })).toBeNull();
		expect(
			screen.getByText(/Blur is not available here: GNOME does not let apps blur/),
		).toBeInTheDocument();
	});

	it('offers blur where the compositor can, and saves the level', async () => {
		const { settings } = await open({
			effects: kdeEffects(),
			settings: {
				...DEFAULT_SETTINGS,
				transparency: { ...DEFAULT_SETTINGS.transparency, enabled: true },
			},
		});
		await goTo('Transparency');
		await userEvent.click(
			within(screen.getByRole('radiogroup', { name: /Blur behind the window/ })).getByRole(
				'radio',
				{
					name: 'High',
				},
			),
		);
		await waitFor(() => expect(settings.current().settings.transparency.blur).toBe('high'));
	});

	it('turns the master switch on through Rust', async () => {
		const { settings } = await open();
		await goTo('Transparency');
		await userEvent.click(screen.getByRole('switch', { name: /Transparent window/ }));
		await waitFor(() => expect(settings.current().settings.transparency.enabled).toBe(true));
		expect(slider('Title bar and menu bar opacity')).toBeEnabled();
	});

	it('previews the opacity while the slider is dragged and saves only when it is let go', async () => {
		const { settings } = await open({
			settings: {
				...DEFAULT_SETTINGS,
				transparency: { ...DEFAULT_SETTINGS.transparency, enabled: true },
			},
		});
		await goTo('Transparency');
		const titleBar = () =>
			document.querySelector<HTMLElement>('[data-region="titleBar"]')!.dataset.alpha;
		const before = titleBar();
		fireEvent.input(slider('Title bar and menu bar opacity'), { target: { value: '45' } });
		expect(before).toBe('0.82');
		// No theme colours in a test, so the fallback floor lifts the 45 % to 70 %.
		expect(titleBar()).toBe('0.7');
		expect(settings.calls).toHaveLength(0);
		fireEvent.change(slider('Title bar and menu bar opacity'));
		await waitFor(() => expect(settings.current().settings.transparency.opacity).toBe(45));
	});

	it('shows the refusal under the row and keeps the value in force', async () => {
		const { settings } = await open({
			settings: {
				...DEFAULT_SETTINGS,
				transparency: { ...DEFAULT_SETTINGS.transparency, enabled: true },
			},
		});
		await goTo('Transparency');
		settings.failNext({
			kind: 'invalid',
			message: 'transparency.opacity must be between 40 and 100',
			field: 'transparency.opacity',
			min: 40,
			max: 100,
		});
		fireEvent.input(slider('Title bar and menu bar opacity'), { target: { value: '50' } });
		fireEvent.change(slider('Title bar and menu bar opacity'));
		expect(await screen.findByRole('alert')).toHaveTextContent('between 40 and 100');
		expect(slider('Title bar and menu bar opacity')).toHaveValue('82');
	});

	it('has an independent slider per part, at the defaults that reproduce the old tiers', async () => {
		await open({
			settings: {
				...DEFAULT_SETTINGS,
				transparency: { ...DEFAULT_SETTINGS.transparency, enabled: true },
			},
		});
		await goTo('Transparency');
		expect(slider('Title bar and menu bar opacity')).toHaveValue('82');
		expect(slider('Tabs and toolbar opacity')).toHaveValue('90');
		expect(slider('Sidebar opacity')).toHaveValue('94');
		expect(slider('File area opacity')).toHaveValue('98');
	});

	it('moves one part without moving the others, in the preview and when saved', async () => {
		const { settings } = await open({
			settings: {
				...DEFAULT_SETTINGS,
				transparency: {
					...DEFAULT_SETTINGS.transparency,
					enabled: true,
					regions: { sidebar: true, content: true, titleBar: true },
				},
			},
		});
		await goTo('Transparency');
		const alpha = (region: string) =>
			document.querySelector<HTMLElement>(`[data-region="${region}"]`)!.dataset.alpha;
		fireEvent.input(slider('Sidebar opacity'), { target: { value: '75' } });
		expect(alpha('sidebar')).toBe('0.75');
		expect([alpha('titleBar'), alpha('rows'), alpha('content')]).toEqual(['0.82', '0.9', '0.98']);
		expect(settings.calls).toHaveLength(0);
		fireEvent.change(slider('Sidebar opacity'));
		await waitFor(() => expect(settings.current().settings.transparency.sidebarOpacity).toBe(75));
		const saved = settings.current().settings.transparency;
		expect([saved.opacity, saved.rowsOpacity, saved.contentOpacity]).toEqual([82, 90, 98]);
	});

	it('dims a part slider while its region is solid, and refuses nothing it offers', async () => {
		await open({
			settings: {
				...DEFAULT_SETTINGS,
				transparency: { ...DEFAULT_SETTINGS.transparency, enabled: true },
			},
		});
		await goTo('Transparency');
		// The file area is solid by default.
		expect(slider('File area opacity')).toBeDisabled();
		expect(slider('Sidebar opacity')).toBeEnabled();
		await userEvent.click(screen.getByRole('switch', { name: /File area/ }));
		await waitFor(() => expect(slider('File area opacity')).toBeEnabled());
	});

	it('resets every setting on the page but the master switch, through the settings path', async () => {
		const { settings } = await open({
			settings: {
				...DEFAULT_SETTINGS,
				transparency: {
					...DEFAULT_SETTINGS.transparency,
					enabled: true,
					opacity: 50,
					rowsOpacity: 60,
					sidebarOpacity: 70,
					contentOpacity: 80,
					blur: 'high',
					regions: { sidebar: false, content: true, titleBar: false },
					menus: true,
					menuOpacity: 70,
					solidWhenUnfocused: false,
				},
			},
		});
		await goTo('Transparency');
		const reset = screen.getByRole('button', { name: 'Reset' });
		expect(reset).toBeEnabled();
		await userEvent.click(reset);
		await waitFor(() =>
			expect(settings.current().settings.transparency).toEqual({
				...DEFAULT_SETTINGS.transparency,
				enabled: true,
			}),
		);
		expect(settings.calls).toHaveLength(1);
		await waitFor(() => expect(reset).toBeDisabled());
		expect(slider('Tabs and toolbar opacity')).toHaveValue('90');
	});

	it('has nothing to reset at the defaults, whether the page is on or off', async () => {
		await open();
		await goTo('Transparency');
		expect(screen.getByRole('button', { name: 'Reset' })).toBeDisabled();
	});

	it('keeps the rows from moving when the unfocused note comes and goes', async () => {
		await open({
			settings: {
				...DEFAULT_SETTINGS,
				transparency: { ...DEFAULT_SETTINGS.transparency, enabled: true },
			},
		});
		await goTo('Transparency');
		const note = screen.getByText('This window is solid while it is not in front.');
		// Laid out (not removed) whether or not it is showing, and not under the master switch.
		expect(note).toHaveAttribute('data-active', 'false');
		expect(
			within(
				screen.getByRole('switch', { name: /Transparent window/ }).closest('div')!,
			).queryByText(/solid while it is not in front/),
		).toBeNull();
		expect(note.closest('[class*="row" i]')).toContainElement(
			screen.getByRole('switch', { name: /Solid when not in front/ }),
		);
		document.documentElement.dataset.transparencyReason = 'unfocused';
		try {
			await waitFor(() => expect(note).toHaveAttribute('data-active', 'true'));
			expect(screen.getByText('This window is solid while it is not in front.')).toBe(note);
		} finally {
			delete document.documentElement.dataset.transparencyReason;
		}
	});

	it('keeps the menu opacity dimmed until menus are translucent', async () => {
		await open({
			settings: {
				...DEFAULT_SETTINGS,
				transparency: { ...DEFAULT_SETTINGS.transparency, enabled: true },
			},
		});
		await goTo('Transparency');
		expect(slider('Menu opacity')).toBeDisabled();
		await userEvent.click(screen.getByRole('switch', { name: /Translucent menus/ }));
		await waitFor(() => expect(slider('Menu opacity')).toBeEnabled());
	});

	it('offers nothing where windows cannot be see-through, and says why', async () => {
		const status = gnomeEffects();
		await open({
			effects: {
				...status,
				features: status.features.map((f) =>
					f.name === 'opacity'
						? {
								...f,
								available: false,
								reason: 'x11-no-compositor' as never,
								message: 'no compositing manager is running, so windows cannot be transparent',
							}
						: f,
				),
			},
		});
		await goTo('Transparency');
		expect(
			screen.getByText(/Transparency is unavailable on this system: no compositing/),
		).toBeInTheDocument();
		expect(screen.queryByRole('switch')).toBeNull();
	});

	it('does not offer anything when the plugin cannot be read', async () => {
		await open({ effects: new Error('no plugin') });
		await goTo('Transparency');
		expect(
			screen.getByText(/could not tell whether this system can show the desktop/),
		).toBeInTheDocument();
		expect(screen.queryByRole('switch')).toBeNull();
	});

	it('is not labelled Experimental on Windows', async () => {
		const status = gnomeEffects();
		await open({ effects: { ...status, flavour: 'windows' } });
		await goTo('Transparency');
		expect(screen.queryByText('Experimental')).toBeNull();
	});
});
