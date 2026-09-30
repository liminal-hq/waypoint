// Tests for the in-memory VfsClient: it must honour the same contract the real plugin will
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ListingEvent } from '@liminal-hq/waypoint-protocol/generated/ListingEvent';
import { describe, expect, it } from 'vitest';
import { diff, FakeVfsClient, fileLocation, makeEntry, syntheticEntries } from './fakeVfsClient';
import { isVfsError } from './vfsClient';

const home = fileLocation('/home/scott');

function setup(
	entries = [
		makeEntry(1, 'b.txt'),
		makeEntry(2, 'a.txt'),
		makeEntry(3, 'docs', { kind: 'directory' }),
	],
) {
	const client = new FakeVfsClient();
	client.setFolder(home, entries);
	const events: ListingEvent[] = [];
	client.onListingEvent((event) => events.push(event));
	return { client, events };
}

describe('FakeVfsClient locations', () => {
	const base = fileLocation('/home/scott/docs');

	it('parses absolute, relative, home and file:// text', async () => {
		const { client } = setup();
		expect((await client.parseLocation('/etc/ssh', base)).display).toBe('/etc/ssh');
		expect((await client.parseLocation('../music/', base)).display).toBe('/home/scott/music');
		expect((await client.parseLocation('~/notes', base)).display).toBe('/home/demo/notes');
		const uri = await client.parseLocation('file:///tmp/a%20b', base);
		expect(uri).toEqual(fileLocation('/tmp/a b'));
	});

	it('rejects empty text and reports other schemes as unsupported', async () => {
		const { client } = setup();
		await expect(client.parseLocation('   ', base)).rejects.toMatchObject({
			kind: 'invalidLocation',
		});
		await expect(client.parseLocation('sftp://host/x', base)).rejects.toMatchObject({
			kind: 'unsupported',
		});
	});

	it('describes a location as breadcrumbs with a parent, and the root has none', async () => {
		const { client } = setup();
		const info = await client.describeLocation(base);
		expect(info.segments.map((s) => s.label)).toEqual(['/', 'home', 'scott', 'docs']);
		expect(info.parent).toEqual(fileLocation('/home/scott'));
		expect((await client.describeLocation(fileLocation('/'))).parent).toBeNull();
	});

	it('names an entry by its location, and rejects an unknown entry', async () => {
		const { client } = setup();
		const snapshot = await client.openListing(home);
		expect(await client.entryLocation(snapshot.handle, 3)).toEqual(
			fileLocation('/home/scott/docs'),
		);
		await expect(client.entryLocation(snapshot.handle, 99)).rejects.toMatchObject({
			kind: 'notFound',
		});
		await expect(client.entryLocation(999, 3)).rejects.toMatchObject({ kind: 'staleHandle' });
	});
});

