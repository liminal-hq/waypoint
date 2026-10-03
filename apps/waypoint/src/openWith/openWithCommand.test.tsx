// Verifies the Open With… command: the system chooser for one file, the dialog otherwise, and nothing for a mixed selection
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createFakeOpenWithClient, fakeApp } from './fakeOpenWithClient';
import { createOpenWithChooserStore } from './openWithChooserStore';
import { openWithCommandAvailable, runOpenWithCommand } from './openWithCommand';
import { openWithAbilities } from './OpenWithContext';

const uris = ['file:///a.png', 'file:///b.png'];

/** A session with two files selected, as the command reads it. */
function session(count = 2) {
	const entries = uris.slice(0, count).map((_, index) => ({ id: index + 1 }));
	return {
		model: {
			handle: 1,
			count: entries.length,
			readRange: async () => entries,
		},
		store: {
			getState: () => ({ selection: { kind: 'some', ids: new Set(entries.map((e) => e.id)) } }),
		},
	};
}
const vfs = {
	entryLocation: async (_handle: number, id: number) => ({
		display: uris[id - 1]!,
		uri: uris[id - 1]!,
	}),
};

async function run(
	options: Parameters<typeof createFakeOpenWithClient>[0],
	count = 2,
	say = vi.fn(),
) {
	const client = createFakeOpenWithClient(options);
	const chooser = createOpenWithChooserStore();
	await runOpenWithCommand(session(count) as never, {
		client,
		status: await client.getStatus(),
		vfs,
		chooser,
		say,
	});
	return { client, chooser, say };
}

describe('Open With…', () => {
	it('opens the dialog over every application for a selection', async () => {
		const { chooser } = await run({
			handlers: { default: fakeApp('v'), others: [fakeApp('o')] },
		});
		expect(chooser.getState().request).toMatchObject({ scope: 'all', uris });
	});

	it('goes to the system chooser for one file where there is one', async () => {
		const { client, chooser } = await run(
			{ features: ['handlers', 'openWith', 'openDefault', 'chooser'] },
			1,
		);
		expect(client.calls).toEqual([['choose', [uris[0]]]]);
		expect(chooser.getState().request).toBeNull();
	});

	it('says so for a selection of more than one type, and opens nothing', async () => {
		const { chooser, say } = await run({ handlers: { mixed: true } });
		expect(chooser.getState().request).toBeNull();
		expect(say).toHaveBeenCalledWith('Open With needs items of one type.');
	});

	it('does nothing where the plugin cannot list applications or choose', async () => {
		const { client, chooser } = await run({ features: ['openDefault'] });
		expect(client.asked).toEqual([]);
		expect(chooser.getState().request).toBeNull();
	});

	it('is available only with a way to list or choose', async () => {
		const status = (features: string[]) =>
			createFakeOpenWithClient({ features }).getStatus().then(openWithAbilities);
		expect(openWithCommandAvailable(await status(['handlers', 'openWith']))).toBe(true);
		expect(openWithCommandAvailable(await status(['chooser']))).toBe(true);
		expect(openWithCommandAvailable(await status(['openDefault']))).toBe(false);
	});
});
