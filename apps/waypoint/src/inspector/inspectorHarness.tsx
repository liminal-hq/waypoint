// Shared setup for the Inspector's tests: a details client that answers for any listing, and a panel mounted over a real listing
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { render } from '@testing-library/react';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import { openListingModel } from '../browse/listingModel';
import { createListingSession, type ListingSession } from '../browse/useListingSession';
import { VfsClientProvider } from '../browse/VfsClientContext';
import { createFakeOpenWithClient } from '../openWith/fakeOpenWithClient';
import type { OpenWithClient } from '../openWith/openWithClient';
import { OpenWithProvider } from '../openWith/OpenWithContext';
import { FakeDetailsClient, type FakeEntry } from '../services/fakeDetailsClient';
import type { FolderSizeEvent } from '@liminal-hq/waypoint-protocol/generated/FolderSizeEvent';
import type { FolderSizeJob } from '../services/detailsClient';
import type { FakeVfsClient } from '../services/fakeVfsClient';
import { createTree, HOME } from '../test/workspaceHarness';
import { DetailsClientProvider } from './DetailsClientContext';
import { InspectorPanel } from './InspectorPanel';
import {
	createInspectorStore,
	InspectorStoreContext,
	type InspectorState,
	type InspectorStore,
} from './inspectorStore';

/**
 * A `FakeDetailsClient` that knows its entries by id alone, so a test can describe them before it
 * knows the listing's handle, and that records the folder-size jobs the panel cancelled.
 */
export class LazyDetails extends FakeDetailsClient {
	readonly cancelled: number[] = [];
	readonly started: number[] = [];
	private readonly listeners = new Map<number, (event: FolderSizeEvent) => void>();

	constructor(private readonly specs: Record<number, FakeEntry>) {
		super();
	}

	private ensure(handle: ListingHandle, id: EntryId) {
		const spec = this.specs[id];
		if (spec) this.setEntry(handle, id, spec);
	}

	override entryDetails(handle: ListingHandle, id: EntryId) {
		this.ensure(handle, id);
		return super.entryDetails(handle, id);
	}

	override async folderSize(
		handle: ListingHandle,
		id: EntryId,
		onEvent: (event: FolderSizeEvent) => void,
	): Promise<FolderSizeJob> {
		this.ensure(handle, id);
		const job = await super.folderSize(handle, id, onEvent);
		this.started.push(job.job);
		this.listeners.set(job.job, onEvent);
		return {
			job: job.job,
			cancel: async () => {
				this.cancelled.push(job.job);
				await job.cancel();
			},
		};
	}

	/** Ends a folder-size run with a failure, as Rust does when it cannot read the folder. */
	failRun(job: number) {
		this.listeners.get(job)?.({
			kind: 'failed',
			error: { kind: 'io', message: 'unreadable', location: null },
		});
	}

	override readTextHead(handle: ListingHandle, id: EntryId, max?: number) {
		this.ensure(handle, id);
		return super.readTextHead(handle, id, max);
	}
}

export interface MountedInspector {
	vfs: FakeVfsClient;
	session: ListingSession;
	store: InspectorStore;
	details: LazyDetails;
	view: ReturnType<typeof render>;
}

/** Selects the entry at `position` the way a click does. */
export function clickEntry(session: ListingSession, position: number) {
	const entry = session.model.entryAt(position);
	if (!entry) throw new Error(`no entry at ${position}`);
	session.store.getState().click(position, entry.id);
}

/**
 * The panel over the test tree's home folder (docs, music, notes.txt, photo.jpg, in that order),
 * with the entries' details described by `specs`.
 */
export async function mountInspector(
	specs: Record<number, FakeEntry>,
	options: {
		state?: Partial<InspectorState>;
		openWith?: OpenWithClient;
		withDetails?: boolean;
	} = {},
): Promise<MountedInspector> {
	const vfs = createTree();
	const model = await openListingModel(vfs, HOME);
	await model.readRange(0, model.count);
	const session = createListingSession(model);
	const details = new LazyDetails(specs);
	const store = createInspectorStore({ open: true, ...options.state });
	const openWith = options.openWith ?? createFakeOpenWithClient();
	const view = render(
		<VfsClientProvider client={vfs}>
			<OpenWithProvider client={openWith}>
				<DetailsClientProvider client={options.withDetails === false ? undefined : details}>
					<InspectorStoreContext.Provider value={store}>
						<InspectorPanel session={session} location={HOME} />
					</InspectorStoreContext.Provider>
				</DetailsClientProvider>
			</OpenWithProvider>
		</VfsClientProvider>,
	);
	return { vfs, session, store, details, view };
}
