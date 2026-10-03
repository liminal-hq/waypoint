// Verifies "Biggest folders in Home": rows as the scan reports them, the cached "as of", Measure now and Cancel, the setting, and the status bar item
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { act, cleanup, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { dismissNotice } from '../app/notices';
import { FakeDevicesClient, fakeVolume } from '../devices/fakeDevicesClient';
import { FakeDirScanClient, fakeDirScanResult } from '../services/fakeDirScanClient';
import { FakePlacesClient, fakePlaces } from '../services/fakePlacesClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { stubLayout } from '../test/browseHarness';
import { HOME, createTree, renderWorkspace } from '../test/workspaceHarness';
import { FakeTrashClient } from '../trash/fakeTrashClient';
import { FRESH_MS } from './homeMeasure';
import { HomeScanStore } from './homeScanStore';
import { OVERVIEW_LOCATION } from './overviewLocation';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	dismissNotice();
	restoreLayout();
	vi.restoreAllMocks();
});

const SYSTEM = fakeVolume('root', {
	label: 'System',
	kind: 'internal',
	isSystem: true,
	mountPoint: '/',
	fileSystem: 'ext4',
	total: 500_000_000_000,
	free: 200_000_000_000,
	canEject: false,
	canUnmount: false,
});

const GB = 1_000_000_000;

function result(
	rows: ReadonlyArray<readonly [string | null, number]>,
	overrides: Parameters<typeof fakeDirScanResult>[2] = {},
) {
	return fakeDirScanResult(HOME, rows, { measuredAtMs: Date.now(), ...overrides });
}

interface Options {
	scan?: FakeDirScanClient;
	measureOnOpen?: boolean;
}

async function setup({ scan = new FakeDirScanClient(), measureOnOpen = true }: Options = {}) {
	const tabs = new FakeTabsApi();
	await tabs.openTab(OVERVIEW_LOCATION);
	const trash = new FakeTrashClient({ count: 0 });
	const h = await renderWorkspace(
		createTree(),
		tabs,
		new FakePlacesClient({ places: fakePlaces('/home/test') }),
		{
			sidebar: true,
			devices: new FakeDevicesClient([SYSTEM]),
			trash,
			ops: trash.opsClient(),
			dirScan: scan,
			settings: createFakeSettingsClient({
				...DEFAULT_SETTINGS,
				previews: { ...DEFAULT_SETTINGS.previews, measureHomeOnOpen: measureOnOpen },
			}),
		},
	);
	return { ...h, scan };
}

const section = () => screen.findByRole('region', { name: 'Overview' }).then(homeSection);
async function homeSection(page: HTMLElement) {
	return within(await within(page).findByRole('region', { name: 'Biggest folders in Home' }));
}
const stat = async (id: string) => {
	const page = await screen.findByRole('region', { name: 'Overview' });
	const tile = page.querySelector(`[data-stat="${id}"]`);
	if (!tile) throw new Error(`no ${id} stat`);
	return within(tile as HTMLElement);
};
const scans = (client: FakeDirScanClient) => client.calls.filter((call) => call.startsWith('scan'));

