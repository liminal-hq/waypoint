// Verifies `pickDestination`: it asks through the store, answers once, and never waits where nothing can ask
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { fileLocation } from '../services/fakeVfsClient';
import { attachHost, createDestinationStore, pickDestination } from './destinationStore';

const options = {
	title: 'Choose a folder',
	confirmLabel: 'Choose',
	base: fileLocation('/home/test'),
};

describe('pickDestination', () => {
	it('opens a question in the store and resolves to the answer, closing it', async () => {
		const store = createDestinationStore();
		attachHost(store);
		const picked = pickDestination(options, store);
		const { request } = store.getState();
		expect(request?.options).toEqual(options);
		request!.resolve(fileLocation('/home/test/docs'));
		expect(await picked).toEqual(fileLocation('/home/test/docs'));
		expect(store.getState().request).toBeNull();
	});

	it('resolves to null when it is cancelled', async () => {
		const store = createDestinationStore();
		attachHost(store);
		const picked = pickDestination(options, store);
		store.getState().request!.resolve(null);
		expect(await picked).toBeNull();
	});

	it('answers the first question null when a second is asked, and the second is the one open', async () => {
		const store = createDestinationStore();
		attachHost(store);
		const first = pickDestination(options, store);
		const second = pickDestination({ ...options, title: 'Again' }, store);
		expect(await first).toBeNull();
		expect(store.getState().request?.options.title).toBe('Again');
		store.getState().request!.resolve(fileLocation('/x'));
		expect(await second).toEqual(fileLocation('/x'));
	});

	it('does not close a newer question when an older one is answered late', async () => {
		const store = createDestinationStore();
		attachHost(store);
		const first = pickDestination(options, store);
		const older = store.getState().request!;
		const second = pickDestination({ ...options, title: 'Again' }, store);
		older.resolve(fileLocation('/late'));
		await first;
		expect(store.getState().request?.options.title).toBe('Again');
		store.getState().request!.resolve(null);
		await second;
	});

	it('resolves to null at once where no dialog is mounted to ask in', async () => {
		const store = createDestinationStore();
		expect(await pickDestination(options, store)).toBeNull();
		expect(store.getState().request).toBeNull();
	});

	it('cancels a question left open when the last host goes away', async () => {
		const store = createDestinationStore();
		const detach = attachHost(store);
		const picked = pickDestination(options, store);
		detach();
		expect(await picked).toBeNull();
		expect(store.getState().request).toBeNull();
	});
});
