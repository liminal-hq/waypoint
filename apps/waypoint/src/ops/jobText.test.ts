// Verifies the words for a job: titles, routes, states, progress detail and which actions each state offers
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { createFakeOpsClient } from '../services/fakeOpsClient';
import { fileLocation } from '../services/fakeVfsClient';
import { request } from '../test/opsHarness';
import { errorText, jobDoneText, jobTitle, jobViews } from './jobText';
import { withProgress } from './opsSelectors';

const views = (fake: ReturnType<typeof createFakeOpsClient>, canShow = true) =>
	jobViews(
		fake.jobs().map((job) => withProgress(job, 0, undefined)),
		canShow,
	);

describe('the words for creating and renaming', () => {
	it('name what was made, and what was renamed, because a create has no source', async () => {
		const fake = createFakeOpsClient();
		const where = fileLocation('/home/a');
		await fake.submit({
			kind: { kind: 'createFolder' },
			sources: { kind: 'locations', locations: [] },
			destination: where,
			name: 'untitled folder',
			options: { conflict: 'keepBoth', verify: null },
			originWindow: 'main-1',
		});
		await fake.submit({
			kind: { kind: 'createFile' },
			sources: { kind: 'locations', locations: [] },
			destination: where,
			name: 'untitled file',
			options: { conflict: 'keepBoth', verify: null },
			originWindow: 'main-1',
		});
		await fake.submit({
			kind: { kind: 'rename' },
			sources: { kind: 'locations', locations: [fileLocation('/home/a/old.txt')] },
			destination: null,
			name: 'new.txt',
			options: { conflict: null, verify: null },
			originWindow: 'main-1',
		});
		const [folder, file, rename] = fake.jobs();
		expect(jobTitle(folder!)).toBe('Creating folder untitled folder');
		expect(jobDoneText(folder!)).toBe('Created folder untitled folder');
		expect(jobDoneText(file!)).toBe('Created file untitled file');
		expect(jobTitle(rename!)).toBe('Renaming old.txt');
		expect(jobDoneText(rename!)).toBe('Renamed old.txt');
	});
});

describe('job words', () => {
	it('words a single item by its name and several by their count', async () => {
		const fake = createFakeOpsClient();
		await fake.submit(request(['report.pdf']));
		await fake.submit(request(['a', 'b', 'c']));
		const [one, many] = fake.jobs();
		expect(jobTitle(one!)).toBe('Copying report.pdf');
		expect(jobTitle(many!)).toBe('Copying 3 items');
		expect(jobDoneText(one!)).toBe('Copied report.pdf');
		expect(jobDoneText(many!)).toBe('Copied 3 items');
	});

	it('shows source and destination in the route', async () => {
		const fake = createFakeOpsClient();
		await fake.submit(request(['report.pdf'], '/home/me/Backups'));
		expect(views(fake)[0]!.route).toBe('report.pdf → Backups');
	});

	it('words each error with the name of the entry it is about', () => {
		const location = fileLocation('/a/b/secret.txt');
		expect(errorText({ kind: 'permissionDenied', location })).toBe(
			'Permission denied for secret.txt',
		);
		expect(errorText({ kind: 'sameFolder' })).toBe('The items are already in that folder');
	});

	it('gives each state its actions', async () => {
		const fake = createFakeOpsClient({ concurrency: 9 });
		const ids = [];
		for (let i = 0; i < 6; i++) ids.push(await fake.submit(request([`f${i}`])));
		fake.start(ids[0]!);
		fake.start(ids[1]!);
		await fake.pause(ids[1]!);
		fake.start(ids[2]!);
		fake.askConflicts(ids[2]!, []);
		fake.start(ids[3]!);
		fake.fail(ids[3]!, { kind: 'io', message: 'x' });
		fake.start(ids[4]!);
		fake.done(ids[4]!);
		const byState = Object.fromEntries(views(fake).map((v) => [v.state, v.actions]));
		expect(byState).toMatchObject({
			running: ['pause', 'cancel'],
			paused: ['resume', 'cancel'],
			waiting: ['resolve', 'cancel'],
			failed: ['retry', 'dismiss'],
			done: ['showInFolder', 'dismiss'],
			queued: ['cancel'],
		});
	});

	it('offers no Show in folder where the window cannot show one', async () => {
		const fake = createFakeOpsClient();
		const id = await fake.submit(request());
		fake.start(id);
		fake.done(id);
		expect(views(fake, false)[0]!.actions).toEqual(['dismiss']);
	});

	it('numbers the queued jobs and says which can move', async () => {
		const fake = createFakeOpsClient({ concurrency: 1 });
		for (const name of ['a', 'b', 'c']) await fake.submit(request([name]));
		fake.start(fake.jobs()[0]!.id);
		const [running, b, c] = views(fake);
		expect(running!.queuePosition).toBeNull();
		expect([b!.queuePosition, b!.canMoveUp, b!.canMoveDown]).toEqual([0, false, true]);
		expect([c!.queuePosition, c!.canMoveUp, c!.canMoveDown]).toEqual([1, true, false]);
	});

	it('shows speed and time left for a running job', async () => {
		const fake = createFakeOpsClient();
		const id = await fake.submit(request());
		fake.start(id);
		fake.tick(id, {
			bytesDone: 5_000_000,
			bytesTotal: 20_000_000,
			speedBps: 2_000_000,
			etaMs: 90_000,
		});
		const [view] = views(fake);
		expect(view!.detail).toContain('2 MB/s');
		expect(view!.detail).toContain('2 min left');
		expect(view!.fraction).toBeCloseTo(0.25);
	});
});

describe('every worded kind', () => {
	it.each(['copy', 'move', 'link', 'trash', 'delete', 'duplicate', 'restore'] as const)(
		'has a present and a past tense for one named item and for several (%s)',
		async (kind) => {
			const fake = createFakeOpsClient();
			const submit = (names: string[]) =>
				fake.submit({
					...request(names),
					kind: { kind },
				});
			await submit(['one.txt']);
			await submit(['a', 'b']);
			for (const job of fake.jobs()) {
				expect(jobTitle(job)).not.toBe('');
				expect(jobDoneText(job)).not.toContain('Finished:');
			}
		},
	);
});
