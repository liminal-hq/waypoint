// An in-memory DetailsClient for building and testing the Inspector without the Rust side
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { EntryDetails } from '@liminal-hq/waypoint-protocol/generated/EntryDetails';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { FolderSizeEvent } from '@liminal-hq/waypoint-protocol/generated/FolderSizeEvent';
import type { FolderSizeTotals } from '@liminal-hq/waypoint-protocol/generated/FolderSizeTotals';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { TextHead } from '@liminal-hq/waypoint-protocol/generated/TextHead';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import type { DetailsClient, FolderSizeJob } from './detailsClient';

/** The most a text head holds, as in the plugin. */
export const TEXT_HEAD_MAX = 256 * 1024;

/** An `EntryDetails` for a plain file, to override what a test cares about. */
export function fakeDetails(overrides: Partial<EntryDetails> = {}): EntryDetails {
	return {
		name: 'file.txt',
		kind: 'file',
		resolvesTo: null,
		symlinkTarget: null,
		size: 0,
		allocatedSize: 4096,
		createdMs: null,
		modifiedMs: 0,
		accessedMs: 0,
		owner: 'test',
		group: 'test',
		mode: 0o644,
		readOnly: false,
		hidden: false,
		mimeType: 'text/plain',
		unavailable: ['created'],
		...overrides,
	};
}

/** What the fake holds for one entry. */
export interface FakeEntry {
	details: EntryDetails;
	/** The file's text; `null` for a binary file (the head is refused with `notText`). */
	text?: string | null;
	/** The totals a folder-size run ends with. */
	folderTotals?: FolderSizeTotals;
}

interface Run {
	handle: ListingHandle;
	onEvent: (event: FolderSizeEvent) => void;
	totals: FolderSizeTotals;
	final: FolderSizeTotals;
	ended: boolean;
}

const key = (handle: ListingHandle, id: EntryId) => `${handle}:${id}`;

const EMPTY: FolderSizeTotals = {
	files: 0,
	folders: 0,
	bytes: 0,
	allocatedBytes: null,
	symlinksSkipped: 0,
	mountsSkipped: 0,
	placeholders: 0,
	unreadable: 0,
};

/**
 * Follows the plugin's contract: an unknown handle is `staleHandle` and an unknown entry
 * `notFound`; a folder-size run is a scripted one a test advances by hand (`advance`, `finish`)
 * so progress is deterministic, and `cancel` ends it with `cancelled` and the total so far. A
 * text head is cut at the requested limit on a character boundary, and `previewUrl` builds the
 * `wpfile` token the real client does.
 */
export class FakeDetailsClient implements DetailsClient {
	private readonly entries = new Map<string, FakeEntry>();
	private readonly handles = new Set<ListingHandle>();
	private readonly runs = new Map<number, Run>();
	private nextJob = 1;
	/** Every call made, in order, for tests to check what the UI asked for. */
	readonly calls: string[] = [];

	/** Gives an entry of a listing; the listing counts as open from now on. */
	setEntry(handle: ListingHandle, id: EntryId, entry: FakeEntry): void {
		this.handles.add(handle);
		this.entries.set(key(handle, id), entry);
	}

	/** Closes a listing, so its handle is stale and its runs end cancelled. */
	closeListing(handle: ListingHandle): void {
		this.handles.delete(handle);
		for (const [job, run] of this.runs) {
			if (run.handle === handle) this.end(job, 'cancelled');
		}
	}

	private entry(handle: ListingHandle, id: EntryId): FakeEntry {
		if (!this.handles.has(handle)) throw { kind: 'staleHandle' } satisfies VfsError;
		const found = this.entries.get(key(handle, id));
		if (!found) {
			throw { kind: 'notFound', location: { display: '', uri: '' } } satisfies VfsError;
		}
		return found;
	}

	async entryDetails(handle: ListingHandle, id: EntryId): Promise<EntryDetails> {
		this.calls.push(`details ${key(handle, id)}`);
		return this.entry(handle, id).details;
	}

	async folderSize(
		handle: ListingHandle,
		id: EntryId,
		onEvent: (event: FolderSizeEvent) => void,
	): Promise<FolderSizeJob> {
		this.calls.push(`folderSize ${key(handle, id)}`);
		const entry = this.entry(handle, id);
		if (entry.details.kind !== 'directory' && entry.details.resolvesTo !== 'directory') {
			throw { kind: 'notADirectory', location: { display: '', uri: '' } } satisfies VfsError;
		}
		const job = this.nextJob++;
		this.runs.set(job, {
			handle,
			onEvent,
			totals: EMPTY,
			final: entry.folderTotals ?? EMPTY,
			ended: false,
		});
		return { job, cancel: async () => this.end(job, 'cancelled') };
	}

	/** Sends a progress event with `totals` for a run that is still going. */
	advance(job: number, totals: FolderSizeTotals): void {
		const run = this.runs.get(job);
		if (!run || run.ended) return;
		run.totals = totals;
		run.onEvent({ kind: 'progress', totals });
	}

	/** Ends a run with `done` and the totals its entry was given. */
	finish(job: number): void {
		this.end(job, 'done');
	}

	private end(job: number, kind: 'done' | 'cancelled'): void {
		const run = this.runs.get(job);
		if (!run || run.ended) return;
		run.ended = true;
		run.onEvent({ kind, totals: kind === 'done' ? run.final : run.totals });
		this.runs.delete(job);
	}

	async readTextHead(handle: ListingHandle, id: EntryId, max?: number): Promise<TextHead> {
		this.calls.push(`textHead ${key(handle, id)}`);
		const { details, text } = this.entry(handle, id);
		const location = { display: details.name, uri: '' };
		if (details.kind === 'directory') throw { kind: 'isADirectory', location } satisfies VfsError;
		if (text === null || text === undefined) {
			throw { kind: 'notText', location } satisfies VfsError;
		}
		const limit = Math.min(max ?? TEXT_HEAD_MAX, TEXT_HEAD_MAX);
		const bytes = new TextEncoder().encode(text);
		if (bytes.length <= limit) {
			return { text, truncated: false, lossy: false, bytesRead: bytes.length };
		}
		// Cut on a character boundary: back up past any continuation bytes.
		let end = limit;
		while (end > 0 && (bytes[end]! & 0xc0) === 0x80) end -= 1;
		return {
			text: new TextDecoder().decode(bytes.slice(0, end)),
			truncated: true,
			lossy: false,
			bytesRead: end,
		};
	}

	previewUrl(handle: ListingHandle, id: EntryId): string {
		return `wpfile://localhost/${handle}-${id}`;
	}
}
