// Verifies the resolver host: which window opens which dialog, one answerer per job, closing when answered elsewhere, and the ring
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';
import { dismissNotice } from '../app/notices';
import { NoticeToast } from '../app/NoticeToast';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { useAnnouncement } from '../tabs/announcer';
import { conflictFor, request } from '../test/opsHarness';
import { OpsIndicator } from './OpsIndicator';
import { OpsProvider } from './OpsContext';
import { OpsPanel } from './OpsPanel';
import { OpsResolverHost } from './OpsResolverHost';
import { registerResolver, requestResolve } from './resolveHook';

afterEach(() => {
	cleanup();
	dismissNotice();
});

function Live() {
	return <div data-testid="live">{useAnnouncement()}</div>;
}

async function mount(fake: FakeOpsClient, windowLabel = 'main-1') {
	render(
		<OpsProvider client={fake} windowLabel={windowLabel}>
			<OpsIndicator />
			<OpsPanel layout="window" />
			<OpsResolverHost />
			<NoticeToast />
			<Live />
		</OpsProvider>,
	);
	await act(async () => {
		await Promise.resolve();
	});
}

const run = (work: () => void) => act(async () => work());
const submit = async (fake: FakeOpsClient, origin = 'main-1') => {
	let id = 0;
	await act(async () => {
		id = await fake.submit({ ...request(['a.txt', 'b.txt']), originWindow: origin });
	});
	await run(() => fake.start(id));
	return id;
};
const conflicts = () => [conflictFor('a.txt'), conflictFor('b.txt')];
const ring = () => document.querySelector<HTMLButtonElement>('[data-ops-ring]')!;
const callsOf = (fake: FakeOpsClient, name: string) =>
	fake.calls.filter((call) => call[0] === name);

