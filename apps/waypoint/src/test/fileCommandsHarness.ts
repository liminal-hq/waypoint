// Shared by the file command tests: a real listing session over a fake folder, a fake queue and the commands over both
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { expect, vi } from 'vitest';
import { openListingModel } from '../browse/listingModel';
import { createListingSession, type ListingSession } from '../browse/useListingSession';
import { createFileCommands, type ConfirmSpec, type FileCommands } from '../ops/fileCommands';
import { createOpsStore, type OpsHandle } from '../ops/opsStore';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { FakeVfsClient, makeEntry } from '../services/fakeVfsClient';
import type { Location } from '../services/opsClient';
import { FOLDER } from './browseHarness';

export interface CommandsHarness {
	vfs: FakeVfsClient;
	fake: FakeOpsClient;
	ops: OpsHandle;
	session: ListingSession;
	commands: FileCommands;
	/** Every confirmation asked, in order. */
	confirms: ConfirmSpec[];
	/** Every message said, in order. */
	said: string[];
	/** What the next confirmation answers. */
	answer: { value: boolean };
	/** Runs when a confirmation is asked, before it answers: what happens while the dialog is open. */
	onConfirm: { run: (() => void) | null };
	/** Waits for the `n`th job and runs it to the end, as the worker pool would. */
	finish(n?: number, undoable?: string): Promise<number>;
}

export const NAMES = ['alpha.txt', 'beta.jpg', 'gamma'];

/** A folder of `entries` (three by default: two files and a folder) opened as a listing, with commands over a fake queue. */
export async function commandsHarness(
	options: {
		entries?: Entry[];
		readOnly?: boolean;
		/** The folder to open; an archive's folder to test the changes inside one. */
		folder?: Location;
		/** The folder is read only as a folder but its changes rewrite its file (an archive that can be written). */
		rewritable?: boolean;
	} = {},
): Promise<CommandsHarness> {
	const vfs = new FakeVfsClient();
	const folder = options.folder ?? FOLDER;
	vfs.setFolder(
		folder,
		options.entries ?? [
			makeEntry(1, 'alpha.txt'),
			makeEntry(2, 'beta.jpg'),
			makeEntry(3, 'gamma', { kind: 'directory' }),
		],
	);
	if (options.readOnly || options.rewritable) vfs.setReadOnly(folder);
	if (options.rewritable) vfs.setRewritable(folder);
	const session = createListingSession(await openListingModel(vfs, folder));
	const fake = createFakeOpsClient();
	const ops = createOpsStore(fake);
	await ops.ready;
	const confirms: ConfirmSpec[] = [];
	const said: string[] = [];
	const answer = { value: true };
	const onConfirm: { run: (() => void) | null } = { run: null };
	const commands = createFileCommands({
		ops,
		vfs,
		windowLabel: 'main-1',
		activeSession: () => session,
		confirm: async (spec) => {
			confirms.push(spec);
			onConfirm.run?.();
			return answer.value;
		},
		say: (text) => said.push(text),
	});
	return {
		vfs,
		fake,
		ops,
		session,
		commands,
		confirms,
		said,
		answer,
		onConfirm,
		async finish(n = 1, undoable) {
			await vi.waitFor(() => expect(fake.jobs().length).toBeGreaterThanOrEqual(n));
			const job = fake.jobs()[n - 1]!;
			fake.start(job.id);
			fake.done(job.id, undoable);
			return job.id;
		},
	};
}

/** Selects the entries at `positions` of the harness's listing, as a click and Ctrl-clicks would. */
export async function select(h: CommandsHarness, ...positions: number[]): Promise<void> {
	const { model, store } = h.session;
	await model.readRange(0, model.count);
	positions.forEach((position, index) => {
		const entry = model.entryAt(position)!;
		if (index === 0) store.getState().click(position, entry.id);
		else store.getState().toggleAt(position, entry.id);
	});
}
