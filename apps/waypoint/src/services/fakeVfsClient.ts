// An in-memory VfsClient for building and testing the file views without the Rust side
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Breadcrumb } from '@liminal-hq/waypoint-protocol/generated/Breadcrumb';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import type { EntryId } from '@liminal-hq/waypoint-protocol/generated/EntryId';
import type { EntryKind } from '@liminal-hq/waypoint-protocol/generated/EntryKind';
import type { Filter } from '@liminal-hq/waypoint-protocol/generated/Filter';
import type { IconGroup } from '@liminal-hq/waypoint-protocol/generated/IconGroup';
import type { ListingEvent } from '@liminal-hq/waypoint-protocol/generated/ListingEvent';
import type { ListingHandle } from '@liminal-hq/waypoint-protocol/generated/ListingHandle';
import type { ListingSnapshot } from '@liminal-hq/waypoint-protocol/generated/ListingSnapshot';
import type { Location } from '@liminal-hq/waypoint-protocol/generated/Location';
import type { LocationInfo } from '@liminal-hq/waypoint-protocol/generated/LocationInfo';
import type { PatchOp } from '@liminal-hq/waypoint-protocol/generated/PatchOp';
import type { SelectionSpec } from '@liminal-hq/waypoint-protocol/generated/SelectionSpec';
import type { SelectionSummary } from '@liminal-hq/waypoint-protocol/generated/SelectionSummary';
import type { SortSpec } from '@liminal-hq/waypoint-protocol/generated/SortSpec';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import type { VolumeSpace } from '@liminal-hq/waypoint-protocol/generated/VolumeSpace';
import type { OpenOptions, Unsubscribe, VfsClient } from './vfsClient';

/** A local `file://` location for a path, the way `waypoint-path` will build one. */
export function fileLocation(path: string): Location {
	return { display: path, uri: `file://${path.split('/').map(encodeURIComponent).join('/')}` };
}

/** The path a local `file://` location names (what `waypoint-path` parses the `uri` to). */
export function pathOf(location: Location): string {
	const raw = location.uri.startsWith('file://') ? location.uri.slice('file://'.length) : '';
	return raw.split('/').map(decodeURIComponent).join('/') || '/';
}

/** Resolves `.`, `..` and repeated slashes; the result is absolute and has no trailing slash. */
function normalisePath(path: string): string {
	const parts: string[] = [];
	for (const part of path.split('/')) {
		if (part === '' || part === '.') continue;
		if (part === '..') parts.pop();
		else parts.push(part);
	}
	return `/${parts.join('/')}`;
}

function joinPath(folder: string, name: string): string {
	return folder === '/' ? `/${name}` : `${folder}/${name}`;
}

const EXTENSION_GROUPS: Record<string, IconGroup> = {
	jpg: 'image',
	png: 'image',
	mp3: 'audio',
	mp4: 'video',
	zip: 'archive',
	rs: 'code',
	ts: 'code',
	pdf: 'document',
	md: 'document',
	txt: 'document',
};

/** Builds an entry, deriving the icon group from the extension like the Rust side will. */
export function makeEntry(id: number, name: string, overrides: Partial<Entry> = {}): Entry {
	const kind: EntryKind = overrides.kind ?? 'file';
	const extension = name.includes('.') ? name.split('.').pop()!.toLowerCase() : '';
	return {
		id,
		name,
		kind,
		linkTarget: null,
		group: kind === 'directory' ? 'folder' : (EXTENSION_GROUPS[extension] ?? 'other'),
		size: kind === 'directory' ? null : 1024 + id,
		modifiedMs: 1_700_000_000_000 + id * 1000,
		hidden: name.startsWith('.'),
		...overrides,
	};
}

/** `count` deterministic entries with a mix of folders and file types, in scrambled order. */
export function syntheticEntries(count: number): Entry[] {
	const extensions = ['txt', 'jpg', 'pdf', 'rs', 'ts', 'mp3', 'zip', 'md'];
	return Array.from({ length: count }, (_, i) => {
		const scrambled = (i * 7919) % Math.max(count, 1);
		return i % 17 === 0
			? makeEntry(i, `folder-${String(scrambled).padStart(6, '0')}`, { kind: 'directory' })
			: makeEntry(
					i,
					`file-${String(scrambled).padStart(6, '0')}.${extensions[i % extensions.length]}`,
				);
	});
}

