// Verifies the fake details client keeps the plugin's contract
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { FolderSizeEvent } from '@liminal-hq/waypoint-protocol/generated/FolderSizeEvent';
import type { FolderSizeTotals } from '@liminal-hq/waypoint-protocol/generated/FolderSizeTotals';
import { describe, expect, it } from 'vitest';
import { FakeDetailsClient, fakeDetails } from './fakeDetailsClient';

const totals = (bytes: number, files = 1): FolderSizeTotals => ({
	files,
	folders: 0,
	bytes,
	allocatedBytes: null,
	symlinksSkipped: 0,
	mountsSkipped: 0,
	placeholders: 0,
	unreadable: 0,
});

describe('entryDetails', () => {
	it('returns what an entry was given, and rejects a closed listing or an unknown entry', async () => {
		const client = new FakeDetailsClient();
		client.setEntry(1, 5, { details: fakeDetails({ name: 'a.txt', size: 9 }) });
		expect((await client.entryDetails(1, 5)).size).toBe(9);
		await expect(client.entryDetails(1, 6)).rejects.toMatchObject({ kind: 'notFound' });
		await expect(client.entryDetails(2, 5)).rejects.toMatchObject({ kind: 'staleHandle' });
		client.closeListing(1);
		await expect(client.entryDetails(1, 5)).rejects.toMatchObject({ kind: 'staleHandle' });
	});
});

describe('folderSize', () => {
	const folder = (client: FakeDetailsClient) =>
		client.setEntry(1, 1, {
			details: fakeDetails({ kind: 'directory', size: null }),
			folderTotals: totals(300, 3),
		});

	it('streams progress and ends with exactly one done', async () => {
		const client = new FakeDetailsClient();
		folder(client);
		const events: FolderSizeEvent[] = [];
		const run = await client.folderSize(1, 1, (event) => events.push(event));
		client.advance(run.job, totals(100));
		client.finish(run.job);
		client.finish(run.job);
		client.advance(run.job, totals(999));
		expect(events.map((event) => event.kind)).toEqual(['progress', 'done']);
		expect(events[1]).toEqual({ kind: 'done', totals: totals(300, 3) });
	});

	it('cancels with the partial total, once', async () => {
		const client = new FakeDetailsClient();
		folder(client);
		const events: FolderSizeEvent[] = [];
		const run = await client.folderSize(1, 1, (event) => events.push(event));
		client.advance(run.job, totals(100));
		await run.cancel();
		await run.cancel();
		expect(events.at(-1)).toEqual({ kind: 'cancelled', totals: totals(100) });
		expect(events).toHaveLength(2);
	});

	it('cancels a run when its listing closes, and refuses a file', async () => {
		const client = new FakeDetailsClient();
		folder(client);
		client.setEntry(1, 2, { details: fakeDetails() });
		await expect(client.folderSize(1, 2, () => {})).rejects.toMatchObject({
			kind: 'notADirectory',
		});
		const events: FolderSizeEvent[] = [];
		await client.folderSize(1, 1, (event) => events.push(event));
		client.closeListing(1);
		expect(events.map((event) => event.kind)).toEqual(['cancelled']);
	});
});

describe('readTextHead', () => {
	it('cuts at the limit on a character boundary and flags the truncation', async () => {
		const client = new FakeDetailsClient();
		client.setEntry(1, 1, { details: fakeDetails(), text: 'ééé' });
		const head = await client.readTextHead(1, 1, 5);
		expect(head).toEqual({ text: 'éé', truncated: true, lossy: false, bytesRead: 4 });
		expect((await client.readTextHead(1, 1)).truncated).toBe(false);
	});

	it('refuses binary data and folders', async () => {
		const client = new FakeDetailsClient();
		client.setEntry(1, 1, { details: fakeDetails(), text: null });
		client.setEntry(1, 2, { details: fakeDetails({ kind: 'directory' }) });
		await expect(client.readTextHead(1, 1)).rejects.toMatchObject({ kind: 'notText' });
		await expect(client.readTextHead(1, 2)).rejects.toMatchObject({ kind: 'isADirectory' });
	});
});

describe('previewUrl', () => {
	it('is a token over the handle and the entry, never a path', () => {
		expect(new FakeDetailsClient().previewUrl(3, 17)).toBe('wpfile://localhost/3-17');
	});
});
