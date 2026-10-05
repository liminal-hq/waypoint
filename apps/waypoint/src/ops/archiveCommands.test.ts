// Verifies Extract Here, Extract To…, Extract All and Compress…: where they are offered, the requests they make, and the questions they ask first
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import type { VfsError } from '@liminal-hq/waypoint-protocol/generated/VfsError';
import { describe, expect, it, vi } from 'vitest';
import { FakeArchiveClient } from '../archives/fakeArchiveClient';
import { openListingModel } from '../browse/listingModel';
import { createListingSession } from '../browse/useListingSession';
import type { Answered } from '../connections/connectFlow';
import { makeEntry } from '../services/fakeVfsClient';
import type { JobRequest, Location } from '../services/opsClient';
import { commandsHarness, select, type CommandsHarness } from '../test/fileCommandsHarness';
import { FOLDER } from '../test/browseHarness';
import { createFileCommands, commandStates, type CommandContext } from './fileCommands';
import { compressRequest, extractRequest, leftOutCount, limitOf } from './archiveRequests';
import { nameProblem } from './CompressDialog';

const context = (overrides: Partial<CommandContext> = {}): CommandContext => ({
	queue: true,
	listing: true,
	readOnly: false,
	selected: 1,
	focused: true,
	undo: null,
	redo: null,
	...overrides,
});

const ARCHIVE_ENTRIES = [
	makeEntry(1, 'photos.zip'),
	makeEntry(2, 'notes.txt'),
	makeEntry(3, 'src.tar.gz'),
];

const lock: VfsError = {
	kind: 'authRequired',
	location: { display: '/home/test/photos.zip', uri: 'archive:file:///home/test/photos.zip!/' },
	prompt: { kind: 'passphrase', subject: 'photos.zip' },
};

/** What the person answers the password question with. */
const password = (text: string): Answered => ({
	answer: { kind: 'passphrase', passphrase: text },
	remember: false,
});

interface Rig {
	h: CommandsHarness;
	archives: FakeArchiveClient;
	asked: VfsError[];
	answers: Array<Answered | null>;
	destinations: Array<Location | null>;
	compressions: Array<{ name: string; format: 'zip' | 'tarGz' } | null>;
	commands: ReturnType<typeof createFileCommands>;
}

async function rig(entries = ARCHIVE_ENTRIES): Promise<Rig> {
	const h = await commandsHarness({ entries });
	const archives = new FakeArchiveClient();
	const asked: VfsError[] = [];
	const answers: Array<Answered | null> = [];
	const destinations: Array<Location | null> = [];
	const compressions: Rig['compressions'] = [];
	const commands = createFileCommands({
		ops: h.ops,
		vfs: h.vfs,
		windowLabel: 'main-1',
		activeSession: () => h.session,
		confirm: async (spec) => {
			h.confirms.push(spec);
			return h.answer.value;
		},
		say: (text) => h.said.push(text),
		archives,
		askQuestion: async (error) => {
			asked.push(error);
			return answers.shift() ?? null;
		},
		pickDestination: async () => destinations.shift() ?? null,
		pickCompression: async () => compressions.shift() ?? null,
	});
	return { h, archives, asked, answers, destinations, compressions, commands };
}

const submitted = (h: CommandsHarness): JobRequest[] =>
	h.fake.calls.filter((call) => call[0] === 'submit').map((call) => call[1] as JobRequest);

describe('the requests', () => {
	it('extract the archives into a folder, or beside each when there is none', () => {
		const a: Location = { display: '/a.zip', uri: 'file:///a.zip' };
		expect(extractRequest([a], null, 'main-3')).toEqual({
			kind: { kind: 'extract' },
			sources: { kind: 'locations', locations: [a] },
			destination: null,
			name: null,
			options: { conflict: null, verify: null },
			originWindow: 'main-3',
			archive: { kind: 'extract', layout: 'auto', allowLarge: false },
		});
		const request = extractRequest([a], FOLDER, 'main-3', { layout: 'folder', allowLarge: true });
		expect(request.destination).toEqual(FOLDER);
		expect(request.archive).toEqual({ kind: 'extract', layout: 'folder', allowLarge: true });
	});

	it('compress the items into a named archive of a format, in the open folder', () => {
		const items: Location[] = [{ display: '/a', uri: 'file:///a' }];
		expect(compressRequest(items, FOLDER, 'a.tar.gz', 'tarGz', 'main-1')).toEqual({
			kind: { kind: 'compress' },
			sources: { kind: 'locations', locations: items },
			destination: FOLDER,
			name: 'a.tar.gz',
			options: { conflict: null, verify: null },
			originWindow: 'main-1',
			archive: { kind: 'compress', format: 'tarGz' },
		});
	});

	it('count what planning left out and find the limit that stopped it', () => {
		const at: Location = { display: '', uri: '' };
		expect(
			leftOutCount({
				notes: [
					{ kind: 'leftOut', location: at, why: 'traversal' },
					{ kind: 'emptyArchive', location: at },
					{ kind: 'leftOut', location: at, why: 'linkOutside' },
				],
			}),
		).toBe(2);
		const limit: OpsError = {
			kind: 'archiveLimit',
			location: at,
			limit: { kind: 'entries', found: 5, max: 2 },
		};
		expect(limitOf({ kind: 'ops', message: '', error: limit })).toEqual(limit);
		expect(limitOf(limit)).toEqual(limit);
		expect(limitOf({ kind: 'ops', error: { kind: 'io', message: 'x' } })).toBeNull();
	});

	it('refuse a name with nothing in it or a slash', () => {
		expect(nameProblem('')).toBe('compress.error.empty');
		expect(nameProblem('   ')).toBe('compress.error.empty');
		expect(nameProblem('a/b')).toBe('compress.error.invalid');
		expect(nameProblem('a\\b')).toBe('compress.error.invalid');
		expect(nameProblem('photos')).toBeNull();
	});
});

