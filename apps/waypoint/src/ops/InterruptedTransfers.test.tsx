// Verifies the Operations list's interrupted transfers: each with Resume and Discard…, and Discard asks first
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ResumableRecord } from '@liminal-hq/waypoint-protocol/generated/ResumableRecord';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { request } from '../test/opsHarness';
import type { ConfirmSpec } from './fileCommands';
import { InterruptedTransfers } from './InterruptedTransfers';

afterEach(cleanup);

const record = (job: number, name: string): ResumableRecord => ({
	job,
	label: `Copying ${name}`,
	atMs: job * 10,
	request: request([name]),
	points: [
		{
			source: { display: `/home/${name}`, uri: `file:///home/${name}` },
			target: { display: `sftp://nas/up/${name}`, uri: `sftp://nas/up/${name}` },
			partial: { display: 'p', uri: `sftp://nas/up/.waypoint-partial-${job}-1-${name}` },
			sourceSize: 100,
			sourceModifiedMs: 1,
			offset: 40,
		},
	],
});

describe('interrupted transfers in the Operations list', () => {
	it('shows nothing when there are none', async () => {
		const fake = createFakeOpsClient();
		const { container } = render(
			<InterruptedTransfers client={fake} confirm={async () => true} revision="" />,
		);
		await act(async () => {});
		expect(container).toBeEmptyDOMElement();
	});

	it('lists each with Resume and Discard…; Discard asks, and only then removes', async () => {
		const fake = createFakeOpsClient();
		fake.setInterrupted([record(1, 'a'), record(2, 'b')]);
		const asked: ConfirmSpec[] = [];
		let answer = false;
		const confirm = vi.fn(async (spec: ConfirmSpec) => {
			asked.push(spec);
			return answer;
		});
		render(<InterruptedTransfers client={fake} confirm={confirm} revision="" />);
		const section = await screen.findByRole('region', { name: 'Interrupted transfers' });
		const rows = within(section).getAllByRole('listitem');
		expect(rows).toHaveLength(2);
		expect(rows[0]).toHaveTextContent('1 file was partly sent');
		const discardB = within(section).getByRole('button', { name: 'Discard…: Copying b' });
		await userEvent.click(discardB);
		expect(asked[0]?.items).toEqual(['sftp://nas/up/b']);
		expect(within(section).getAllByRole('listitem')).toHaveLength(2);
		answer = true;
		await userEvent.click(discardB);
		await waitFor(() => expect(within(section).getAllByRole('listitem')).toHaveLength(1));
		await userEvent.click(within(section).getByRole('button', { name: 'Resume: Copying a' }));
		await waitFor(() =>
			expect(screen.queryByRole('region', { name: 'Interrupted transfers' })).toBeNull(),
		);
		expect(fake.jobs()).toHaveLength(1);
	});
});