describe('FakeVfsClient', () => {
	it('opens a listing with folders first and natural name order', async () => {
		const { client } = setup();
		const snapshot = await client.openListing(home);
		expect(snapshot).toMatchObject({ count: 3, revision: 1, phase: 'ready' });
		const rows = await client.getRange(snapshot.handle, 0, 10);
		expect(rows.map((row) => row.name)).toEqual(['docs', 'a.txt', 'b.txt']);
	});

	it('sorts numbers inside names as numbers', async () => {
		const { client } = setup([makeEntry(1, 'file10.txt'), makeEntry(2, 'file2.txt')]);
		const { handle } = await client.openListing(home);
		expect((await client.getRange(handle, 0, 5)).map((row) => row.name)).toEqual([
			'file2.txt',
			'file10.txt',
		]);
	});

	it('reads a range, which is shorter at the end of the listing', async () => {
		const { client } = setup();
		const { handle } = await client.openListing(home);
		expect(await client.getRange(handle, 1, 10)).toHaveLength(2);
		expect(await client.getRange(handle, 5, 10)).toEqual([]);
	});

	it('re-sorts and bumps the revision', async () => {
		const { client } = setup();
		const { handle } = await client.openListing(home);
		const snapshot = await client.setSort(handle, {
			key: 'name',
			descending: true,
			directoriesFirst: false,
		});
		expect(snapshot.revision).toBe(2);
		expect((await client.getRange(handle, 0, 5)).map((row) => row.name)).toEqual([
			'docs',
			'b.txt',
			'a.txt',
		]);
	});

	it('hides hidden files until asked to show them', async () => {
		const { client } = setup([makeEntry(1, '.secret'), makeEntry(2, 'shown.txt')]);
		const { handle } = await client.openListing(home);
		expect((await client.getRange(handle, 0, 5)).map((row) => row.name)).toEqual(['shown.txt']);
		const snapshot = await client.setFilter(handle, { showHidden: true });
		expect(snapshot.count).toBe(2);
	});

	it('rejects a missing location with a typed error', async () => {
		const { client } = setup();
		const error = await client.openListing(fileLocation('/nope')).catch((e: unknown) => e);
		expect(isVfsError(error)).toBe(true);
		expect(error).toMatchObject({ kind: 'notFound' });
	});

	it('can be told to fail a location, for error-state tests', async () => {
		const { client } = setup();
		client.failOpening(home, { kind: 'permissionDenied', location: home });
		await expect(client.openListing(home)).rejects.toMatchObject({ kind: 'permissionDenied' });
	});

	it('reports a closed handle as stale, and closing twice is harmless', async () => {
		const { client } = setup();
		const { handle } = await client.openListing(home);
		await client.closeListing(handle);
		await client.closeListing(handle);
		expect(client.openCount).toBe(0);
		await expect(client.getRange(handle, 0, 1)).rejects.toMatchObject({ kind: 'staleHandle' });
	});

	it('announces the first scan as a ready progress event', async () => {
		const { client, events } = setup();
		const { handle } = await client.openListing(home);
		expect(events).toEqual([
			{ kind: 'progress', handle, revision: 1, phase: 'ready', scanned: 3, count: 3 },
		]);
	});

	it('patches an open listing when files are added, removed and changed', async () => {
		const { client, events } = setup();
		const { handle } = await client.openListing(home);
		events.length = 0;

		client.addEntries(home, [makeEntry(4, 'c.txt')]);
		expect(events.at(-1)).toEqual({
			kind: 'changed',
			handle,
			revision: 2,
			count: 4,
			ops: [{ kind: 'insert', at: 3, count: 1 }],
		});

		client.removeEntries(home, [2]);
		expect(events.at(-1)).toMatchObject({
			revision: 3,
			count: 3,
			ops: [{ kind: 'remove', at: 1, count: 1 }],
		});

		client.updateEntries(home, new Map([[1, { size: 99 }]]));
		expect(events.at(-1)).toMatchObject({
			revision: 4,
			ops: [{ kind: 'update', at: 1, count: 1 }],
		});
	});

	it('does not announce a change that leaves the view as it was', async () => {
		const { client, events } = setup();
		await client.openListing(home);
		events.length = 0;
		client.updateEntries(home, new Map());
		expect(events).toEqual([]);
	});

	it('leaves other folders alone', async () => {
		const { client, events } = setup();
		const other = fileLocation('/tmp');
		client.setFolder(other, []);
		await client.openListing(home);
		events.length = 0;
		client.addEntries(other, [makeEntry(9, 'x')]);
		expect(events).toEqual([]);
	});

	it('stops delivering events after unsubscribing', async () => {
		const { client } = setup();
		const seen: ListingEvent[] = [];
		const stop = client.onListingEvent((event) => seen.push(event));
		await client.openListing(home);
		stop();
		client.addEntries(home, [makeEntry(7, 'late.txt')]);
		expect(seen).toHaveLength(1);
	});

	it('serves large synthetic folders', async () => {
		const client = new FakeVfsClient();
		client.setFolder(home, syntheticEntries(50_000));
		const { handle, count } = await client.openListing(home);
		expect(count).toBe(50_000);
		expect(await client.getRange(handle, 49_990, 50)).toHaveLength(10);
	});

	it('percent-encodes file locations', () => {
		expect(fileLocation('/a b/c').uri).toBe('file:///a%20b/c');
	});
});

describe('diff', () => {
	const a = makeEntry(1, 'a');
	const b = makeEntry(2, 'b');
	const c = makeEntry(3, 'c');

	it('is empty for identical views', () => {
		expect(diff([a, b], [a, b])).toEqual([]);
	});

	it('merges adjacent removals and insertions', () => {
		expect(diff([a, b, c], [a])).toEqual([{ kind: 'remove', at: 1, count: 2 }]);
		expect(diff([a], [a, b, c])).toEqual([{ kind: 'insert', at: 1, count: 2 }]);
	});

	it('removes from the highest position down so each position stays valid', () => {
		expect(diff([a, b, c], [b])).toEqual([
			{ kind: 'remove', at: 2, count: 1 },
			{ kind: 'remove', at: 0, count: 1 },
		]);
	});

	it('resets when surviving entries change order', () => {
		expect(diff([a, b], [b, a])).toEqual([{ kind: 'reset' }]);
	});
});