describe('where the commands are offered', () => {
	it('offers Extract on an archive, Here where it can be written and To… where it can be read', () => {
		const onArchive = commandStates(context({ archive: true }));
		expect(onArchive.extractHere).toEqual({ visible: true, enabled: true });
		expect(onArchive.extractTo).toEqual({ visible: true, enabled: true });
		const readOnly = commandStates(context({ archive: true, readOnly: true }));
		expect(readOnly.extractHere.visible).toBe(false);
		expect(readOnly.extractTo.visible).toBe(true);
		const notArchive = commandStates(context());
		expect(notArchive.extractHere.visible).toBe(false);
		expect(notArchive.extractTo.visible).toBe(false);
	});

	it('offers Extract All only inside an archive', () => {
		expect(commandStates(context()).extractAll.visible).toBe(false);
		expect(commandStates(context({ inArchive: true, readOnly: true })).extractAll).toEqual({
			visible: true,
			enabled: true,
		});
	});

	it('offers Compress where things are written and something is selected', () => {
		expect(commandStates(context()).compress).toEqual({ visible: true, enabled: true });
		expect(commandStates(context({ selected: 0 })).compress).toEqual({
			visible: true,
			enabled: false,
		});
		expect(commandStates(context({ readOnly: true })).compress.visible).toBe(false);
	});

	it('knows an archive is under the keyboard from the listing', async () => {
		const { h, commands } = await rig();
		await select(h, 1);
		expect(commands.states().extractHere.visible).toBe(true);
		await select(h, 0);
		expect(commands.states().extractHere.visible).toBe(false);
	});
});

describe('Extract Here and Extract To…', () => {
	it('extracts only the archives of a mixed selection, beside themselves, answering to the queue', async () => {
		const { h, commands } = await rig();
		await select(h, 0, 1, 2);
		const done = commands.extractHere();
		await vi.waitFor(() => expect(submitted(h)).toHaveLength(1));
		const [request] = submitted(h);
		expect(request?.kind).toEqual({ kind: 'extract' });
		expect(request?.destination).toBeNull();
		expect(request?.sources).toEqual({
			kind: 'locations',
			locations: [
				{ display: '/home/test/photos.zip', uri: 'file:///home/test/photos.zip' },
				{ display: '/home/test/src.tar.gz', uri: 'file:///home/test/src.tar.gz' },
			],
		});
		await h.finish();
		await done;
	});

	it('says so, and submits nothing, when none of the selection is an archive', async () => {
		const { h, commands } = await rig();
		await select(h, 0);
		await commands.extractHere();
		expect(submitted(h)).toEqual([]);
		expect(h.said).toEqual(['None of the selected items is an archive.']);
	});

	it('asks where to, and extracts there', async () => {
		const { h, commands, destinations } = await rig();
		const destination: Location = { display: '/out', uri: 'file:///out' };
		destinations.push(destination);
		await select(h, 1);
		const done = commands.extractTo();
		await vi.waitFor(() => expect(submitted(h)).toHaveLength(1));
		expect(submitted(h)[0]?.destination).toEqual(destination);
		await h.finish();
		await done;
	});

	it('does nothing when the destination question is cancelled', async () => {
		const { h, commands } = await rig();
		await select(h, 1);
		await commands.extractTo();
		expect(submitted(h)).toEqual([]);
	});

	it('mentions the entries planning left out', async () => {
		const { h, commands } = await rig();
		vi.spyOn(h.fake, 'plan').mockResolvedValue({
			kind: { kind: 'extract' },
			sources: { count: 1, first: 'photos.zip' },
			items: 3,
			bytes: 0,
			sameVolume: true,
			conflicts: [],
			notes: [
				{ kind: 'leftOut', location: { display: '', uri: '' }, why: 'traversal' },
				{ kind: 'leftOut', location: { display: '', uri: '' }, why: 'absolute' },
			],
		});
		await select(h, 1);
		const done = commands.extractHere();
		await h.finish();
		await done;
		expect(h.said).toContain(
			'2 entries of the archive were left out: their names or links were unsafe.',
		);
	});
});

