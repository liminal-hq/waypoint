// Verifies the error dialog: the message and place, the decisions each error offers, focus, Escape and the destination hook
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ArchiveClientProvider } from '../archives/ArchiveContext';
import { archiveTopUri } from '../archives/archiveNames';
import { FakeArchiveClient } from '../archives/fakeArchiveClient';
import { ConnectionsProvider } from '../connections/ConnectionsContext';
import { connectStore } from '../connections/connectStore';
import { FakeConnectionsClient, serverLocation } from '../connections/fakeConnectionsClient';
import { fileLocation } from '../services/fakeVfsClient';
import { fakeJobSnapshot } from '../trash/fakeTrashClient';
import { OperationErrorDialog, partialText } from './OperationErrorDialog';

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
		expect(dialog()).toHaveTextContent(/Permission denied for \/src\/big\.iso/);
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

describe('a server that wants a login', () => {
	const at = serverLocation('sftp://me@nas.lan', '/up/big.iso');

	function mountWithConnections(error: OpsError) {
		const onDecide = vi.fn();
		const client = new FakeConnectionsClient({});
		render(
			<ConnectionsProvider client={client}>
				<OperationErrorDialog
					job={{
						...fakeJobSnapshot(3, { kind: 'copy' }, { state: 'queued' }),
						sources: { count: 1, first: 'big.iso' },
					}}
					error={error}
					item={item}
					onDecide={onDecide}
					onLater={vi.fn()}
				/>
			</ConnectionsProvider>,
		);
		return { onDecide, client };
	}

	it('offers Sign In first, asks, connects and then retries the item', async () => {
		const { onDecide, client } = mountWithConnections({
			kind: 'connection',
			error: { kind: 'authRequired', location: at, prompt: { kind: 'password', user: 'me' } },
		});
		// (The window's connections start in an effect, so the button joins once they have.)
		const signIn = await screen.findByRole('button', { name: 'Sign In…' });
		expect(buttons().indexOf('Sign In…')).toBeLessThan(buttons().indexOf('Retry'));
		await userEvent.click(signIn);
		await waitFor(() => expect(connectStore.getState().question).not.toBeNull());
		act(() =>
			connectStore.getState().answer({
				answer: { kind: 'password', user: 'me', password: 'secret' },
				remember: false,
			}),
		);
		await waitFor(() => expect(onDecide).toHaveBeenCalledWith('retry'));
		expect(client.calls.some((call) => call.method === 'connect')).toBe(true);
	});

	it('offers only Retry for a connection that was merely lost', () => {
		mountWithConnections({
			kind: 'connection',
			error: { kind: 'disconnected', location: at },
		});
		expect(screen.queryByRole('button', { name: 'Sign In…' })).toBeNull();
		expect(buttons()).toContain('Retry');
	});
});

describe('an encrypted archive', () => {
	const zip = fileLocation('/home/me/enc.zip');

	function mountLocked(kind: 'authRequired' | 'authFailed', archives: FakeArchiveClient) {
		const onDecide = vi.fn();
		render(
			<ArchiveClientProvider client={archives}>
				<OperationErrorDialog
					job={{
						...fakeJobSnapshot(3, { kind: 'copy' }, { state: 'queued' }),
						sources: { count: 1, first: 'enc.zip' },
					}}
					error={{
						kind: 'connection',
						error: {
							kind,
							location: { display: 'enc.zip', uri: archiveTopUri(zip) },
							prompt: { kind: 'passphrase', subject: 'enc.zip' },
						},
					}}
					item={item}
					onDecide={onDecide}
					onLater={vi.fn()}
				/>
			</ArchiveClientProvider>,
		);
		return onDecide;
	}

	it('speaks of the archive and its password, not of a server', () => {
		mountLocked('authRequired', new FakeArchiveClient());
		expect(dialog()).toHaveTextContent('enc.zip is encrypted and needs its password');
		expect(dialog()).not.toHaveTextContent(/server|Not connected/);
		expect(buttons()).toContain('Enter password');
		expect(buttons()).not.toContain('Sign In…');
	});

	it('gives the password to the archive provider and retries the item', async () => {
		const archives = new FakeArchiveClient();
		const onDecide = mountLocked('authRequired', archives);
		await userEvent.click(screen.getByRole('button', { name: 'Enter password' }));
		await waitFor(() => expect(connectStore.getState().question).not.toBeNull());
		act(() =>
			connectStore.getState().answer({
				answer: { kind: 'passphrase', passphrase: 'secret' },
				remember: false,
			}),
		);
		await waitFor(() => expect(onDecide).toHaveBeenCalledWith('retry'));
		expect(archives.unlocked.map((given) => given.passphrase)).toEqual(['secret']);
	});
});

describe('what Retry does with a file stopped part way', () => {
	const item = fileLocation('/src/big.iso');
	it('says it continues, with what was sent when the server said', () => {
		expect(partialText({ item, resumes: true, kept: 3_200_000 })).toBe(
			'Retry continues big.iso from where it stopped (3.2 MB already sent).',
		);
		expect(partialText({ item, resumes: true, kept: null })).toBe(
			'Retry continues big.iso from where it stopped.',
		);
		expect(partialText({ item, resumes: false, kept: null })).toBe(
			'Retry starts big.iso again: this server cannot continue a file part way.',
		);
	});
});
