// Verifies the status bar ring and its popover: states, the keyboard model, reordering and the row actions
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { dismissNotice } from '../app/notices';
import { NoticeToast } from '../app/NoticeToast';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { FakeVfsClient, fileLocation } from '../services/fakeVfsClient';
import { StatusBar } from '../status/StatusBar';
import { request } from '../test/opsHarness';
import { OpsProvider } from './OpsContext';
import { OpsIndicator } from './OpsIndicator';
import { registerResolver } from './resolveHook';

afterEach(() => {
	cleanup();
	dismissNotice();
});

interface Mounted {
	fake: FakeOpsClient;
	popOut: ReturnType<typeof vi.fn>;
	showInFolder: ReturnType<typeof vi.fn>;
}

async function mount(fake = createFakeOpsClient({ concurrency: 4 })): Promise<Mounted> {
	const popOut = vi.fn();
	const showInFolder = vi.fn();
	render(
		<OpsProvider client={fake} windowLabel="main-1" popOut={popOut} showInFolder={showInFolder}>
			<OpsIndicator />
			<NoticeToast />
		</OpsProvider>,
	);
	// The provider reads the snapshot after it mounts.
	await act(async () => {
		await Promise.resolve();
	});
	return { fake, popOut, showInFolder };
}

const ring = () => document.querySelector<HTMLButtonElement>('[data-ops-ring]')!;
const submit = async (fake: FakeOpsClient, ...names: string[]) => {
	let id = 0;
	await act(async () => {
		id = await fake.submit(request(names));
	});
	return id;
};
const run = (work: () => void) => act(async () => work());

describe('the ring', () => {
	it('is quiet with nothing queued', async () => {
		await mount();
		expect(ring()).toHaveAttribute('data-urgency', 'idle');
		expect(ring()).toHaveAccessibleName('Operations, none in progress');
		// A hover tooltip says the same, since the ring is only a picture.
		expect(ring()).toHaveAttribute('title', 'Operations, none in progress');
		expect(ring()).toHaveAttribute('aria-expanded', 'false');
		expect(document.querySelector('circle')).toBeNull();
	});

	it('shows the fill and the count while jobs run', async () => {
		const { fake } = await mount();
		const a = await submit(fake, 'a');
		const b = await submit(fake, 'b');
		await run(() => fake.start(a));
		await run(() => fake.tick(a, { bytesDone: 50, bytesTotal: 100 }));
		await waitFor(() => expect(ring()).toHaveAttribute('data-urgency', 'running'));
		await waitFor(() =>
			expect(ring()).toHaveAccessibleName('Operations, 2 in progress, 50% complete'),
		);
		expect(ring()).toHaveTextContent('2');
		void b;
		const arc = document.querySelector('circle[data-fraction]')!;
		expect(arc.getAttribute('data-fraction')).toBe('0.500');
	});

	it('shows a still quarter, not a spinner, for jobs that are not sized yet', async () => {
		const { fake } = await mount(createFakeOpsClient({ totals: () => ({ items: 0, bytes: 0 }) }));
		await submit(fake, 'a');
		const arc = document.querySelector('circle[stroke-dasharray]:not([data-fraction])')!;
		expect(arc).not.toBeNull();
		expect(arc.getAttribute('stroke-dasharray')).toMatch(/^[\d.]+ [\d.]+$/);
		expect(document.querySelector('animate, animateTransform')).toBeNull();
	});

	it('marks a job that waits for the person and then a failed one', async () => {
		const { fake } = await mount();
		const a = await submit(fake, 'a');
		await run(() => fake.start(a));
		await run(() => fake.askConflicts(a, []));
		await waitFor(() => expect(ring()).toHaveAttribute('data-urgency', 'waiting'));
		expect(ring()).toHaveAccessibleName(expect.stringContaining('waiting for you'));
		expect(ring().querySelector('[data-kind="waiting"]')).not.toBeNull();

		const b = await submit(fake, 'b');
		await run(() => fake.cancel(a));
		await run(() => fake.settleCancel(a));
		await run(() => fake.start(b));
		await run(() => fake.fail(b, { kind: 'io', message: 'x' }));
		await waitFor(() => expect(ring()).toHaveAttribute('data-urgency', 'failed'));
		expect(ring().querySelector('[data-kind="failed"]')).not.toBeNull();
	});

	it('shows a check once everything has finished', async () => {
		const { fake } = await mount();
		const a = await submit(fake, 'a');
		await run(() => fake.start(a));
		await run(() => fake.done(a));
		await waitFor(() => expect(ring()).toHaveAttribute('data-urgency', 'done'));
		expect(ring()).toHaveAccessibleName('Operations, all finished');
	});

	it('is in the status bar, before the view switcher', async () => {
		const fake = createFakeOpsClient();
		render(
			<VfsClientProvider client={new FakeVfsClient()}>
				<OpsProvider client={fake} windowLabel="main-1">
					<StatusBar session={null} location={undefined} notice={null}>
						<button type="button">View</button>
					</StatusBar>
				</OpsProvider>
			</VfsClientProvider>,
		);
		const bar = screen.getByRole('group', { name: 'Status bar' });
		const buttons = within(bar).getAllByRole('button');
		expect(buttons.map((b) => b.getAttribute('data-ops-ring') ?? b.textContent)).toEqual([
			'',
			'View',
		]);
	});

	it('is absent where there is no queue', () => {
		render(<OpsIndicator />);
		expect(document.querySelector('[data-ops-ring]')).toBeNull();
	});
});