describe('an archive past the limits', () => {
	const limit = (location: Location): OpsError => ({
		kind: 'archiveLimit',
		location,
		limit: { kind: 'ratio', ratio: 5000, max: 1000 },
	});

	it('asks first, saying which limit, and goes ahead only when told to', async () => {
		const { h, commands } = await rig();
		const refuse = vi
			.spyOn(h.fake, 'plan')
			.mockRejectedValueOnce({ kind: 'ops', message: '', error: limit(FOLDER) });
		await select(h, 1);
		const done = commands.extractHere();
		await vi.waitFor(() => expect(h.confirms).toHaveLength(1));
		expect(h.confirms[0]?.message).toContain('expands to 5000 times its own size');
		expect(h.confirms[0]?.danger).toBe(true);
		await vi.waitFor(() => expect(submitted(h)).toHaveLength(1));
		expect(submitted(h)[0]?.archive).toEqual({ kind: 'extract', layout: 'auto', allowLarge: true });
		expect(refuse).toHaveBeenCalledTimes(2);
		await h.finish();
		await done;
	});

	it('submits nothing when the person declines', async () => {
		const { h, commands } = await rig();
		vi.spyOn(h.fake, 'plan').mockRejectedValue({
			kind: 'ops',
			message: '',
			error: limit(FOLDER),
		});
		h.answer.value = false;
		await select(h, 1);
		await commands.extractHere();
		expect(h.confirms).toHaveLength(1);
		expect(submitted(h)).toEqual([]);
	});
});

describe('an encrypted archive', () => {
	const refused = { kind: 'ops', message: '', error: { kind: 'connection', error: lock } };

	it('asks for the password, gives it to Rust and then plans and extracts', async () => {
		const { h, commands, archives, asked, answers } = await rig();
		vi.spyOn(h.fake, 'plan').mockRejectedValueOnce(refused);
		answers.push(password('s3cret'));
		await select(h, 1);
		const done = commands.extractHere();
		await vi.waitFor(() => expect(submitted(h)).toHaveLength(1));
		expect(asked).toHaveLength(1);
		expect(asked[0]).toMatchObject({ kind: 'authRequired', prompt: { kind: 'passphrase' } });
		expect(archives.unlocked).toEqual([
			{ location: lock.kind === 'authRequired' ? lock.location : null, passphrase: 's3cret' },
		]);
		await h.finish();
		await done;
	});

	it('asks again when a job fails for want of the password, and tries the job again', async () => {
		const { h, commands, archives, answers } = await rig();
		answers.push(password('right'));
		await select(h, 1);
		const done = commands.extractHere();
		await vi.waitFor(() => expect(h.fake.jobs()).toHaveLength(1));
		const first = h.fake.jobs()[0]!;
		h.fake.start(first.id);
		h.fake.fail(first.id, { kind: 'connection', error: lock });
		await vi.waitFor(() => expect(submitted(h)).toHaveLength(2));
		expect(archives.unlocked.map((u) => u.passphrase)).toEqual(['right']);
		// The failed job was taken off the queue before the password was asked for.
		await vi.waitFor(() => expect(h.fake.jobs().some((job) => job.id !== first.id)).toBe(true));
		const second = h.fake.jobs().find((job) => job.id !== first.id)!;
		expect(h.fake.jobs().some((job) => job.id === first.id)).toBe(false);
		h.fake.start(second.id);
		h.fake.done(second.id);
		await done;
		expect(h.said).toEqual([]);
	});

	it('does nothing more when the question is cancelled', async () => {
		const { h, commands, archives } = await rig();
		vi.spyOn(h.fake, 'plan').mockRejectedValue(refused);
		await select(h, 1);
		await commands.extractHere();
		expect(archives.unlocked).toEqual([]);
		expect(submitted(h)).toEqual([]);
	});

	it('says why when the password could not be given', async () => {
		const { h, commands, archives, answers } = await rig();
		vi.spyOn(h.fake, 'plan').mockRejectedValue(refused);
		archives.failNext = new Error('the command is gone');
		answers.push(password('x'));
		await select(h, 1);
		await commands.extractHere();
		expect(h.said).toEqual(['The archive’s password could not be given: the command is gone']);
		expect(submitted(h)).toEqual([]);
	});

	it('stops asking after a few wrong passwords', async () => {
		const { h, commands, answers, asked } = await rig();
		vi.spyOn(h.fake, 'plan').mockRejectedValue(refused);
		for (let n = 0; n < 6; n++) answers.push(password(`wrong ${n}`));
		await select(h, 1);
		await commands.extractHere();
		expect(asked.length).toBeLessThanOrEqual(4);
		expect(h.said.at(-1)).toBe('The archive is still locked.');
		expect(submitted(h)).toEqual([]);
	});
});

