// Verifies what a typed or chosen destination comes to, and the list of recent destinations
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import { FOLDER } from '../test/browseHarness';
import { checkDestination, createRecentDestinations, RECENT_LIMIT } from './destinationModel';

function vfsWith() {
	const vfs = new FakeVfsClient({ home: '/home/test' });
	vfs.setFolder(FOLDER, [
		makeEntry(1, 'notes.txt'),
		makeEntry(2, 'docs', { kind: 'directory' }),
		makeEntry(3, 'archive', { kind: 'directory' }),
	]);
	vfs.setFolder(fileLocation('/home/test/docs'), []);
	vfs.setFolder(fileLocation('/home/test/archive'), []);
	vfs.setReadOnly(fileLocation('/home/test/archive'));
	return vfs;
}

describe('checkDestination', () => {
	it('accepts an absolute path to a folder that can be written to, parsed by Rust', async () => {
		const result = await checkDestination(vfsWith(), '/home/test/docs', FOLDER);
		expect(result).toEqual({ state: 'ok', location: fileLocation('/home/test/docs') });
	});

	it('reads relative text against the base and ~ against home, and trims the text', async () => {
		const vfs = vfsWith();
		expect(await checkDestination(vfs, '  docs  ', FOLDER)).toMatchObject({ state: 'ok' });
		expect(await checkDestination(vfs, '~/docs', FOLDER)).toEqual({
			state: 'ok',
			location: fileLocation('/home/test/docs'),
		});
	});

	it('says a folder that is not there does not exist', async () => {
		expect(await checkDestination(vfsWith(), '/home/test/nope', FOLDER)).toEqual({
			state: 'problem',
			message: '“nope” does not exist.',
		});
	});

	it('says a destination in a protocol that is turned off is turned off, not invalid', async () => {
		const vfs = new FakeVfsClient({ home: '/home/test', offSchemes: ['sftp'] });
		expect(await checkDestination(vfs, 'sftp://me@nas.lan/srv', FOLDER)).toEqual({
			state: 'problem',
			message: 'SFTP (SSH) is turned off in Settings → Experimental.',
		});
	});

	it('says a file is not a folder', async () => {
		expect(await checkDestination(vfsWith(), 'notes.txt', FOLDER)).toEqual({
			state: 'problem',
			message: '“notes.txt” is not a folder.',
		});
	});

	it('says a folder that cannot be written to cannot be', async () => {
		expect(await checkDestination(vfsWith(), 'archive', FOLDER)).toEqual({
			state: 'problem',
			message: '“archive” cannot be written to.',
		});
	});

	it('says text that is not a location is not one, and a kind of location it cannot use', async () => {
		const vfs = vfsWith();
		expect(await checkDestination(vfs, '', FOLDER)).toMatchObject({ state: 'problem' });
		expect(await checkDestination(vfs, '   ', FOLDER)).toEqual({
			state: 'problem',
			message: 'Type a folder, or pick one from the lists.',
		});
		expect(await checkDestination(vfs, 'sftp://host/x', FOLDER)).toEqual({
			state: 'problem',
			message: 'Locations of the kind sftp cannot be used yet.',
		});
	});

	it('says permission was denied, and falls back to a plain sentence for anything else', async () => {
		const vfs = vfsWith();
		vfs.failOpening(fileLocation('/home/test/docs'), {
			kind: 'permissionDenied',
			location: fileLocation('/home/test/docs'),
		});
		expect(await checkDestination(vfs, 'docs', FOLDER)).toEqual({
			state: 'problem',
			message: 'You do not have permission to use “docs”.',
		});
		vfs.failOpening(fileLocation('/home/test/docs'), {
			kind: 'io',
			message: 'boom',
			location: null,
		});
		expect(await checkDestination(vfs, 'docs', FOLDER)).toEqual({
			state: 'problem',
			message: 'Could not check that folder.',
		});
	});

	it('refuses the folder the items are in when told to, and only then', async () => {
		const vfs = vfsWith();
		expect(
			await checkDestination(vfs, '/home/test/', FOLDER, { origin: FOLDER, forbidOrigin: true }),
		).toEqual({ state: 'problem', message: 'The items are already in that folder.' });
		expect(await checkDestination(vfs, '/home/test', FOLDER, { origin: FOLDER })).toMatchObject({
			state: 'ok',
		});
		expect(
			await checkDestination(vfs, 'docs', FOLDER, { origin: FOLDER, forbidOrigin: true }),
		).toMatchObject({ state: 'ok' });
	});
});

describe('recent destinations', () => {
	const at = (path: string) => fileLocation(path);

	function storage(initial?: string) {
		const data = new Map<string, string>();
		if (initial !== undefined) data.set('waypoint.recentDestinations', initial);
		return {
			data,
			getItem: (key: string) => data.get(key) ?? null,
			setItem: (key: string, value: string) => void data.set(key, value),
		};
	}

	it('keeps the newest first and forgets a repeat, however its uri is spelt', () => {
		const recent = createRecentDestinations(storage());
		recent.remember(at('/a'));
		recent.remember(at('/b'));
		recent.remember({ display: '/a/', uri: 'file:///a/' });
		expect(recent.list().map((l) => l.uri)).toEqual(['file:///a/', 'file:///b']);
	});

	it('keeps a limited number, and saves them for the next start of the page', () => {
		const store = storage();
		const recent = createRecentDestinations(store);
		for (let i = 0; i < RECENT_LIMIT + 3; i++) recent.remember(at(`/f${i}`));
		expect(recent.list()).toHaveLength(RECENT_LIMIT);
		expect(recent.list()[0]!.display).toBe(`/f${RECENT_LIMIT + 2}`);
		expect(createRecentDestinations(store).list()).toEqual(recent.list());
	});

	it('starts empty from storage that is empty, damaged, or blocked, and does not fail to save', () => {
		expect(createRecentDestinations(storage()).list()).toEqual([]);
		expect(createRecentDestinations(storage('not json')).list()).toEqual([]);
		expect(createRecentDestinations(storage('{"a":1}')).list()).toEqual([]);
		expect(createRecentDestinations(storage('[1,{"uri":"x"}]')).list()).toEqual([]);
		const blocked = createRecentDestinations({
			getItem: () => {
				throw new Error('blocked');
			},
			setItem: () => {
				throw new Error('blocked');
			},
		});
		expect(blocked.list()).toEqual([]);
		blocked.remember(at('/a'));
		expect(blocked.list()).toHaveLength(1);
		const none = createRecentDestinations(null);
		none.remember(at('/a'));
		expect(none.list()).toHaveLength(1);
	});
});
