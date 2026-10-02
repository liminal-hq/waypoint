// Verifies the fake places client keeps the plugin's bookmarks semantics and announces each mutation
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { FakePlacesClient, fakePlaces } from './fakePlacesClient';
import { fileLocation } from './fakeVfsClient';

const A = fileLocation('/srv/alpha');
const B = fileLocation('/srv/beta');
const C = fileLocation('/srv/gamma');
const labels = async (client: FakePlacesClient) =>
	(await client.list()).favourites.map((favourite) => favourite.label);

describe('the places', () => {
	it('lists the fixed places in order, Home first', async () => {
		const { places } = await new FakePlacesClient().list();
		expect(places.map((place) => place.kind)).toEqual([
			'home',
			'desktop',
			'documents',
			'downloads',
			'pictures',
			'music',
			'videos',
			'trash',
		]);
		expect(fakePlaces('/h')[2]!.location.display).toBe('/h/Documents');
		expect(fakePlaces('/h')[7]!.location).toEqual({ display: 'Trash', uri: 'trash:/' });
	});
});

describe('the favourites', () => {
	it('adds a folder labelled with its own name, or with the label given', async () => {
		const client = new FakePlacesClient();
		await client.addFavourite(A);
		await client.addFavourite(B, '  My   beta ');
		expect(await labels(client)).toEqual(['alpha', 'My beta']);
	});

	it('does not pin a folder twice', async () => {
		const client = new FakePlacesClient();
		await client.addFavourite(A);
		const after = await client.addFavourite(A, 'again');
		expect(after.favourites).toHaveLength(1);
		expect(after.favourites[0]!.label).toBe('alpha');
	});

	it('removes a favourite, and leaves the rest alone when it is not pinned', async () => {
		const client = new FakePlacesClient();
		await client.addFavourite(A);
		await client.addFavourite(B);
		await client.removeFavourite(C);
		expect(await labels(client)).toEqual(['alpha', 'beta']);
		await client.removeFavourite(A);
		expect(await labels(client)).toEqual(['beta']);
	});

	it('renames a favourite and clears the label with null or a blank one', async () => {
		const client = new FakePlacesClient();
		await client.addFavourite(A);
		await client.renameFavourite(A, 'First');
		expect(await labels(client)).toEqual(['First']);
		await client.renameFavourite(A, null);
		expect(await labels(client)).toEqual(['alpha']);
		await client.renameFavourite(A, 'x');
		await client.renameFavourite(A, '   ');
		expect(await labels(client)).toEqual(['alpha']);
	});

	it('moves a favourite to an index of the list without it, clamped to the ends', async () => {
		const client = new FakePlacesClient();
		for (const location of [A, B, C]) await client.addFavourite(location);
		await client.moveFavourite(A, 1);
		expect(await labels(client)).toEqual(['beta', 'alpha', 'gamma']);
		await client.moveFavourite(A, 99);
		expect(await labels(client)).toEqual(['beta', 'gamma', 'alpha']);
		await client.moveFavourite(A, -4);
		expect(await labels(client)).toEqual(['alpha', 'beta', 'gamma']);
	});
});

describe('changes', () => {
	it('tells every listener the new places after a mutation, until it unsubscribes', async () => {
		const client = new FakePlacesClient();
		const first = vi.fn();
		const second = vi.fn();
		client.onChange(first);
		const stop = client.onChange(second);
		await client.addFavourite(A);
		stop();
		await client.addFavourite(B);
		expect(first).toHaveBeenCalledTimes(2);
		expect(first.mock.lastCall![0].favourites).toHaveLength(2);
		expect(second).toHaveBeenCalledTimes(1);
	});

	it('does not announce a read, and rejects a mutation when told to fail', async () => {
		const client = new FakePlacesClient();
		const listener = vi.fn();
		client.onChange(listener);
		await client.list();
		expect(listener).not.toHaveBeenCalled();
		client.failNext({ kind: 'permissionDenied', location: A });
		await expect(client.addFavourite(A)).rejects.toMatchObject({ kind: 'permissionDenied' });
		await client.addFavourite(A);
		expect(listener).toHaveBeenCalledTimes(1);
	});
});