const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: 'base' });

function compare(sort: SortSpec): (a: Entry, b: Entry) => number {
	const direction = sort.descending ? -1 : 1;
	return (a, b) => {
		if (sort.directoriesFirst) {
			const aDir = a.kind === 'directory' || a.linkTarget === 'directory';
			const bDir = b.kind === 'directory' || b.linkTarget === 'directory';
			if (aDir !== bDir) return aDir ? -1 : 1;
		}
		let result = 0;
		switch (sort.key) {
			case 'name':
				result = collator.compare(a.name, b.name);
				break;
			case 'size':
				result = (a.size ?? -1) - (b.size ?? -1);
				break;
			case 'modified':
				result = (a.modifiedMs ?? 0) - (b.modifiedMs ?? 0);
				break;
			case 'kind':
				result = a.group.localeCompare(b.group) || collator.compare(a.name, b.name);
				break;
		}
		return (result || a.id - b.id) * direction;
	};
}

interface OpenListing {
	handle: number;
	location: Location;
	sort: SortSpec;
	filter: Filter;
	revision: number;
	view: Entry[];
}

export interface FakeVfsOptions {
	/** Delays every reply, to exercise placeholder and loading states. */
	latencyMs?: number;
	/** What `~` means when parsing typed text. Defaults to `/home/demo`. */
	home?: string;
	/** The space every volume reports unless `setFreeSpace` says otherwise; `null` for unknown. */
	freeSpace?: VolumeSpace | null;
}

/**
 * Serves folders from memory through the `VfsClient` contract, including sorting, the hidden filter,
 * revisions and the patches a live watcher would produce. Tests and the views use `setFolder`,
 * `addEntries`, `removeEntries` and `updateEntries` to change a folder under an open listing.
 */
export class FakeVfsClient implements VfsClient {
	private folders = new Map<string, Entry[]>();
	private listings = new Map<number, OpenListing>();
	private listeners = new Set<(event: ListingEvent) => void>();
	private nextHandle = 1;
	private failures = new Map<string, VfsError>();
	private space = new Map<string, VolumeSpace | null>();
	private openFailure: VfsError | null = null;
	/** The files `openEntry` was asked to open, in order, as `(handle, id)` pairs. */
	readonly opened: Array<{ handle: ListingHandle; id: EntryId }> = [];

	constructor(private options: FakeVfsOptions = {}) {}

	/** Defines (or replaces) the contents of a folder. Open listings of it are refreshed. */
	setFolder(location: Location, entries: Entry[]): void {
		this.folders.set(location.uri, entries);
		this.refresh(location.uri);
	}

	addEntries(location: Location, entries: Entry[]): void {
		this.setFolder(location, [...(this.folders.get(location.uri) ?? []), ...entries]);
	}

	removeEntries(location: Location, ids: number[]): void {
		const drop = new Set(ids);
		this.setFolder(
			location,
			(this.folders.get(location.uri) ?? []).filter((entry) => !drop.has(entry.id)),
		);
	}

	updateEntries(location: Location, changes: Map<number, Partial<Entry>>): void {
		this.setFolder(
			location,
			(this.folders.get(location.uri) ?? []).map((entry) =>
				changes.has(entry.id) ? { ...entry, ...changes.get(entry.id) } : entry,
			),
		);
	}

	/** Makes opening a location fail with this error, to exercise the error states. */
	failOpening(location: Location, error: VfsError): void {
		this.failures.set(location.uri, error);
	}

	/** Sets the space reported for a location's volume (`null` for unknown). */
	setFreeSpace(location: Location, space: VolumeSpace | null): void {
		this.space.set(location.uri, space);
	}

	/** Makes `openEntry` fail with this error (or work again with `null`). */
	failOpeningEntries(error: VfsError | null): void {
		this.openFailure = error;
	}

	/** Reports a failure on an open listing, as the real watcher does when a folder disappears. */
	failListing(handle: ListingHandle, error: VfsError): void {
		this.emit({ kind: 'failed', handle, error });
	}

