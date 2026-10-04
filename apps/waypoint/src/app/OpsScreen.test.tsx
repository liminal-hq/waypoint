// Verifies the Operations window lists the queue on its own store and speaks for every job
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { WindowChromeProvider } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import { tauriWindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { request } from '../test/opsHarness';
import { OpsScreen } from './OpsScreen';

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

function renderScreen(fake = createFakeOpsClient({ concurrency: 4 })) {
	render(
		<WindowChromeProvider controls={tauriWindowControls}>
			<OpsScreen client={fake} />
		</WindowChromeProvider>,
	);
	return fake;
}

describe('OpsScreen', () => {
	it('shows the queue as a list, with the title bar, and no Pop out or Show in folder', async () => {
		const fake = createFakeOpsClient({ concurrency: 4 });
		const first = await fake.submit(request(['a']));
		fake.start(first);
		fake.done(first);
		renderScreen(fake);
		const list = await screen.findByRole('list', { name: 'Jobs' });
		expect(within(list).getAllByRole('listitem')).toHaveLength(1);
		expect(screen.getAllByText('Waypoint — Operations').length).toBeGreaterThan(0);
		expect(screen.queryByRole('button', { name: 'Pop out' })).toBeNull();
		expect(screen.queryByRole('button', { name: /Show in folder/ })).toBeNull();
		expect(screen.getByRole('button', { name: 'Dismiss: Copying a' })).toBeInTheDocument();
	});

	it('follows jobs other windows start and acts on them', async () => {
		const user = userEvent.setup();
		const fake = renderScreen();
		await screen.findByText('Nothing is running.');
		let id = 0;
		await act(async () => {
			id = await fake.submit(request(['b']));
			fake.start(id);
		});
		const cancel = await screen.findByRole('button', { name: 'Cancel: Copying b' });
		await user.click(cancel);
		await waitFor(() => expect(fake.jobs()[0]!.state.state).toBe('cancelling'));
	});

	it('speaks the start and the end of every job through its own live region', async () => {
		const fake = renderScreen();
		await screen.findByText('Nothing is running.');
		let id = 0;
		await act(async () => {
			id = await fake.submit(request(['c']));
			fake.start(id);
		});
		const regions = screen.getAllByRole('status');
		await waitFor(() =>
			expect(regions.map((r) => r.textContent).join('|')).toContain('Started: Copying c'),
		);
		await act(async () => fake.done(id));
		await waitFor(() =>
			expect(
				screen
					.getAllByRole('status')
					.map((r) => r.textContent)
					.join('|'),
			).toContain('Copied c.'),
		);
	});
});

describe('OpsScreen limits and priority', () => {
	it('sets a running copy’s speed limit from its row and speaks it', async () => {
		const user = userEvent.setup();
		const fake = renderScreen();
		await screen.findByText('Nothing is running.');
		let id = 0;
		await act(async () => {
			id = await fake.submit(request(['a']));
			fake.start(id);
		});
		const select = await screen.findByRole('combobox', { name: 'Speed limit: Copying a' });
		expect(select).toHaveValue('');
		expect(screen.queryByRole('combobox', { name: /^Priority/ })).toBeNull();
		await user.selectOptions(select, '10 MB/s');
		await waitFor(() => expect(fake.jobs()[0]!.options.speedLimit).toBe(10_000_000));
		await waitFor(() =>
			expect(
				screen
					.getAllByRole('status')
					.map((r) => r.textContent)
					.join('|'),
			).toContain('Speed limit for Copying a: 10 MB/s'),
		);
		await user.selectOptions(select, 'No limit');
		await waitFor(() => expect(fake.jobs()[0]!.options.speedLimit).toBeUndefined());
	});

	it('offers a priority only while a job waits, and a higher one starts first', async () => {
		const user = userEvent.setup();
		const fake = renderScreen(createFakeOpsClient({ concurrency: 1, autoStart: true }));
		await screen.findByText('Nothing is running.');
		const ids: number[] = [];
		await act(async () => {
			for (const name of ['a', 'b', 'c']) ids.push(await fake.submit(request([name])));
		});
		expect(fake.jobs().map((j) => j.state.state)).toEqual(['running', 'queued', 'queued']);
		const priority = await screen.findByRole('combobox', { name: 'Priority: Copying c' });
		expect(screen.queryByRole('combobox', { name: 'Priority: Copying a' })).toBeNull();
		await user.selectOptions(priority, 'High');
		await waitFor(() => expect(fake.jobs()[2]!.options.priority).toBe('high'));
		await act(async () => fake.done(ids[0]!));
		await waitFor(() =>
			expect(fake.jobs().map((j) => j.state.state)).toEqual(['done', 'queued', 'running']),
		);
	});
});

describe('OpsScreen resolving', () => {
	it('opens the conflict dialog from "Resolve…" for a job another window started', async () => {
		const user = userEvent.setup();
		const fake = renderScreen();
		await screen.findByText('Nothing is running.');
		let id = 0;
		await act(async () => {
			id = await fake.submit(request(['a.txt']));
			fake.start(id);
		});
		await act(async () =>
			fake.askError(
				id,
				{ kind: 'io', message: 'boom' },
				{ display: '/src/a.txt', uri: 'file:///src/a.txt' },
			),
		);
		// The job belongs to main-1: this window waits to be asked.
		expect(screen.queryByRole('dialog')).toBeNull();
		await user.click(await screen.findByRole('button', { name: /^Resolve…/ }));
		expect(
			await screen.findByRole('dialog', { name: 'An item could not be processed' }),
		).toBeInTheDocument();
	});
});