describe('the popover', () => {
	it('opens with Enter or Space, focuses the first row, and closes with Escape returning focus to the ring', async () => {
		const user = userEvent.setup();
		const { fake } = await mount();
		await submit(fake, 'a');
		ring().focus();
		await user.keyboard('{Enter}');
		const dialog = await screen.findByRole('dialog', { name: 'Operations' });
		expect(dialog).toHaveAttribute('aria-modal', 'false');
		expect(ring()).toHaveAttribute('aria-expanded', 'true');
		const rows = within(dialog).getAllByRole('listitem');
		expect(rows).toHaveLength(1);
		expect(rows[0]).toHaveFocus();

		await user.keyboard('{Escape}');
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(ring()).toHaveFocus();

		await user.keyboard(' ');
		expect(await screen.findByRole('dialog')).toBeInTheDocument();
	});

	it('lists each job with its title, route and state', async () => {
		const user = userEvent.setup();
		const { fake } = await mount();
		const a = await submit(fake, 'report.pdf');
		await run(() => fake.start(a));
		await user.click(ring());
		const row = within(await screen.findByRole('dialog')).getByRole('listitem');
		expect(row).toHaveAccessibleName('Copying report.pdf');
		expect(row).toHaveAccessibleDescription('Running');
		expect(within(row).getByText('report.pdf → dest')).toBeInTheDocument();
		expect(within(row).getByRole('progressbar')).toBeInTheDocument();
	});

	it('is non-modal: a press outside closes it without taking focus', async () => {
		const user = userEvent.setup();
		const { fake } = await mount();
		await submit(fake, 'a');
		await user.click(ring());
		await screen.findByRole('dialog');
		fireEvent.pointerDown(document.body);
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(ring()).not.toHaveFocus();
	});

	it('moves between rows with the arrow keys, Home and End', async () => {
		const user = userEvent.setup();
		const { fake } = await mount();
		await submit(fake, 'a');
		await submit(fake, 'b');
		await submit(fake, 'c');
		await user.click(ring());
		const rows = within(await screen.findByRole('dialog')).getAllByRole('listitem');
		expect(rows[0]).toHaveFocus();
		await user.keyboard('{ArrowDown}');
		expect(rows[1]).toHaveFocus();
		await user.keyboard('{End}');
		expect(rows[2]).toHaveFocus();
		await user.keyboard('{ArrowDown}');
		expect(rows[2]).toHaveFocus();
		await user.keyboard('{Home}');
		expect(rows[0]).toHaveFocus();
	});

	it('reorders a queued job with Alt+Up and Alt+Down and keeps focus on it', async () => {
		const user = userEvent.setup();
		const { fake } = await mount(createFakeOpsClient({ concurrency: 1 }));
		const a = await submit(fake, 'a');
		const b = await submit(fake, 'b');
		const c = await submit(fake, 'c');
		await run(() => fake.start(a));
		await user.click(ring());
		const dialog = await screen.findByRole('dialog');
		const rowFor = (id: number) => dialog.querySelector<HTMLElement>(`[data-job="${id}"]`)!;
		await act(async () => rowFor(c).focus());
		await user.keyboard('{Alt>}{ArrowUp}{/Alt}');
		await waitFor(() => expect(fake.jobs().map((j) => j.id)).toEqual([a, c, b]));
		await waitFor(() => expect(rowFor(c)).toHaveFocus());
		await user.keyboard('{Alt>}{ArrowDown}{/Alt}');
		await waitFor(() => expect(fake.jobs().map((j) => j.id)).toEqual([a, b, c]));
		await waitFor(() => expect(rowFor(c)).toHaveFocus());
		// The running job cannot be moved.
		await act(async () => rowFor(a).focus());
		await user.keyboard('{Alt>}{ArrowDown}{/Alt}');
		expect(fake.jobs().map((j) => j.id)).toEqual([a, b, c]);
	});

	it('pauses, resumes and cancels from the row buttons', async () => {
		const user = userEvent.setup();
		const { fake } = await mount();
		const a = await submit(fake, 'a');
		await run(() => fake.start(a));
		await user.click(ring());
		const dialog = await screen.findByRole('dialog');
		await user.click(within(dialog).getByRole('button', { name: 'Pause: Copying a' }));
		await waitFor(() => expect(fake.jobs()[0]!.state.state).toBe('paused'));
		await user.click(await within(dialog).findByRole('button', { name: 'Resume: Copying a' }));
		await waitFor(() => expect(fake.jobs()[0]!.state.state).toBe('running'));
		await user.click(within(dialog).getByRole('button', { name: 'Cancel: Copying a' }));
		await waitFor(() => expect(fake.jobs()[0]!.state.state).toBe('cancelling'));
		expect(within(dialog).queryByRole('button', { name: /Cancel: / })).toBeNull();
	});

	it('retries and dismisses a failed job and clears the finished ones', async () => {
		const user = userEvent.setup();
		const { fake } = await mount();
		const a = await submit(fake, 'a');
		await run(() => fake.start(a));
		await run(() => fake.fail(a, { kind: 'io', message: 'x' }));
		await user.click(ring());
		const dialog = await screen.findByRole('dialog');
		await user.click(within(dialog).getByRole('button', { name: 'Retry: Copying a' }));
		await waitFor(() => expect(fake.jobs()).toHaveLength(2));
		await user.click(within(dialog).getByRole('button', { name: 'Clear finished' }));
		await waitFor(() => expect(fake.jobs()).toHaveLength(1));
		expect(fake.jobs()[0]!.id).not.toBe(a);
		expect(within(dialog).getByRole('button', { name: 'Clear finished' })).toBeDisabled();
	});

	it('shows a finished job’s folder and dismisses it', async () => {
		const user = userEvent.setup();
		const { fake, showInFolder } = await mount();
		const a = await submit(fake, 'a');
		await run(() => fake.start(a));
		await run(() => fake.done(a));
		await user.click(ring());
		const dialog = await screen.findByRole('dialog');
		await user.click(within(dialog).getByRole('button', { name: 'Show in folder: Copying a' }));
		expect(showInFolder).toHaveBeenCalledWith(fileLocation('/dest'));
		await user.click(within(dialog).getByRole('button', { name: 'Dismiss: Copying a' }));
		await waitFor(() => expect(fake.jobs()).toHaveLength(0));
		expect(within(dialog).getByText('Nothing is running.')).toBeInTheDocument();
	});

	it('hands a waiting job to the resolver, or says it cannot yet', async () => {
		const user = userEvent.setup();
		const { fake } = await mount();
		const a = await submit(fake, 'a');
		await run(() => fake.start(a));
		await run(() => fake.askConflicts(a, []));
		await user.click(ring());
		const dialog = await screen.findByRole('dialog');
		const resolve = await within(dialog).findByRole('button', { name: 'Resolve…: Copying a' });
		await user.click(resolve);
		// No dialog is registered yet, so a notice says so.
		expect(
			await screen.findByText('Answering from here is not available yet.'),
		).toBeInTheDocument();

		const handler = vi.fn();
		const unregister = registerResolver(handler);
		await user.click(resolve);
		expect(handler).toHaveBeenCalledWith(a);
		unregister();
	});

	it('pops out into the Operations window and closes', async () => {
		const user = userEvent.setup();
		const { fake, popOut } = await mount();
		await submit(fake, 'a');
		await user.click(ring());
		await user.click(await screen.findByRole('button', { name: 'Pop out' }));
		expect(popOut).toHaveBeenCalledTimes(1);
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('says so when nothing is running', async () => {
		const user = userEvent.setup();
		await mount();
		await user.click(ring());
		const dialog = await screen.findByRole('dialog');
		expect(within(dialog).getByRole('group', { name: 'Operations' })).toHaveFocus();
		expect(within(dialog).getByText('Nothing is running.')).toBeInTheDocument();
	});
});