describe('conflicts', () => {
	it('opens by itself in the window that started the job', async () => {
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		expect(screen.queryByRole('dialog')).toBeNull();
		await run(() => fake.askConflicts(id, conflicts()));
		expect(
			await screen.findByRole('dialog', { name: '2 items already exist in /dest' }),
		).toBeInTheDocument();
	});

	it('asks the client to compare each clash of the job it is answering', async () => {
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		const clashes = conflicts();
		for (const clash of clashes) {
			fake.previews.set(clash.source.uri, {
				existing: { location: clash.existing, size: 1024, modifiedMs: null },
				incoming: { location: clash.source, size: 2048, modifiedMs: null },
				kind: { type: 'identical' },
			});
		}
		await run(() => fake.askConflicts(id, clashes));
		expect(await screen.findAllByText(/Identical contents/)).toHaveLength(2);
		expect(callsOf(fake, 'conflictPreview').map((call) => [call[0], call[1]])).toEqual([
			['conflictPreview', id],
			['conflictPreview', id],
		]);
	});

	it('does not open in a window that did not start the job, but "Resolve…" does', async () => {
		const user = userEvent.setup();
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake, 'ops');
		const id = await submit(fake, 'main-1');
		await run(() => fake.askConflicts(id, conflicts()));
		await waitFor(() => expect(ring()).toHaveAttribute('data-urgency', 'waiting'));
		expect(screen.queryByRole('dialog')).toBeNull();
		await user.click(screen.getByRole('button', { name: /^Resolve….*Copying 2 items/ }));
		expect(
			await screen.findByRole('dialog', { name: /2 items already exist/ }),
		).toBeInTheDocument();
	});

	it('answers through the client, closes, announces, and the ring stops asking', async () => {
		const user = userEvent.setup();
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		await run(() => fake.askConflicts(id, conflicts()));
		await waitFor(() => expect(ring()).toHaveAttribute('data-urgency', 'waiting'));
		const dialog = await screen.findByRole('dialog');
		await user.selectOptions(
			within(dialog).getByRole('combobox', { name: 'Choice for a.txt' }),
			'Skip',
		);
		await user.selectOptions(
			within(dialog).getByRole('combobox', { name: 'Choice for b.txt' }),
			'Keep both',
		);
		await user.click(within(dialog).getByRole('button', { name: 'Continue' }));
		const [call] = callsOf(fake, 'resolve');
		expect(call).toEqual([
			'resolve',
			id,
			[
				{ source: conflicts()[0]!.source, policy: 'skip' },
				{ source: conflicts()[1]!.source, policy: 'keepBoth' },
			],
			undefined,
		]);
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(screen.getByTestId('live')).toHaveTextContent('Continuing: Skip 1, Keep both 1');
		await waitFor(() => expect(ring()).not.toHaveAttribute('data-urgency', 'waiting'));
	});

	it('sends the bulk choice as the job’s own when asked to apply it to all', async () => {
		const user = userEvent.setup();
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		await run(() => fake.askConflicts(id, conflicts()));
		const dialog = await screen.findByRole('dialog');
		await user.selectOptions(
			within(dialog).getByRole('combobox', { name: 'Apply to all remaining' }),
			'Skip',
		);
		await user.click(within(dialog).getByRole('checkbox', { name: /Apply to all conflicts/ }));
		await user.click(within(dialog).getByRole('button', { name: 'Continue' }));
		expect(callsOf(fake, 'resolve')[0]).toEqual(['resolve', id, [], 'skip']);
	});

	it('cancels the job from "Cancel the operation"', async () => {
		const user = userEvent.setup();
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		await run(() => fake.askConflicts(id, conflicts()));
		const dialog = await screen.findByRole('dialog');
		await user.click(within(dialog).getByRole('button', { name: 'Cancel the operation' }));
		expect(callsOf(fake, 'cancel')).toEqual([['cancel', id]]);
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
	});

	it('closes by itself when another window answered first', async () => {
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		await run(() => fake.askConflicts(id, conflicts()));
		await screen.findByRole('dialog');
		await act(async () => {
			await fake.resolve(id, [], 'skip');
		});
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(callsOf(fake, 'cancel')).toHaveLength(0);
	});

	it('leaves the job waiting on "Decide later" and opens again from "Resolve…"', async () => {
		const user = userEvent.setup();
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		await run(() => fake.askConflicts(id, conflicts()));
		const dialog = await screen.findByRole('dialog');
		await user.click(within(dialog).getByRole('button', { name: 'Decide later' }));
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(ring()).toHaveAttribute('data-urgency', 'waiting');
		await user.click(screen.getByRole('button', { name: /^Resolve…/ }));
		expect(await screen.findByRole('dialog')).toBeInTheDocument();
	});

	it('brings a parked question back when a notification\u2019s "Show" asks for it', async () => {
		const user = userEvent.setup();
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		await run(() => fake.askConflicts(id, conflicts()));
		const dialog = await screen.findByRole('dialog');
		await user.click(within(dialog).getByRole('button', { name: 'Decide later' }));
		expect(screen.queryByRole('dialog')).toBeNull();
		// Another job's id does nothing.
		await run(() => fake.emitShowJob(id + 100));
		expect(screen.queryByRole('dialog')).toBeNull();
		await run(() => fake.emitShowJob(id));
		expect(await screen.findByRole('dialog')).toBeInTheDocument();
	});

	it('ignores "Show" for a job that is not waiting', async () => {
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		await run(() => fake.emitShowJob(id));
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('gives focus to the operations button when the dialog closes with nothing to return to', async () => {
		const user = userEvent.setup();
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		expect(document.body).toHaveFocus();
		await run(() => fake.askConflicts(id, conflicts()));
		const dialog = await screen.findByRole('dialog');
		await user.click(within(dialog).getByRole('button', { name: 'Decide later' }));
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(document.body).not.toHaveFocus();
		expect(document.querySelector('[data-ops-ring]')).toHaveFocus();
	});

	it('opens one dialog at a time, the next job after the first is answered', async () => {
		const user = userEvent.setup();
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const first = await submit(fake);
		const second = await submit(fake);
		await run(() => fake.askConflicts(first, [conflictFor('a.txt')]));
		await run(() => fake.askConflicts(second, [conflictFor('z.txt')]));
		expect(await screen.findAllByRole('dialog')).toHaveLength(1);
		expect(screen.getByRole('combobox', { name: 'Choice for a.txt' })).toBeInTheDocument();
		await user.selectOptions(screen.getByRole('combobox', { name: 'Choice for a.txt' }), 'Skip');
		await user.click(screen.getByRole('button', { name: 'Continue' }));
		expect(await screen.findByRole('combobox', { name: 'Choice for z.txt' })).toBeInTheDocument();
		expect(screen.getAllByRole('dialog')).toHaveLength(1);
	});

	it('asks again, with the answers cleared, when the job meets a new clash', async () => {
		const user = userEvent.setup();
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		await run(() => fake.askConflicts(id, [conflictFor('a.txt')]));
		await user.selectOptions(
			await screen.findByRole('combobox', { name: 'Choice for a.txt' }),
			'Skip',
		);
		await user.click(screen.getByRole('button', { name: 'Continue' }));
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		await run(() => fake.askConflicts(id, [conflictFor('deep.txt')]));
		const next = await screen.findByRole('combobox', { name: 'Choice for deep.txt' });
		expect(next).toHaveValue('');
	});
});

describe('errors', () => {
	const failure = {
		kind: 'permissionDenied',
		location: { display: '/dest/a.txt', uri: 'file:///dest/a.txt' },
	} as const;

	it('opens the error dialog for the window’s own job and sends each decision', async () => {
		const user = userEvent.setup();
		for (const [label, decision] of [
			['Retry', 'retry'],
			['Skip', 'skip'],
			['Skip all like this', 'skipAll'],
			['Cancel the operation', 'cancel'],
		] as const) {
			const fake = createFakeOpsClient({ concurrency: 4 });
			await mount(fake);
			const id = await submit(fake);
			await run(() => fake.askError(id, failure, failure.location));
			const dialog = await screen.findByRole('dialog', { name: 'An item could not be processed' });
			await user.click(within(dialog).getByRole('button', { name: label }));
			expect(callsOf(fake, 'resolveError')).toEqual([['resolveError', id, decision]]);
			await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
			cleanup();
		}
	});

	it('is answered from "Resolve…" in another window', async () => {
		const user = userEvent.setup();
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake, 'ops');
		const id = await submit(fake, 'main-1');
		await run(() => fake.askError(id, failure, failure.location));
		expect(screen.queryByRole('dialog')).toBeNull();
		await user.click(screen.getByRole('button', { name: /^Resolve…/ }));
		await user.click(
			within(await screen.findByRole('dialog')).getByRole('button', { name: 'Retry' }),
		);
		expect(callsOf(fake, 'resolveError')).toHaveLength(1);
	});

	it('closes when the job leaves the wait some other way', async () => {
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		await run(() => fake.askError(id, failure, failure.location));
		await screen.findByRole('dialog');
		await act(async () => {
			await fake.resolveError(id, 'skip');
		});
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
	});

	it('closes on Escape without answering, and "Resolve…" brings it back', async () => {
		const user = userEvent.setup();
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		await run(() => fake.askError(id, failure, failure.location));
		await screen.findByRole('dialog');
		await user.keyboard('{Escape}');
		expect(screen.queryByRole('dialog')).toBeNull();
		expect(callsOf(fake, 'resolveError')).toHaveLength(0);
		await user.click(screen.getByRole('button', { name: /^Resolve…/ }));
		expect(await screen.findByRole('dialog')).toBeInTheDocument();
	});
});

