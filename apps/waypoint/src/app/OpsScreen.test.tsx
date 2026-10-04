// Verifies the Operations window lists the queue on its own store and speaks for every job
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
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

describe('OpsScreen scheduling', () => {
	it('schedules a waiting job from its row, words it in the row and the live region, and runs it now', async () => {
		const user = userEvent.setup();
		const fake = renderScreen(createFakeOpsClient({ concurrency: 1, autoStart: true }));
		await screen.findByText('Nothing is running.');
		await act(async () => {
			await fake.submit(request(['a']));
			await fake.submit(request(['b']));
		});
		expect(screen.queryByRole('button', { name: 'Schedule…: Copying a' })).toBeNull();
		const open = await screen.findByRole('button', { name: 'Schedule…: Copying b' });
		await user.click(open);
		const form = await screen.findByRole('group', { name: 'Schedule: Copying b' });
		expect(within(form).queryByRole('button', { name: 'Run now' })).toBeNull();
		// A time that has passed is refused where the person can read why.
		const time = within(form).getByLabelText('Start time');
		fireEvent.change(time, { target: { value: '2001-01-01T10:00' } });
		await user.click(within(form).getByRole('button', { name: 'Set schedule' }));
		expect(await within(form).findByRole('alert')).toHaveTextContent(
			'Choose a time in the future.',
		);
		expect(fake.jobs()[1]!.options.schedule).toBeUndefined();
		fireEvent.change(time, { target: { value: '2999-01-01T10:00' } });
		await user.click(within(form).getByRole('button', { name: 'Set schedule' }));
		await waitFor(() => expect(fake.jobs()[1]!.options.schedule?.kind).toBe('startAt'));
		expect(screen.queryByRole('group', { name: 'Schedule: Copying b' })).toBeNull();
		expect(await screen.findByText(/^Scheduled for .*2999/)).toBeInTheDocument();
		await waitFor(() =>
			expect(
				screen
					.getAllByRole('status')
					.map((r) => r.textContent)
					.join('|'),
			).toContain('Schedule for Copying b: Scheduled for'),
		);
		// The row's button now offers Run now, which clears it.
		await user.click(screen.getByRole('button', { name: 'Schedule…: Copying b' }));
		const again = await screen.findByRole('group', { name: 'Schedule: Copying b' });
		await user.click(within(again).getByRole('button', { name: 'Run now' }));
		await waitFor(() => expect(fake.jobs()[1]!.options.schedule).toBeUndefined());
	});

	it('takes a daily window, refuses two equal times and closes on Escape', async () => {
		const user = userEvent.setup();
		const fake = renderScreen(createFakeOpsClient({ concurrency: 1, autoStart: true }));
		await screen.findByText('Nothing is running.');
		await act(async () => {
			await fake.submit(request(['a']));
			await fake.submit(request(['b']));
		});
		await user.click(await screen.findByRole('button', { name: 'Schedule…: Copying b' }));
		const form = await screen.findByRole('group', { name: 'Schedule: Copying b' });
		await user.click(within(form).getByRole('radio', { name: 'Only between' }));
		fireEvent.change(within(form).getByLabelText('From'), { target: { value: '09:00' } });
		fireEvent.change(within(form).getByLabelText('Until'), { target: { value: '09:00' } });
		await user.click(within(form).getByRole('button', { name: 'Set schedule' }));
		expect(await within(form).findByRole('alert')).toHaveTextContent('Choose two different times.');
		fireEvent.change(within(form).getByLabelText('Until'), { target: { value: '17:30' } });
		await user.click(within(form).getByRole('button', { name: 'Set schedule' }));
		await waitFor(() =>
			expect(fake.jobs()[1]!.options.schedule).toMatchObject({
				kind: 'window',
				startMinute: 540,
				endMinute: 1050,
			}),
		);
		await user.click(screen.getByRole('button', { name: 'Schedule…: Copying b' }));
		await screen.findByRole('group', { name: 'Schedule: Copying b' });
		await user.keyboard('{Escape}');
		await waitFor(() =>
			expect(screen.queryByRole('group', { name: 'Schedule: Copying b' })).toBeNull(),
		);
	});
});

describe('OpsScreen Pause all', () => {
	it('pauses every running job, says nothing starts, and resumes them', async () => {
		const user = userEvent.setup();
		const fake = renderScreen(createFakeOpsClient({ concurrency: 2, autoStart: true }));
		await screen.findByText('Nothing is running.');
		const pause = screen.getByRole('button', { name: 'Pause all' });
		expect(pause).toBeDisabled();
		await act(async () => {
			await fake.submit(request(['a']));
		});
		await waitFor(() => expect(screen.getByRole('button', { name: 'Pause all' })).toBeEnabled());
		await user.click(screen.getByRole('button', { name: 'Pause all' }));
		await waitFor(() => expect(fake.jobs()[0]!.state.state).toBe('paused'));
		expect(await screen.findByText('Paused: nothing starts until you resume.')).toBeInTheDocument();
		await waitFor(() =>
			expect(
				screen
					.getAllByRole('status')
					.map((r) => r.textContent)
					.join('|'),
			).toContain('All jobs paused'),
		);
		await user.click(screen.getByRole('button', { name: 'Resume all' }));
		await waitFor(() => expect(fake.jobs()[0]!.state.state).toBe('running'));
		expect(screen.queryByText(/^Paused: nothing starts/)).toBeNull();
		expect(screen.getByRole('button', { name: 'Pause all' })).toBeInTheDocument();
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