describe('Extract All', () => {
	it('extracts the whole archive the pane is in, to the folder chosen', async () => {
		const h = await commandsHarness();
		const insideUri = 'archive:file:///home/test/pack.zip!/docs';
		const inside: Location = { display: '/home/test/pack.zip › docs', uri: insideUri };
		h.vfs.setFolder(inside, [makeEntry(1, 'readme.md')]);
		const session = createListingSession(await openListingModel(h.vfs, inside));
		const file: Location = { display: '/home/test/pack.zip', uri: 'file:///home/test/pack.zip' };
		const beside: Location = { display: '/home/test', uri: 'file:///home/test' };
		vi.spyOn(h.vfs, 'parseLocation').mockImplementation(async (input) =>
			input === '..' ? beside : file,
		);
		const chosen: Location = { display: '/out', uri: 'file:///out' };
		let options: { base?: Location; initial?: Location; title?: string } = {};
		const commands = createFileCommands({
			ops: h.ops,
			vfs: h.vfs,
			windowLabel: 'main-1',
			activeSession: () => session,
			confirm: async () => true,
			say: (text) => h.said.push(text),
			pickDestination: async (asked) => {
				options = asked;
				return chosen;
			},
		});
		expect(commands.states(session).extractAll.visible).toBe(true);
		const done = commands.extractAll(session);
		await vi.waitFor(() => expect(submitted(h)).toHaveLength(1));
		const [request] = submitted(h);
		expect(request?.sources).toEqual({ kind: 'locations', locations: [file] });
		expect(request?.destination).toEqual(chosen);
		expect(options.initial).toEqual(beside);
		expect(options.title).toBe('Extract pack.zip to…');
		await h.finish();
		await done;
	});
});

describe('Compress…', () => {
	it('names the archive after the one item, and makes it in the open folder', async () => {
		const { h, commands, compressions } = await rig();
		compressions.push({ name: 'photos.tar.gz', format: 'tarGz' });
		await select(h, 1);
		const seen = vi.fn();
		const asked = createFileCommands({
			ops: h.ops,
			vfs: h.vfs,
			windowLabel: 'main-1',
			activeSession: () => h.session,
			confirm: async () => true,
			say: (text) => h.said.push(text),
			pickCompression: async (options) => {
				seen(options);
				return compressions.shift() ?? null;
			},
		});
		const done = asked.compress();
		await vi.waitFor(() => expect(submitted(h)).toHaveLength(1));
		expect(seen).toHaveBeenCalledWith({ name: 'photos', count: 1 });
		const [request] = submitted(h);
		expect(request?.kind).toEqual({ kind: 'compress' });
		expect(request?.name).toBe('photos.tar.gz');
		expect(request?.destination).toEqual(FOLDER);
		expect(request?.archive).toEqual({ kind: 'compress', format: 'tarGz' });
		await h.finish();
		await done;
		void commands;
	});

	it('starts from "Archive" for several items, and sends them all', async () => {
		const { h, commands, compressions } = await rig();
		compressions.push({ name: 'Archive.zip', format: 'zip' });
		await select(h, 0, 1);
		const done = commands.compress();
		await vi.waitFor(() => expect(submitted(h)).toHaveLength(1));
		const sources = submitted(h)[0]?.sources;
		expect(sources?.kind === 'locations' && sources.locations.length).toBe(2);
		await h.finish();
		await done;
	});

	it('does nothing when the dialog is cancelled, and says so with nothing selected', async () => {
		const { h, commands } = await rig();
		await commands.compress();
		expect(h.said).toEqual(['Select something to compress first.']);
		await select(h, 1);
		await commands.compress();
		expect(submitted(h)).toEqual([]);
	});

	it('is refused in a read-only folder', async () => {
		const h = await commandsHarness({ entries: ARCHIVE_ENTRIES, readOnly: true });
		await select(h, 1);
		await h.commands.compress();
		expect(submitted(h)).toEqual([]);
		expect(h.said).toEqual(['This location cannot be changed.']);
	});
});
