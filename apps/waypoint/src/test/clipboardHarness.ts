// Shared by the clipboard and pane operation tests: two panes over fake folders, a fake queue, a fake system clipboard and the commands over all of them
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import { expect, vi } from 'vitest';
import { openListingModel } from '../browse/listingModel';
import { createListingSession, type ListingSession } from '../browse/useListingSession';
import { createClipboardService, type ClipboardService } from '../ops/clipboardService';
import type { DestinationOptions } from '../ops/destinationStore';
import { createFileCommands, type FileCommands } from '../ops/fileCommands';
import { createOpsStore, type OpsHandle } from '../ops/opsStore';
import { createFakeOpsClient, type FakeOpsClient } from '../services/fakeOpsClient';
import { FakeOsClipboardClient } from '../services/fakeOsClipboardClient';
import { FakeVfsClient, fileLocation, makeEntry, pathOf } from '../services/fakeVfsClient';
import { FOLDER } from './browseHarness';

/** The second pane's folder. */
export const OTHER: Location = fileLocation('/home/other');

export interface ClipboardHarness {
	vfs: FakeVfsClient;
	fake: FakeOpsClient;
	ops: OpsHandle;
	os: FakeOsClipboardClient;
	clipboard: ClipboardService;
	/** The pane the keys act on. */
	session: ListingSession;
	/** The pane beside it, or `null` when the window shows no pair. */
	other: { session: ListingSession | null };
	commands: FileCommands;
	/** Every message said, in order. */
	said: string[];
	/** Every question the destination dialog was asked, in order. */
	asked: DestinationOptions[];
	/** What the destination dialog answers next (`null` is a cancel). */
	pick: { value: Location | null };
	/** Waits for the `n`th job and runs it to the end. */
	finish(n?: number, undoable?: string): Promise<number>;
	/** The last request submitted. */
	lastRequest(): Record<string, unknown>;
}

export interface ClipboardHarnessOptions {
	entries?: Entry[];
	otherEntries?: Entry[];
	/** Show a second pane beside the first. */
	paired?: boolean;
	readOnly?: boolean;
	otherReadOnly?: boolean;
	os?: ConstructorParameters<typeof FakeOsClipboardClient>[0] | null;
}

export const FILE_ENTRIES = [
	makeEntry(1, 'alpha.txt'),
	makeEntry(2, 'beta.jpg'),
	makeEntry(3, 'gamma', { kind: 'directory' }),
];

/** The location of `name` in `folder`, as Rust would hand it out. */
export function child(folder: Location, name: string): Location {
	return fileLocation(`${pathOf(folder)}/${name}`);
}

export async function clipboardHarness(
	options: ClipboardHarnessOptions = {},
): Promise<ClipboardHarness> {
	const vfs = new FakeVfsClient();
	const entries = options.entries ?? FILE_ENTRIES;
	const otherEntries = options.otherEntries ?? [makeEntry(11, 'omega.txt')];
	vfs.setFolder(FOLDER, entries);
	vfs.setFolder(OTHER, otherEntries);
	if (options.readOnly) vfs.setReadOnly(FOLDER);
	if (options.otherReadOnly) vfs.setReadOnly(OTHER);
	const session = createListingSession(await openListingModel(vfs, FOLDER));
	const otherSession = options.paired
		? createListingSession(await openListingModel(vfs, OTHER))
		: null;
	const other = { session: otherSession };
	// What Rust's resolver does: the entries of the listing a handle names, by the selection.
	const listings = new Map<number, { folder: Location; entries: Entry[] }>([
		[session.model.handle, { folder: FOLDER, entries }],
	]);
	if (otherSession) {
		listings.set(otherSession.model.handle, { folder: OTHER, entries: otherEntries });
	}
	const fake = createFakeOpsClient({
		resolveSelection: (handle, spec) => {
			const listing = listings.get(handle);
			return (listing?.entries ?? [])
				.filter((entry) => spec.ids.includes(entry.id) === (spec.kind === 'some'))
				.map((entry) => child(listing!.folder, entry.name));
		},
	});
	const ops = createOpsStore(fake);
	await ops.ready;
	const os = new FakeOsClipboardClient(options.os ?? {});
	const clipboard = createClipboardService({
		client: fake,
		vfs,
		os: options.os === null ? null : os,
		focusTarget: null,
	});
	await clipboard.ready;
	const said: string[] = [];
	const asked: DestinationOptions[] = [];
	const pick: { value: Location | null } = { value: null };
	const commands = createFileCommands({
		ops,
		vfs,
		windowLabel: 'main-1',
		activeSession: () => session,
		confirm: async () => true,
		say: (text) => said.push(text),
		clipboard,
		otherPane: (from) => (from === session ? other.session : null),
		pickDestination: async (request) => {
			asked.push(request);
			return pick.value;
		},
	});
	return {
		vfs,
		fake,
		ops,
		os,
		clipboard,
		session,
		other,
		commands,
		said,
		asked,
		pick,
		async finish(n = 1, undoable) {
			await vi.waitFor(() => expect(fake.jobs().length).toBeGreaterThanOrEqual(n));
			const job = fake.jobs()[n - 1]!;
			fake.start(job.id);
			fake.done(job.id, undoable);
			return job.id;
		},
		lastRequest() {
			const call = [...fake.calls].reverse().find((c) => c[0] === 'submit');
			return call![1] as Record<string, unknown>;
		},
	};
}

/** Selects the entries at `positions` of `session`'s listing, as a click and Ctrl-clicks would. */
export async function selectIn(session: ListingSession, ...positions: number[]): Promise<void> {
	const { model, store } = session;
	await model.readRange(0, model.count);
	positions.forEach((position, index) => {
		const entry = model.entryAt(position)!;
		if (index === 0) store.getState().click(position, entry.id);
		else store.getState().toggleAt(position, entry.id);
	});
}
