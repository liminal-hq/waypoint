// Verifies D29: closing one half of a pair asks while an operation writes to the other half, and only then
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { FakeTabsApi } from '../services/fakeTabsApi';
import { fileLocation } from '../services/fakeVfsClient';
import { request } from '../test/opsHarness';
import { createCloseGuard, otherHalves } from './closeGuard';
import { createTabActions } from './tabActions';

const home = fileLocation('/home/test');
const docs = fileLocation('/home/test/docs');
const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

async function pairOfTwo() {
	const api = new FakeTabsApi();
	const a = await api.openTab(home);
	const b = await api.openTab(docs);
	await api.joinPair([a, b], 'sideBySide');
	return { api, a, b };
}

describe('otherHalves', () => {
	it('lists the folders of the other panes of the pair, and none for a tab on its own', async () => {
		const { api, a, b } = await pairOfTwo();
		const lone = await api.openTab(fileLocation('/tmp'));
		const snapshot = await api.getSnapshot();
		expect(otherHalves(snapshot, a)).toEqual([docs]);
		expect(otherHalves(snapshot, b)).toEqual([home]);
		expect(otherHalves(snapshot, lone)).toEqual([]);
	});
});

describe('createCloseGuard', () => {
	it('lets a close through without asking when no job targets the other half', async () => {
		const { api, a } = await pairOfTwo();
		const confirm = vi.fn(async () => false);
		const guard = createCloseGuard(async () => [], confirm);
		expect(await guard(await api.getSnapshot(), a)).toBe(true);
		expect(confirm).not.toHaveBeenCalled();
	});

	it('asks when a running job targets the other half, and obeys the answer', async () => {
		const { api, a } = await pairOfTwo();
		const snapshot = await api.getSnapshot();
		const asked: string[] = [];
		const targeting = vi.fn(async (location: { uri: string }) =>
			location.uri === docs.uri ? [1] : [],
		);
		const no = createCloseGuard(targeting, async () => {
			asked.push('no');
			return false;
		});
		expect(await no(snapshot, a)).toBe(false);
		expect(targeting).toHaveBeenCalledWith(docs);
		const yes = createCloseGuard(targeting, async () => true);
		expect(await yes(snapshot, a)).toBe(true);
		expect(asked).toEqual(['no']);
	});

	it('does not ask about a tab that is not in a pair, whatever the queue says', async () => {
		const { api } = await pairOfTwo();
		const lone = await api.openTab(fileLocation('/tmp'));
		const guard = createCloseGuard(
			async () => [1],
			vi.fn(async () => false),
		);
		expect(await guard(await api.getSnapshot(), lone)).toBe(true);
	});

	it('never blocks a close when the queue cannot answer', async () => {
		const { api, a } = await pairOfTwo();
		const guard = createCloseGuard(
			async () => {
				throw new Error('no queue');
			},
			vi.fn(async () => false),
		);
		expect(await guard(await api.getSnapshot(), a)).toBe(true);
	});
});

describe('closing a tab', () => {
	it('is held back by the guard until the person agrees, and the other half’s job really counts', async () => {
		const { api, a } = await pairOfTwo();
		const fake = createFakeOpsClient();
		const job = await fake.submit(request(['x'], docs.display));
		const answers: boolean[] = [false, true];
		const confirm = vi.fn(async () => answers.shift()!);
		const guard = createCloseGuard((location) => fake.jobsTargeting(location), confirm);
		const actions = createTabActions(api, await api.getSnapshot(), home, guard);

		actions.close(a);
		await settle();
		expect((await api.getSnapshot()).tabs.map((tab) => tab.id)).toContain(a);

		actions.close(a);
		await settle();
		expect((await api.getSnapshot()).tabs.map((tab) => tab.id)).not.toContain(a);
		expect(confirm).toHaveBeenCalledTimes(2);

		// Once the job is finished nothing is asked.
		fake.start(job);
		fake.done(job);
		const { api: other, a: first } = await pairOfTwo();
		const later = createTabActions(other, await other.getSnapshot(), home, guard);
		later.close(first);
		await settle();
		expect(confirm).toHaveBeenCalledTimes(2);
	});
});