describe('measuring when Overview opens', () => {
	it('starts a scan of Home and lists the folders as the partial results arrive, largest first', async () => {
		const h = await setup();
		const home = await section();
		await waitFor(() => expect(scans(h.scan)).toHaveLength(1));
		expect(h.scan.calls).toContain('scan file:///home/test');
		expect(home.getByText('Measuring Home…')).toBeInTheDocument();

		act(() =>
			h.scan.partial(1, result([['Backup', 128 * GB]], { foldersScanned: 1, foldersTotal: 4 })),
		);
		expect(
			await home.findByRole('button', { name: 'Backup, 128 GB, 100% of Home' }),
		).toBeInTheDocument();
		expect(home.getByText('Measuring Home: 1 of 4 folders done')).toBeInTheDocument();

		act(() =>
			h.scan.partial(
				1,
				result(
					[
						['Backup', 128 * GB],
						['Photos', 64 * GB],
						['Code', 16 * GB],
					],
					{ foldersScanned: 3, foldersTotal: 4 },
				),
			),
		);
		const rows = (await home.findAllByRole('listitem')).map((row) => row.textContent);
		expect(rows[0]).toContain('Backup');
		expect(rows[1]).toContain('Photos');
		expect(rows[2]).toContain('Code');
		expect(home.getByRole('button', { name: 'Photos, 64 GB, 31% of Home' })).toBeInTheDocument();
	});

	it('ends with the remainder row last, written and hatched rather than coloured', async () => {
		const h = await setup();
		const home = await section();
		await waitFor(() => expect(scans(h.scan)).toHaveLength(1));
		act(() =>
			h.scan.finish(
				1,
				result([
					['Backup', 128 * GB],
					[null, 2 * GB],
				]),
			),
		);
		const items = await home.findAllByRole('listitem');
		expect(items).toHaveLength(2);
		const remainder = items[1]!;
		expect(remainder).toHaveTextContent('Other files and folders, including hidden');
		expect(remainder).toHaveTextContent('2 GB');
		expect(remainder).toHaveTextContent('2%');
		// Not a link into a folder, and its bar is the hatched kind.
		expect(within(remainder).queryByRole('button')).not.toBeInTheDocument();
		expect(remainder.querySelector('[data-kind="other"]')).not.toBeNull();
		expect(home.getByRole('button', { name: 'Measure Home now' })).toBeInTheDocument();
	});

	it('fills the Home stat and "Your files" on the volume that holds Home, then says it is as of a time', async () => {
		const h = await setup();
		await section();
		await waitFor(() => expect(scans(h.scan)).toHaveLength(1));
		expect((await stat('home')).getByText('Measuring…')).toBeInTheDocument();
		act(() => h.scan.finish(1, result([['Backup', 100 * GB]])));
		expect(await (await stat('home')).findByText('100 GB')).toBeInTheDocument();
		expect((await stat('home')).getByText(/33% of the space used on System · as of /)).toBeTruthy();
		const system = await screen.findByRole('group', { name: 'System' });
		await waitFor(() => expect(system.querySelector('[data-part="files"]')).not.toBeNull());
		expect(within(system).getByText('Your files')).toBeInTheDocument();
		expect(within(system).getByText('Everything else')).toBeInTheDocument();
	});

	it('announces the start, each quarter of the folders and the end politely', async () => {
		const h = await setup();
		await section();
		const live = () => screen.getAllByRole('status').map((node) => node.textContent);
		await waitFor(() => expect(live()).toContain('Measuring Home'));
		act(() => h.scan.partial(1, result([['A', GB]], { foldersScanned: 2, foldersTotal: 4 })));
		await waitFor(() => expect(live()).toContain('Measuring Home: 50% done'));
		act(() => h.scan.finish(1, result([['A', GB]])));
		await waitFor(() => expect(live()).toContain('Measured Home: 1 GB'));
	});

	it('notes cloud-only files counted as empty, only when the scan skipped some', async () => {
		const h = await setup();
		const home = await section();
		await waitFor(() => expect(scans(h.scan)).toHaveLength(1));
		act(() => h.scan.finish(1, result([['OneDrive', 0]], { placeholders: 1 })));
		expect(
			await home.findByText('1 cloud-only file was counted as empty and not downloaded.'),
		).toBeInTheDocument();
	});

	it('says why when the scan fails', async () => {
		const h = await setup();
		const home = await section();
		await waitFor(() => expect(scans(h.scan)).toHaveLength(1));
		act(() => h.scan.fail(1, { kind: 'io', message: 'disk went away', location: null }));
		expect(await home.findByText('Home could not be measured: disk went away')).toBeTruthy();
	});
});

describe('the cached result', () => {
	it('shows its rows and "as of" at once, and a fresh one skips the scan', async () => {
		const scan = new FakeDirScanClient();
		scan.setCached(result([['Backup', 128 * GB]], { measuredAtMs: Date.now() - 5 * 60_000 }));
		const h = await setup({ scan });
		const home = await section();
		expect(
			await home.findByRole('button', { name: 'Backup, 128 GB, 100% of Home' }),
		).toBeInTheDocument();
		expect(home.getByText(/^as of /)).toBeInTheDocument();
		expect((await stat('home')).getByText('128 GB')).toBeInTheDocument();
		expect(scans(h.scan)).toHaveLength(0);
	});

	it('is shown while a fresh scan runs when it is over an hour old', async () => {
		const scan = new FakeDirScanClient();
		scan.setCached(result([['Backup', 128 * GB]], { measuredAtMs: Date.now() - FRESH_MS - 1000 }));
		const h = await setup({ scan });
		const home = await section();
		await waitFor(() => expect(scans(h.scan)).toHaveLength(1));
		expect(home.getByRole('button', { name: /^Backup, 128 GB/ })).toBeInTheDocument();
		expect((await stat('home')).getByText('128 GB')).toBeInTheDocument();
		expect(home.getByRole('button', { name: 'Cancel measuring Home' })).toBeInTheDocument();
	});
});

describe('the setting', () => {
	it('off means no scan until Measure now', async () => {
		const h = await setup({ measureOnOpen: false });
		const home = await section();
		expect(home.getByText(/has not been measured yet/)).toBeInTheDocument();
		expect((await stat('home')).getByText('Not measured yet')).toBeInTheDocument();
		await new Promise((resolve) => setTimeout(resolve, 20));
		expect(scans(h.scan)).toHaveLength(0);

		await userEvent.click(home.getByRole('button', { name: 'Measure Home now' }));
		await waitFor(() => expect(scans(h.scan)).toHaveLength(1));
	});

	it('off still shows the cached result without measuring', async () => {
		const scan = new FakeDirScanClient();
		scan.setCached(result([['Backup', 128 * GB]], { measuredAtMs: Date.now() - 10 * FRESH_MS }));
		const h = await setup({ scan, measureOnOpen: false });
		const home = await section();
		expect(await home.findByRole('button', { name: /^Backup/ })).toBeInTheDocument();
		expect(scans(h.scan)).toHaveLength(0);
	});
});