	private async delay(): Promise<void> {
		if (this.options.latencyMs) {
			await new Promise((resolve) => setTimeout(resolve, this.options.latencyMs));
		}
	}

	private emit(event: ListingEvent): void {
		for (const listener of [...this.listeners]) listener(event);
	}

	private build(listing: Pick<OpenListing, 'location' | 'sort' | 'filter'>): Entry[] {
		const all = this.folders.get(listing.location.uri) ?? [];
		return all
			.filter((entry) => listing.filter.showHidden || !entry.hidden)
			.sort(compare(listing.sort));
	}

	private snapshot(listing: OpenListing): ListingSnapshot {
		return {
			handle: listing.handle,
			location: listing.location,
			revision: listing.revision,
			count: listing.view.length,
			phase: 'ready',
			sort: listing.sort,
			filter: listing.filter,
		};
	}

	private get(handle: ListingHandle): OpenListing {
		const listing = this.listings.get(handle);
		if (!listing) throw { kind: 'staleHandle' } satisfies VfsError;
		return listing;
	}

	private refresh(uri: string): void {
		for (const listing of this.listings.values()) {
			if (listing.location.uri !== uri) continue;
			const next = this.build(listing);
			const ops = diff(listing.view, next);
			listing.view = next;
			if (ops.length === 0) continue;
			listing.revision += 1;
			this.emit({
				kind: 'changed',
				handle: listing.handle,
				revision: listing.revision,
				count: next.length,
				ops,
			});
		}
	}

	async openListing(location: Location, options: OpenOptions = {}): Promise<ListingSnapshot> {
		await this.delay();
		const failure = this.failures.get(location.uri);
		if (failure) throw failure;
		if (!this.folders.has(location.uri)) throw { kind: 'notFound', location } satisfies VfsError;
		const listing: OpenListing = {
			handle: this.nextHandle++,
			location,
			sort: options.sort ?? { key: 'name', descending: false, directoriesFirst: true },
			filter: options.filter ?? { showHidden: false },
			revision: 1,
			view: [],
		};
		listing.view = this.build(listing);
		this.listings.set(listing.handle, listing);
		this.emit({
			kind: 'progress',
			handle: listing.handle,
			revision: listing.revision,
			phase: 'ready',
			scanned: listing.view.length,
			count: listing.view.length,
		});
		return this.snapshot(listing);
	}

	async getRange(handle: ListingHandle, start: number, count: number): Promise<Entry[]> {
		await this.delay();
		return this.get(handle).view.slice(start, start + count);
	}

	async setSort(handle: ListingHandle, sort: SortSpec): Promise<ListingSnapshot> {
		await this.delay();
		const listing = this.get(handle);
		listing.sort = sort;
		listing.view = this.build(listing);
		listing.revision += 1;
		return this.snapshot(listing);
	}

	async setFilter(handle: ListingHandle, filter: Filter): Promise<ListingSnapshot> {
		await this.delay();
		const listing = this.get(handle);
		listing.filter = filter;
		listing.view = this.build(listing);
		listing.revision += 1;
		return this.snapshot(listing);
	}

	async parseLocation(input: string, base: Location): Promise<Location> {
		const text = input.trim();
		if (text === '' || text.includes('\0'))
			throw { kind: 'invalidLocation', input } satisfies VfsError;
		const scheme = /^([a-z][a-z0-9+.-]*):\/\//i.exec(text);
		if (scheme && scheme[1]!.toLowerCase() !== 'file') {
			throw { kind: 'unsupported', what: scheme[1]!.toLowerCase() } satisfies VfsError;
		}
		let path: string;
		if (scheme) {
			path = pathOf({ display: text, uri: text });
		} else if (text === '~' || text.startsWith('~/')) {
			path = `${this.options.home ?? '/home/demo'}${text.slice(1)}`;
		} else if (text.startsWith('/')) {
			path = text;
		} else {
			path = `${pathOf(base)}/${text}`;
		}
		return fileLocation(normalisePath(path));
	}

	async describeLocation(location: Location): Promise<LocationInfo> {
		const path = pathOf(location);
		const names = path.split('/').filter((part) => part !== '');
		const segments: Breadcrumb[] = [{ label: '/', location: fileLocation('/') }];
		names.forEach((name, index) => {
			segments.push({
				label: name,
				location: fileLocation(`/${names.slice(0, index + 1).join('/')}`),
			});
		});
		const parent = segments.length > 1 ? segments[segments.length - 2]!.location : null;
		return { parent, segments };
	}

