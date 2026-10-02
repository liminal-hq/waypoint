// Verifies the checksum: never automatic, never for a folder, a warning above 1 GiB, progress, cancelling, the digest and its live announcement
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { FakeChecksumClient } from '../services/fakeChecksumClient';
import { fakeDetails } from '../services/fakeDetailsClient';
import { makeEntry } from '../services/fakeVfsClient';
import { ChecksumSection } from './ChecksumSection';
import { LARGE_FILE_BYTES } from './checksumModel';

afterEach(cleanup);

const DIGEST = 'ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad';

function mount(
	overrides: {
		client?: FakeChecksumClient;
		entry?: ReturnType<typeof makeEntry>;
		details?: ReturnType<typeof fakeDetails> | null;
		writeText?: (text: string) => Promise<void>;
	} = {},
) {
	const client = overrides.client ?? new FakeChecksumClient();
	const entry = overrides.entry ?? makeEntry(3, 'notes.txt', { size: 1200 });
	const details =
		overrides.details === undefined
			? fakeDetails({ name: 'notes.txt', size: 1200 })
			: overrides.details;
	const view = render(
		<ChecksumSection
			client={client}
			handle={7}
			entry={entry}
			details={details}
			{...(overrides.writeText ? { writeText: overrides.writeText } : {})}
		/>,
	);
	const again = (next: Pick<NonNullable<Parameters<typeof mount>[0]>, 'entry' | 'details'>) =>
		view.rerender(
			<ChecksumSection
				client={client}
				handle={7}
				entry={next.entry ?? entry}
				details={next.details === undefined ? details : next.details}
			/>,
		);
	return { client, again, ...view };
}