describe('the resolve hook', () => {
	it('registers one resolver for the window and gives it up on unmount', async () => {
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake, 'main-1');
		await run(() => fake.askConflicts(id, conflicts()));
		await run(() => {});
		cleanup();
		render(<NoticeToast />);
		requestResolve(id);
		expect(
			await screen.findByText('Answering from here is not available yet.'),
		).toBeInTheDocument();
	});

	it('says so when the job is not waiting any more', async () => {
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		await run(() => requestResolve(id));
		expect(
			await screen.findByText('That job is no longer waiting for an answer.'),
		).toBeInTheDocument();
	});

	it('is replaced by a later registration like any other resolver', () => {
		const seen: number[] = [];
		const stop = registerResolver((job) => seen.push(job));
		requestResolve(4);
		expect(seen).toEqual([4]);
		stop();
	});
});

describe('when the plugin refuses the answer', () => {
	it('says so and asks again', async () => {
		const user = userEvent.setup();
		const fake = createFakeOpsClient({ concurrency: 4 });
		await mount(fake);
		const id = await submit(fake);
		await run(() => fake.askConflicts(id, [conflictFor('a.txt')]));
		const original = fake.resolve.bind(fake);
		fake.resolve = async () => {
			throw new Error('the job is busy');
		};
		await user.selectOptions(
			await screen.findByRole('combobox', { name: 'Choice for a.txt' }),
			'Skip',
		);
		await user.click(screen.getByRole('button', { name: 'Continue' }));
		expect(
			await screen.findByText('Could not send the answer: the job is busy'),
		).toBeInTheDocument();
		expect(await screen.findByRole('dialog')).toBeInTheDocument();
		fake.resolve = original;
	});
});
