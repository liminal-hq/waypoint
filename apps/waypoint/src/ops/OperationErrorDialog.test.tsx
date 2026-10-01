// Verifies the error dialog: the message and place, the decisions each error offers, focus, Escape and the destination hook
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import { cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { fileLocation } from '../services/fakeVfsClient';
import { fakeJobSnapshot } from '../trash/fakeTrashClient';
import { OperationErrorDialog } from './OperationErrorDialog';

afterEach(cleanup);

const item = fileLocation('/src/big.iso');

function mount(error: OpsError, extra: { onChooseLocation?: () => void } = {}) {
	const onDecide = vi.fn();
	const onLater = vi.fn();
	render(
		<OperationErrorDialog
			job={{
				...fakeJobSnapshot(3, { kind: 'copy' }, { state: 'queued' }),
				sources: { count: 1, first: 'big.iso' },
			}}
			error={error}
			item={item}
			onDecide={onDecide}
			onLater={onLater}
			{...extra}
		/>,
	);
	return { onDecide, onLater };
}

const dialog = () => screen.getByRole('dialog');
const buttons = () =>
	within(dialog())
		.getAllByRole('button')
		.map((b) => b.textContent);

describe('what it says', () => {
	it('says what went wrong, which item and which job', () => {
		mount({ kind: 'permissionDenied', location: item });
		expect(
			screen.getByRole('dialog', { name: 'An item could not be processed' }),
		).toBeInTheDocument();
		expect(dialog()).toHaveTextContent(/not allowed to change \/src\/big\.iso/);
		expect(dialog()).toHaveTextContent('Item: /src/big.iso');
		expect(dialog()).toHaveTextContent('Job: Copying big.iso');
	});

	it('gives the sizes when space runs out', () => {
		mount({ kind: 'notEnoughSpace', needed: 4_700_000_000, free: 120_000_000 });
		expect(dialog()).toHaveTextContent('4.7 GB needed, 120 MB free');
	});

	it('keeps checksums behind a disclosure', async () => {
		mount({ kind: 'verifyFailed', location: item, expected: 'abc123', actual: 'def456' });
		const details = dialog().querySelector('details')!;
		expect(details).not.toHaveAttribute('open');
		expect(within(details).getByText('Expected checksum: abc123')).toBeInTheDocument();
		expect(within(dialog()).getByText('Details')).toBeInTheDocument();
	});

	it('has no disclosure for errors without details', () => {
		mount({ kind: 'io', message: 'broken pipe' });
		expect(dialog().querySelector('details')).toBeNull();
		expect(dialog()).toHaveTextContent('broken pipe');
	});

	it('titles a missing original folder for what it is', () => {
		mount({ kind: 'originMissingParent', location: fileLocation('/home/a/docs') });
		expect(screen.getByRole('dialog', { name: 'The original folder is gone' })).toBeInTheDocument();
	});
});

describe('what it offers', () => {
	it('offers Retry, Skip, Skip all like this and Cancel, and calls the decision pressed', async () => {
		const user = userEvent.setup();
		const { onDecide } = mount({ kind: 'io', message: 'x' });
		expect(buttons()).toEqual([
			'Decide later',
			'Retry',
			'Skip',
			'Skip all like this',
			'Cancel the operation',
		]);
		for (const [label, decision] of [
			['Retry', 'retry'],
			['Skip', 'skip'],
			['Skip all like this', 'skipAll'],
			['Cancel the operation', 'cancel'],
		] as const) {
			await user.click(within(dialog()).getByRole('button', { name: label }));
			expect(onDecide).toHaveBeenLastCalledWith(decision);
		}
	});

	it('adds Recreate folders for a missing original folder, and calls createParents', async () => {
		const user = userEvent.setup();
		const { onDecide } = mount({ kind: 'originMissingParent', location: fileLocation('/a') });
		await user.click(within(dialog()).getByRole('button', { name: 'Recreate folders' }));
		expect(onDecide).toHaveBeenCalledWith('createParents');
	});

	it('does not offer Recreate folders for other errors', () => {
		mount({ kind: 'notFound', location: item });
		expect(within(dialog()).queryByRole('button', { name: 'Recreate folders' })).toBeNull();
	});

	it('leaves "Choose another location…" out where there is no destination picker', () => {
		mount({ kind: 'notEnoughSpace', needed: 10, free: 1 });
		expect(within(dialog()).queryByRole('button', { name: /Choose another location/ })).toBeNull();
	});

	it('offers it for a full volume when a picker hook is given, and for no other error', async () => {
		const user = userEvent.setup();
		const onChooseLocation = vi.fn();
		mount({ kind: 'notEnoughSpace', needed: 10, free: 1 }, { onChooseLocation });
		await user.click(within(dialog()).getByRole('button', { name: 'Choose another location…' }));
		expect(onChooseLocation).toHaveBeenCalledOnce();
		cleanup();
		mount({ kind: 'io', message: 'x' }, { onChooseLocation });
		expect(within(dialog()).queryByRole('button', { name: /Choose another location/ })).toBeNull();
	});
});

describe('focus and Escape', () => {
	it('starts on Retry, never on Skip all like this', () => {
		mount({ kind: 'io', message: 'x' });
		expect(within(dialog()).getByRole('button', { name: 'Retry' })).toHaveFocus();
		expect(within(dialog()).getByRole('button', { name: 'Skip all like this' })).not.toHaveFocus();
	});

	it('starts on Recreate folders when only the folder is missing', () => {
		mount({ kind: 'originMissingParent', location: fileLocation('/a') });
		expect(within(dialog()).getByRole('button', { name: 'Recreate folders' })).toHaveFocus();
	});

	it('closes without answering on Escape, leaving the job waiting', async () => {
		const user = userEvent.setup();
		const { onDecide, onLater } = mount({ kind: 'io', message: 'x' });
		await user.keyboard('{Escape}');
		expect(onLater).toHaveBeenCalledOnce();
		expect(onDecide).not.toHaveBeenCalled();
	});

	it('does nothing on a click outside', async () => {
		const user = userEvent.setup();
		const { onDecide, onLater } = mount({ kind: 'io', message: 'x' });
		await user.click(dialog());
		expect(onLater).not.toHaveBeenCalled();
		expect(onDecide).not.toHaveBeenCalled();
	});

	it('explains Skip all like this in words', () => {
		mount({ kind: 'io', message: 'x' });
		expect(dialog()).toHaveTextContent(
			/skips this item and every later item that fails the same way/,
		);
	});
});