describe('Measure now and Cancel', () => {
	it('Cancel stops the scan at once and keeps the folders reached', async () => {
		const h = await setup();
		const home = await section();
		await waitFor(() => expect(scans(h.scan)).toHaveLength(1));
		act(() => h.scan.partial(1, result([['Backup', GB]], { foldersScanned: 1, foldersTotal: 3 })));
		await userEvent.click(await home.findByRole('button', { name: 'Cancel measuring Home' }));
		expect(h.scan.cancelled).toEqual([1]);
		expect(await home.findByRole('button', { name: 'Measure Home now' })).toBeInTheDocument();
		expect(
			home.getByText('Measuring stopped. These are the folders measured so far.'),
		).toBeTruthy();
		expect(home.getByRole('button', { name: /^Backup/ })).toBeInTheDocument();
		// A late event from the stopped scan changes nothing.
		act(() => h.scan.partial(1, result([['Late', GB]])));
		expect(home.queryByRole('button', { name: /^Late/ })).not.toBeInTheDocument();
	});

	it('Measure now after a cancel starts a new scan', async () => {
		const h = await setup();
		const home = await section();
		await waitFor(() => expect(scans(h.scan)).toHaveLength(1));
		await userEvent.click(await home.findByRole('button', { name: 'Cancel measuring Home' }));
		await userEvent.click(await home.findByRole('button', { name: 'Measure Home now' }));
		await waitFor(() => expect(scans(h.scan)).toHaveLength(2));
	});

	it('never runs two scans at once', async () => {
		const scan = new FakeDirScanClient();
		const store = new HomeScanStore(scan);
		const home: Location = { display: '/home/test', uri: 'file:///home/test' };
		expect(store.start(home)).toBe(true);
		expect(store.start(home)).toBe(false);
		await Promise.resolve();
		expect(scans(scan)).toHaveLength(1);
		store.cancel();
		expect(store.getSnapshot().running).toBe(false);
		expect(store.start(home)).toBe(true);
	});

	it('stops a scan cancelled before the engine has answered', async () => {
		const scan = new FakeDirScanClient();
		const store = new HomeScanStore(scan);
		const home: Location = { display: '/home/test', uri: 'file:///home/test' };
		store.start(home);
		store.cancel();
		await waitFor(() => expect(scan.cancelled).toEqual([1]));
	});
});

describe('leaving Overview', () => {
	it('cancels the scan when the tab moves to another location', async () => {
		const h = await setup();
		await section();
		await waitFor(() => expect(scans(h.scan)).toHaveLength(1));
		await act(async () => {
			await h.tabs.navigate(1, HOME);
		});
		await waitFor(() => expect(h.scan.cancelled).toEqual([1]));
	});

	it('cancels the scan when the tab closes', async () => {
		const h = await setup();
		await section();
		await waitFor(() => expect(scans(h.scan)).toHaveLength(1));
		await act(async () => {
			await h.tabs.openTab(HOME);
			await h.tabs.closeTab(1);
		});
		await waitFor(() => expect(h.scan.cancelled).toEqual([1]));
	});
});

describe('the rows by keyboard', () => {
	it('Enter on a folder opens it in the tab', async () => {
		const h = await setup();
		const home = await section();
		await waitFor(() => expect(scans(h.scan)).toHaveLength(1));
		act(() =>
			h.scan.finish(
				1,
				result([
					['docs', 4 * GB],
					[null, GB],
				]),
			),
		);
		const row = await home.findByRole('button', { name: 'docs, 4 GB, 80% of Home' });
		row.focus();
		await userEvent.keyboard('{Enter}');
		await waitFor(async () => {
			const snapshot = await h.tabs.getSnapshot();
			expect(snapshot.tabs[0]!.location.uri).toBe('file:///home/test/docs');
		});
	});
});

describe('the status bar item', () => {
	it('shows Measuring Home with the folders done while a scan runs, and nothing otherwise', async () => {
		const h = await setup();
		const bar = await screen.findByRole('group', { name: 'Status bar' });
		await waitFor(() => expect(scans(h.scan)).toHaveLength(1));
		expect(await within(bar).findByText('Measuring Home')).toBeInTheDocument();
		act(() => h.scan.partial(1, result([['A', GB]], { foldersScanned: 2, foldersTotal: 5 })));
		expect(await within(bar).findByText('Measuring Home: 2 of 5')).toBeInTheDocument();
		act(() => h.scan.finish(1, result([['A', GB]])));
		await waitFor(() => expect(within(bar).queryByText(/Measuring Home/)).not.toBeInTheDocument());
	});

	it('is hidden when Overview does not measure', async () => {
		await setup({ measureOnOpen: false });
		const bar = await screen.findByRole('group', { name: 'Status bar' });
		expect(within(bar).queryByText(/Measuring Home/)).not.toBeInTheDocument();
	});

	it('can cancel the scan', async () => {
		const h = await setup();
		const bar = await screen.findByRole('group', { name: 'Status bar' });
		await userEvent.click(
			await within(bar).findByRole('button', { name: 'Cancel measuring Home' }),
		);
		expect(h.scan.cancelled).toEqual([1]);
		await waitFor(() => expect(within(bar).queryByText(/Measuring Home/)).not.toBeInTheDocument());
	});
});