describe('the checksum section', () => {
	it('reads nothing until it is asked, and offers SHA-256 first', async () => {
		const { client } = mount();
		expect(screen.getByRole('heading', { name: 'Checksum' })).toBeInTheDocument();
		expect(screen.getByRole('combobox', { name: 'Algorithm' })).toHaveValue('sha256');
		expect(client.calls).toEqual([]);
		await userEvent.click(screen.getByRole('button', { name: 'Calculate checksum…' }));
		expect(client.calls).toEqual(['7:3:sha256']);
	});

	it('hashes with the algorithm that was chosen', async () => {
		const { client } = mount();
		await userEvent.selectOptions(screen.getByRole('combobox', { name: 'Algorithm' }), 'blake3');
		await userEvent.click(screen.getByRole('button', { name: 'Calculate checksum…' }));
		expect(client.calls).toEqual(['7:3:blake3']);
	});

	it('is not shown for a folder, a link to one, or a special file', () => {
		for (const [kind, resolvesTo] of [
			['directory', null],
			['symlink', 'directory'],
			['other', null],
		] as const) {
			const { container, unmount } = mount({
				entry: makeEntry(4, 'thing', { kind, linkTarget: resolvesTo }),
				details: fakeDetails({ kind, resolvesTo }),
			});
			expect(container).toBeEmptyDOMElement();
			unmount();
		}
	});

	it('is not shown where the window has no checksum service', () => {
		const { container } = render(
			<ChecksumSection
				client={null}
				handle={1}
				entry={makeEntry(3, 'notes.txt')}
				details={fakeDetails()}
			/>,
		);
		expect(container).toBeEmptyDOMElement();
	});

	it('shows progress, and cancelling stops the run and says nothing was calculated', async () => {
		const { client } = mount();
		await userEvent.click(screen.getByRole('button', { name: 'Calculate checksum…' }));
		const job = client.live[0]!;
		act(() => client.advance(job, 600, 1200));
		const bar = await screen.findByRole('progressbar', { name: 'Checksum progress' });
		expect(bar).toHaveAttribute('value', '0.5');
		expect(screen.getByText(/Calculating the SHA-256 checksum/)).toBeInTheDocument();
		await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(await screen.findByText('Cancelled. Nothing was calculated.')).toBeInTheDocument();
		expect(client.live).toEqual([]);
		expect(screen.queryByRole('progressbar')).toBeNull();
	});

	it('shows the digest when it is done and announces only that', async () => {
		const { client } = mount();
		await userEvent.click(screen.getByRole('button', { name: 'Calculate checksum…' }));
		const job = client.live[0]!;
		const live = screen.getByRole('status');
		act(() => client.advance(job, 600, 1200));
		expect(live).toBeEmptyDOMElement();
		act(() => client.finish(job, DIGEST, 1200));
		expect(await screen.findByText(DIGEST)).toBeInTheDocument();
		expect(live).toHaveTextContent('The SHA-256 checksum of notes.txt is ready');
		expect(screen.getByText('SHA-256 checksum')).toBeInTheDocument();
	});

	it('copies the digest', async () => {
		const writeText = vi.fn().mockResolvedValue(undefined);
		const { client } = mount({ writeText });
		await userEvent.click(screen.getByRole('button', { name: 'Calculate checksum…' }));
		act(() => client.finish(client.live[0]!, DIGEST, 1200));
		await userEvent.click(await screen.findByRole('button', { name: 'Copy' }));
		expect(writeText).toHaveBeenCalledWith(DIGEST);
		expect(await screen.findByRole('button', { name: 'Copied' })).toBeInTheDocument();
	});

	it('says why it failed, and can be asked again', async () => {
		const { client } = mount();
		await userEvent.click(screen.getByRole('button', { name: 'Calculate checksum…' }));
		act(() =>
			client.fail(client.live[0]!, {
				kind: 'permissionDenied',
				location: { display: '/x', uri: 'file:///x' },
			}),
		);
		expect(
			await screen.findByText('The checksum could not be calculated: permission was denied'),
		).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'Calculate checksum…' })).toBeEnabled();
	});

	it('says why it could not start', async () => {
		const client = new FakeChecksumClient();
		client.refuseNext({ kind: 'unsupported', what: 'x' });
		mount({ client });
		await userEvent.click(screen.getByRole('button', { name: 'Calculate checksum…' }));
		expect(
			await screen.findByText(
				'The checksum could not be calculated: only files on this computer can be checked',
			),
		).toBeInTheDocument();
	});

	it('forgets a digest when another algorithm is picked', async () => {
		const { client } = mount();
		await userEvent.click(screen.getByRole('button', { name: 'Calculate checksum…' }));
		act(() => client.finish(client.live[0]!, DIGEST, 1200));
		await screen.findByText(DIGEST);
		await userEvent.selectOptions(screen.getByRole('combobox', { name: 'Algorithm' }), 'blake3');
		expect(screen.queryByText(DIGEST)).toBeNull();
	});

	it('cancels a run, and forgets its result, when the file changes under it', async () => {
		const { client, again } = mount();
		await userEvent.click(screen.getByRole('button', { name: 'Calculate checksum…' }));
		const job = client.live[0]!;
		again({ entry: makeEntry(3, 'notes.txt', { size: 1300, modifiedMs: 5 }) });
		expect(client.live).not.toContain(job);
		expect(screen.queryByRole('progressbar')).toBeNull();
		expect(screen.getByRole('button', { name: 'Calculate checksum…' })).toBeInTheDocument();
	});

	it('cancels a run when the window closes', async () => {
		const { client, unmount } = mount();
		await userEvent.click(screen.getByRole('button', { name: 'Calculate checksum…' }));
		expect(client.live).toHaveLength(1);
		unmount();
		expect(client.live).toEqual([]);
	});

	describe('a file above 1 GiB', () => {
		const big = () =>
			mount({
				entry: makeEntry(3, 'disk.img', { size: LARGE_FILE_BYTES + 1 }),
				details: fakeDetails({ name: 'disk.img', size: LARGE_FILE_BYTES + 1 }),
			});

		it('asks first, and nothing is read until it is confirmed', async () => {
			const { client } = big();
			await userEvent.click(screen.getByRole('button', { name: 'Calculate checksum…' }));
			expect(screen.getByText(/disk\.img is 1\.1 GB/)).toBeInTheDocument();
			expect(client.calls).toEqual([]);
			await userEvent.click(screen.getByRole('button', { name: 'Calculate anyway' }));
			expect(client.calls).toEqual(['7:3:sha256']);
		});

		it('drops the warning when it is declined', async () => {
			const { client } = big();
			await userEvent.click(screen.getByRole('button', { name: 'Calculate checksum…' }));
			await userEvent.click(screen.getByRole('button', { name: 'Cancel' }));
			expect(screen.queryByText(/disk\.img is/)).toBeNull();
			expect(client.calls).toEqual([]);
		});
	});

	it('does not warn at 1 GiB or below', async () => {
		const { client } = mount({
			entry: makeEntry(3, 'a.bin', { size: LARGE_FILE_BYTES }),
			details: fakeDetails({ size: LARGE_FILE_BYTES }),
		});
		await userEvent.click(screen.getByRole('button', { name: 'Calculate checksum…' }));
		expect(client.calls).toEqual(['7:3:sha256']);
	});
});
