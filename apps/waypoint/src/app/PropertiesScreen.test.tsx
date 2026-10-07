// Verifies the Properties window: its subject and the Inspector's content, the extra detail, following a rename and a removal, the checksum on request, and Esc
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { tauriWindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls';
import { WindowChromeProvider } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { configureSystemIcons } from '../icons/systemIcons';
import { createFakeSystemIconsClient } from '../services/fakeSystemIconsClient';
import { LazyDetails } from '../inspector/inspectorHarness';
import { createFakeOpenWithClient } from '../openWith/fakeOpenWithClient';
import { FakeChecksumClient } from '../services/fakeChecksumClient';
import { fakeDetails } from '../services/fakeDetailsClient';
import { FakePropertiesWindowClient } from '../services/fakePropertiesWindowClient';
import { FakeTimeFormatClient } from '../services/fakeTimeFormatClient';
import { fileLocation, makeEntry, type FakeVfsClient } from '../services/fakeVfsClient';
import { createTree, DOCS, HOME } from '../test/workspaceHarness';
import { PropertiesScreen } from './PropertiesScreen';

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

const DIGEST = 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad';
const NOTES = fileLocation('/home/test/notes.txt');

function mount(
	options: {
		subject?: ReturnType<typeof fileLocation> | null;
		vfs?: FakeVfsClient;
		onClose?: () => void;
	} = {},
) {
	const vfs = options.vfs ?? createTree();
	const details = new LazyDetails({
		3: {
			details: fakeDetails({
				name: 'notes.txt',
				size: 1200,
				owner: 'scott',
				group: 'staff',
				mode: 0o640,
				createdMs: Date.UTC(2026, 0, 2, 3, 4, 5),
				modifiedMs: Date.UTC(2026, 5, 7, 8, 9, 10),
				accessedMs: Date.UTC(2026, 8, 9, 10, 11, 12),
				unavailable: [],
			}),
		},
		1: { details: fakeDetails({ name: 'docs', kind: 'directory', size: null, mode: 0o755 }) },
		5: { details: fakeDetails({ name: 'Setup.exe', size: 2048, mode: 0o755 }) },
	});
	const windows = new FakePropertiesWindowClient(
		options.subject === undefined ? NOTES : options.subject,
	);
	const checksum = new FakeChecksumClient();
	const onClose = options.onClose ?? vi.fn();
	render(
		<WindowChromeProvider controls={tauriWindowControls}>
			<PropertiesScreen
				vfs={vfs}
				details={details}
				windows={windows}
				checksum={checksum}
				timeFormat={new FakeTimeFormatClient('h23')}
				openWith={createFakeOpenWithClient()}
				onClose={onClose}
				writeText={vi.fn().mockResolvedValue(undefined)}
			/>
		</WindowChromeProvider>,
	);
	return { vfs, details, windows, checksum, onClose };
}

const region = () => screen.findByRole('main', { name: 'Properties of notes.txt' });

describe('the Properties window', () => {
	it('is a labelled region about its subject, with the Inspector’s Properties content', async () => {
		mount();
		const main = await region();
		expect(within(main).getByRole('heading', { level: 1, name: 'notes.txt' })).toBeInTheDocument();
		await waitFor(() => expect(within(main).getAllByText('scott').length).toBeGreaterThan(0));
		expect(within(main).getByText('Location')).toBeInTheDocument();
		expect(within(main).getByText('/home/test')).toBeInTheDocument();
		expect(document.title).toBe('Properties of notes.txt');
	});

	it('adds the detail the narrow panel leaves out: permissions taken apart, times in full, exact size', async () => {
		mount();
		const main = await region();
		const more = await within(main).findByRole('region', { name: 'More detail' });
		const table = within(more).getByRole('table', { name: 'Who may read, write and run it' });
		const owner = within(table).getByRole('row', { name: /^Owner/ });
		expect(
			within(owner)
				.getAllByRole('cell')
				.map((cell) => cell.textContent),
		).toEqual(['Yes', 'Yes', 'No']);
		const group = within(table).getByRole('row', { name: /^Group/ });
		expect(
			within(group)
				.getAllByRole('cell')
				.map((cell) => cell.textContent),
		).toEqual(['Yes', 'No', 'No']);
		expect(within(more).getByText('0640')).toBeInTheDocument();
		expect(within(more).getByText('2026-01-02T03:04:05.000Z UTC')).toBeInTheDocument();
		expect(within(more).getByText('1,200 bytes')).toBeInTheDocument();
		expect(within(more).getByText('Read-only')).toBeInTheDocument();
	});

	it('shows a folder with no checksum, since a folder has none', async () => {
		mount({ subject: DOCS });
		const main = await screen.findByRole('main', { name: 'Properties of docs' });
		await within(main).findByText('Calculating…');
		expect(within(main).queryByRole('heading', { name: 'Checksum' })).toBeNull();
	});

	it('shows a root with its name, place and kind and nothing to list it in', async () => {
		mount({ subject: fileLocation('/') });
		const main = await screen.findByRole('main', { name: 'Properties of /' });
		expect(within(main).getByText('Folder')).toBeInTheDocument();
		expect(within(main).queryByRole('heading', { name: 'Checksum' })).toBeNull();
	});

	it('says what it cannot find for a subject that is not there', async () => {
		mount({ subject: fileLocation('/home/test/missing.txt') });
		expect(
			await screen.findByRole('heading', { name: 'missing.txt no longer exists' }),
		).toBeInTheDocument();
	});

	describe('the icon in its header', () => {
		const root = document.documentElement;

		beforeEach(() => {
			root.dataset.iconTheme = 'system';
		});

		afterEach(() => {
			configureSystemIcons(null);
			delete root.dataset.iconTheme;
		});

		it('is drawn from the program itself, named by the window’s own listing of its folder, never by a path', async () => {
			const fake = createFakeSystemIconsClient();
			configureSystemIcons(fake);
			const vfs = createTree();
			vfs.setFolder(HOME, [
				makeEntry(1, 'docs', { kind: 'directory' }),
				makeEntry(3, 'notes.txt'),
				makeEntry(5, 'Setup.exe'),
			]);
			mount({ subject: fileLocation('/home/test/Setup.exe'), vfs });
			await screen.findByRole('main', { name: 'Properties of Setup.exe' });
			await waitFor(() => expect(fake.probed.length).toBeGreaterThan(0));
			const modified = makeEntry(5, 'Setup.exe').modifiedMs;
			expect(fake.probed).toEqual([`fake://file/1-5?size=32&scale=1&m=${modified}`]);
			expect(fake.probed.join()).not.toContain('home');
		});
	});

	describe('following its subject', () => {
		it('shows the new name when the entry is renamed, and tells Rust, so a request for it finds this window', async () => {
			const { vfs, windows } = mount();
			await region();
			act(() => vfs.updateEntries(HOME, new Map([[3, { name: 'ideas.txt' }]])));
			expect(
				await screen.findByRole('main', { name: 'Properties of ideas.txt' }),
			).toBeInTheDocument();
			await waitFor(() =>
				expect(windows.current?.uri).toBe(fileLocation('/home/test/ideas.txt').uri),
			);
			expect(document.title).toBe('Properties of ideas.txt');
		});

		it('shows that it no longer exists when the entry is removed', async () => {
			const { vfs } = mount();
			await region();
			act(() => vfs.removeEntries(HOME, [3]));
			expect(
				await screen.findByRole('heading', { name: 'notes.txt no longer exists' }),
			).toBeInTheDocument();
			expect(
				screen.getByText(/removed, or moved somewhere this window cannot follow/),
			).toBeInTheDocument();
			expect(screen.queryByRole('heading', { name: 'Checksum' })).toBeNull();
		});

		it('shows that it no longer exists when its folder cannot be listed any more', async () => {
			const { vfs } = mount();
			await region();
			act(() =>
				vfs.failListing(1, {
					kind: 'notFound',
					location: HOME,
				}),
			);
			expect(
				await screen.findByRole('heading', { name: 'notes.txt no longer exists' }),
			).toBeInTheDocument();
		});

		it('stays on its entry while other entries of the folder change', async () => {
			const { vfs } = mount();
			await region();
			act(() => vfs.removeEntries(HOME, [4]));
			await new Promise((resolve) => setTimeout(resolve, 300));
			expect(screen.getByRole('main', { name: 'Properties of notes.txt' })).toBeInTheDocument();
		});
	});

	describe('the checksum', () => {
		it('is never automatic; it runs on request and announces only its completion', async () => {
			const { checksum } = mount();
			const main = await region();
			await within(main).findByRole('heading', { name: 'Checksum' });
			expect(checksum.calls).toEqual([]);
			await userEvent.click(within(main).getByRole('button', { name: 'Calculate checksum…' }));
			expect(checksum.calls).toHaveLength(1);
			expect(checksum.calls[0]).toMatch(/^\d+:3:sha256$/);
			act(() => checksum.finish(checksum.live[0]!, DIGEST, 1200));
			expect(await within(main).findByText(DIGEST)).toBeInTheDocument();
			expect(
				within(main).getByText('The SHA-256 checksum of notes.txt is ready'),
			).toBeInTheDocument();
		});

		it('is reachable and runnable from the keyboard alone', async () => {
			const { checksum } = mount();
			const main = await region();
			const select = await within(main).findByRole('combobox', { name: 'Algorithm' });
			const user = userEvent.setup();
			for (let step = 0; step < 20 && document.activeElement !== select; step++) await user.tab();
			expect(select).toHaveFocus();
			await user.tab();
			expect(within(main).getByRole('button', { name: 'Calculate checksum…' })).toHaveFocus();
			await user.keyboard('{Enter}');
			expect(checksum.calls).toHaveLength(1);
		});

		it('is cancelled when the file is removed under it', async () => {
			const { vfs, checksum } = mount();
			const main = await region();
			await userEvent.click(
				await within(main).findByRole('button', { name: 'Calculate checksum…' }),
			);
			expect(checksum.live).toHaveLength(1);
			act(() => vfs.removeEntries(HOME, [3]));
			await screen.findByRole('heading', { name: 'notes.txt no longer exists' });
			expect(checksum.live).toEqual([]);
		});
	});

	describe('closing', () => {
		it('closes with Esc', async () => {
			const onClose = vi.fn();
			mount({ onClose });
			await region();
			await userEvent.keyboard('{Escape}');
			expect(onClose).toHaveBeenCalledTimes(1);
		});

		it('leaves an Esc that something else took', async () => {
			const onClose = vi.fn();
			mount({ onClose });
			await region();
			const taken = new KeyboardEvent('keydown', {
				key: 'Escape',
				cancelable: true,
				bubbles: true,
			});
			taken.preventDefault();
			window.dispatchEvent(taken);
			expect(onClose).not.toHaveBeenCalled();
		});
	});
});