	async entryLocation(handle: ListingHandle, id: EntryId): Promise<Location> {
		const listing = this.get(handle);
		const entry = (this.folders.get(listing.location.uri) ?? []).find((e) => e.id === id);
		if (!entry) throw { kind: 'notFound', location: listing.location } satisfies VfsError;
		return fileLocation(joinPath(pathOf(listing.location), entry.name));
	}

	async summariseSelection(
		handle: ListingHandle,
		selection: SelectionSpec,
	): Promise<SelectionSummary> {
		await this.delay();
		const ids = new Set(selection.ids);
		let count = 0;
		let totalSize = 0;
		for (const entry of this.get(handle).view) {
			if (ids.has(entry.id) !== (selection.kind === 'some')) continue;
			count += 1;
			totalSize += entry.size ?? 0;
		}
		return { count, totalSize };
	}

	async getFreeSpace(location: Location): Promise<VolumeSpace | null> {
		await this.delay();
		if (this.space.has(location.uri)) return this.space.get(location.uri) ?? null;
		return this.options.freeSpace === undefined
			? { freeBytes: 120_000_000_000, totalBytes: 500_000_000_000 }
			: this.options.freeSpace;
	}

	async openEntry(handle: ListingHandle, id: EntryId): Promise<void> {
		await this.delay();
		const listing = this.get(handle);
		if (this.openFailure) throw this.openFailure;
		if (!listing.view.some((entry) => entry.id === id)) {
			throw { kind: 'notFound', location: listing.location } satisfies VfsError;
		}
		this.opened.push({ handle, id });
	}

	async closeListing(handle: ListingHandle): Promise<void> {
		this.listings.delete(handle);
	}

	onListingEvent(listener: (event: ListingEvent) => void): Unsubscribe {
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	}

	/** How many listings are open, so tests can check that closing releases them. */
	get openCount(): number {
		return this.listings.size;
	}
}

/**
 * The operations that turn `before` into `after`, in the order a cache applies them: removals from
 * the highest position down, then insertions from the lowest up, then in-place updates, all in
 * final positions. A reordering of surviving entries cannot be expressed that way, so it is a reset.
 */
export function diff(before: Entry[], after: Entry[]): PatchOp[] {
	const afterIds = new Set(after.map((entry) => entry.id));
	const beforeIds = new Set(before.map((entry) => entry.id));
	const survivors = before.filter((entry) => afterIds.has(entry.id));
	const expected = after.filter((entry) => beforeIds.has(entry.id));
	if (survivors.some((entry, i) => entry.id !== expected[i]!.id)) return [{ kind: 'reset' }];

	const ops: PatchOp[] = [];
	for (let i = before.length - 1; i >= 0; i--) {
		if (afterIds.has(before[i]!.id)) continue;
		const last = ops[ops.length - 1];
		if (last?.kind === 'remove' && last.at === i + 1) {
			ops[ops.length - 1] = { kind: 'remove', at: i, count: last.count + 1 };
		} else {
			ops.push({ kind: 'remove', at: i, count: 1 });
		}
	}
	for (let i = 0; i < after.length; i++) {
		if (beforeIds.has(after[i]!.id)) continue;
		const last = ops[ops.length - 1];
		if (last?.kind === 'insert' && last.at + last.count === i) {
			ops[ops.length - 1] = { kind: 'insert', at: last.at, count: last.count + 1 };
		} else {
			ops.push({ kind: 'insert', at: i, count: 1 });
		}
	}
	const previous = new Map(before.map((entry) => [entry.id, entry]));
	for (let i = 0; i < after.length; i++) {
		const old = previous.get(after[i]!.id);
		if (!old || JSON.stringify(old) === JSON.stringify(after[i])) continue;
		const last = ops[ops.length - 1];
		if (last?.kind === 'update' && last.at + last.count === i) {
			ops[ops.length - 1] = { kind: 'update', at: last.at, count: last.count + 1 };
		} else {
			ops.push({ kind: 'update', at: i, count: 1 });
		}
	}
	return ops;
}
