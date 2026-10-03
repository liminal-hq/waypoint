// Verifies the loader: what it asks for, the order, what scrolling and leaving withdraw, and what it keeps
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createFakeThumbnailsClient } from './fakeThumbnailsClient';
import { ThumbnailLoader, type LoaderTransport } from './thumbnailLoader';
import type { EntryRequest } from './thumbnailsClient';

const items = (...ids: number[]): EntryRequest[] => ids.map((id) => ({ key: `k${id}`, id }));

function setup() {
	const client = createFakeThumbnailsClient();
	const transport: LoaderTransport<EntryRequest> = {
		request: (list, size, onEvent) => client.requestEntries(1, list, size, onEvent),
		prioritise: (ticket, keys) => client.prioritise(ticket, keys),
		cancel: (ticket) => client.cancel(ticket),
	};
	const loader = new ThumbnailLoader(transport, 'normal');
	return { client, loader };
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe('asking', () => {
	it('queues what is wanted as one batch, in view first and then the margin', async () => {
		const { client, loader } = setup();
		loader.want(items(3, 4), items(5, 2));
		await settle();
		expect(client.batches).toHaveLength(1);
		expect(client.batches[0]?.keys).toEqual(['k3', 'k4', 'k5', 'k2']);
		expect(client.batches[0]?.size).toBe('normal');
		expect(loader.stateOf('k3')).toEqual({ status: 'pending' });
	});

	it('asks only for what it has not asked for already', async () => {
		const { client, loader } = setup();
		loader.want(items(1, 2));
		await settle();
		loader.want(items(2, 3));
		await settle();
		expect(client.batches.map((batch) => batch.keys)).toEqual([['k1', 'k2'], ['k3']]);
	});

	it('shows a result under its key and tells the listeners of that key only', async () => {
		const { client, loader } = setup();
		const one = vi.fn();
		const two = vi.fn();
		loader.subscribe('k1', one);
		loader.subscribe('k2', two);
		loader.want(items(1, 2));
		await settle();
		client.ready('k1', 'thumb://localhost/normal/a.png');
		expect(loader.urlOf('k1')).toBe('thumb://localhost/normal/a.png');
		expect(one).toHaveBeenCalledTimes(1);
		expect(two).not.toHaveBeenCalled();
	});

	it('keeps a failure or a skip, so the file is not asked about again', async () => {
		const { client, loader } = setup();
		loader.want(items(1, 2));
		await settle();
		client.fail('k1');
		client.skip('k2', 'remote');
		expect(loader.urlOf('k1')).toBeNull();
		expect(loader.stateOf('k1')).toEqual({ status: 'none' });
		loader.want(items(1, 2));
		await settle();
		expect(client.batches).toHaveLength(1);
	});

	it('leaves the icons when the request itself fails', async () => {
		const { client, loader } = setup();
		vi.spyOn(client, 'requestEntries').mockRejectedValue(new Error('no plugin'));
		loader.want(items(1));
		await settle();
		expect(loader.stateOf('k1')).toEqual({ status: 'none' });
	});
});

describe('scrolling', () => {
	it('moves what is in view to the front of its batch, in view order', async () => {
		const { client, loader } = setup();
		loader.want(items(1, 2, 3, 4), []);
		await settle();
		loader.want(items(3, 4), items(2));
		await settle();
		expect(client.batches[0]?.prioritised).toEqual([['k3', 'k4']]);
	});

	it('does not repeat an order it already sent', async () => {
		const { client, loader } = setup();
		loader.want(items(1, 2, 3));
		await settle();
		loader.want(items(2, 3));
		loader.want(items(2, 3));
		await settle();
		expect(client.batches[0]?.prioritised).toHaveLength(1);
	});

	it('withdraws a batch when nothing it holds is wanted any more, and can ask again later', async () => {
		const { client, loader } = setup();
		loader.want(items(1, 2));
		await settle();
		loader.want(items(50, 51));
		await settle();
		expect(client.batches[0]?.cancelled).toBe(true);
		expect(client.batches[1]?.cancelled).toBe(false);
		expect(loader.stateOf('k1')).toBeUndefined();
		loader.want(items(1));
		await settle();
		expect(client.batches[2]?.keys).toEqual(['k1']);
	});

	it('keeps a batch while any of it is still near', async () => {
		const { client, loader } = setup();
		loader.want(items(1, 2, 3));
		await settle();
		loader.want(items(3, 4));
		await settle();
		expect(client.batches[0]?.cancelled).toBe(false);
	});

	it('keeps what arrived, so scrolling back shows it at once', async () => {
		const { client, loader } = setup();
		loader.want(items(1));
		await settle();
		client.ready('k1', 'thumb://localhost/normal/a.png');
		loader.want(items(50));
		await settle();
		expect(loader.urlOf('k1')).toBe('thumb://localhost/normal/a.png');
	});

	it('withdraws a batch whose ticket had not arrived when it was left', async () => {
		const { client, loader } = setup();
		const hold = client.hold();
		loader.want(items(1));
		loader.want(items(9));
		hold.release();
		await settle();
		expect(client.batches.find((batch) => batch.keys[0] === 'k1')?.cancelled).toBe(true);
		expect(client.batches.find((batch) => batch.keys[0] === 'k9')?.cancelled).toBe(false);
	});
});

describe('leaving', () => {
	it('withdraws every batch and ignores what arrives afterwards', async () => {
		const { client, loader } = setup();
		const listener = vi.fn();
		loader.subscribe('k1', listener);
		loader.want(items(1, 2));
		await settle();
		loader.dispose();
		await settle();
		expect(client.batches[0]?.cancelled).toBe(true);
		client.ready('k1');
		expect(loader.urlOf('k1')).toBeNull();
		expect(listener).not.toHaveBeenCalled();
		loader.want(items(7));
		await settle();
		expect(client.batches).toHaveLength(1);
	});
});
