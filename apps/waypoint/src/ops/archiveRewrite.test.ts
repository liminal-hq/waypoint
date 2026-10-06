// Verifies changes inside an open archive: what is offered, the question that says the archive is rewritten and how large it is, and the words for each refusal
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { ArchiveWriteRefusal } from '@liminal-hq/waypoint-protocol/generated/ArchiveWriteRefusal';
import type { OpsError } from '@liminal-hq/waypoint-protocol/generated/OpsError';
import type { PlanPreview } from '@liminal-hq/waypoint-protocol/generated/PlanPreview';
import { describe, expect, it, vi } from 'vitest';
import { makeEntry } from '../services/fakeVfsClient';
import type { JobRequest, Location } from '../services/opsClient';
import { commandsHarness, select, type CommandsHarness } from '../test/fileCommandsHarness';
import { archiveRefusalText } from './archiveRefusal';
import { addConfirm, confirmIfForGood, forGoodConfirm, rewriteConfirm } from './archiveRewrite';
import { addRequest } from './archiveRequests';
import { commandStates, type CommandContext } from './fileCommands';
import { errorText } from './jobText';
import { problemText } from './problemModel';

const ZIP: Location = { display: '/home/test/pack.zip', uri: 'file:///home/test/pack.zip' };
const TOP: Location = {
	display: '/home/test/pack.zip › ',
	uri: 'archive:file:///home/test/pack.zip!/',
};

const context = (overrides: Partial<CommandContext> = {}): CommandContext => ({
	queue: true,
	listing: true,
	readOnly: false,
	selected: 1,
	focused: true,
	undo: null,
	redo: null,
	inArchive: true,
	...overrides,
});

const preview = (archive: PlanPreview['archive']): PlanPreview => ({
	kind: { kind: 'delete' },
	sources: { count: 1, first: 'a.txt' },
	items: 3,
	bytes: 0,
	sameVolume: true,
	conflicts: [],
	notes: [],
	ends: { from: [], to: null },
	...(archive ? { archive } : {}),
});

const REWRITES = preview({ container: ZIP, size: 24_000_000, entries: 3, undo: { kind: 'trash' } });

async function inArchive(): Promise<CommandsHarness> {
	return commandsHarness({
		folder: TOP,
		rewritable: true,
		entries: [
			makeEntry(1, 'a.txt'),
			makeEntry(2, 'b.txt'),
			makeEntry(3, 'docs', { kind: 'directory' }),
		],
	});
}

const submitted = (h: CommandsHarness): JobRequest[] =>
	h.fake.calls.filter((call) => call[0] === 'submit').map((call) => call[1] as JobRequest);

describe('what an archive that can be written offers', () => {
	it('lets entries be made, renamed, deleted and pasted, and nothing that would move them', () => {
		const states = commandStates(context());
		for (const id of [
			'newFolder',
			'newFile',
			'rename',
			'moveToTrash',
			'deletePermanently',
			'paste',
		] as const) {
			expect(states[id].visible, id).toBe(true);
		}
		for (const id of ['duplicate', 'cut', 'moveTo', 'compress', 'extractHere'] as const) {
			expect(states[id].visible, id).toBe(false);
		}
		// Copying out of it, and Extract All, still read.
		expect(states.copy.visible).toBe(true);
		expect(states.extractAll.visible).toBe(true);
	});

	it('is not a read-only listing once the provider says it can be rewritten', async () => {
		const h = await inArchive();
		expect(h.session.model.readOnly).toBe(true);
		expect(h.session.model.rewritable).toBe(true);
		expect(h.session.model.blocksWrites).toBe(false);
		const plain = await commandsHarness({ readOnly: true });
		expect(plain.session.model.blocksWrites).toBe(true);
	});
});

describe('the question before a change that rewrites the archive', () => {
	it('says which archive, how large it is, and that Undo brings the old one back', () => {
		const spec = rewriteConfirm(REWRITES, { kind: 'delete', names: ['a.txt'], count: 1 });
		expect(spec.message).toContain('“pack.zip”');
		expect(spec.message).toContain('24 MB');
		expect(spec.message).toContain('Trash');
		expect(spec.message).toContain('“a.txt”');
		expect(spec.danger).toBe(true);
		const many = rewriteConfirm(REWRITES, { kind: 'delete', names: ['a', 'b'], count: 2 });
		expect(many.message).toContain('2 items');
		expect(many.items).toEqual(['a', 'b']);
		const rename = rewriteConfirm(REWRITES, { kind: 'rename', name: 'a.txt', newName: 'z.txt' });
		expect(rename.message).toContain('“a.txt” to “z.txt”');
		expect(rename.danger).toBe(false);
		expect(addConfirm(REWRITES, '2 items').message).toContain('already exists');
	});

	it('tells the truth when the old archive cannot go to the Trash', () => {
		const remote = preview({ container: ZIP, size: 1500, entries: 3, undo: { kind: 'remote' } });
		const spec = rewriteConfirm(remote, { kind: 'delete', names: ['a.txt'], count: 1 });
		expect(spec.message).toContain('replaced for good');
		expect(spec.message).toContain('Undo is not available');
		expect(spec.message).not.toContain('Undo brings it back');
		const unavailable = preview({
			container: ZIP,
			size: 1500,
			entries: 3,
			undo: { kind: 'unavailable', reason: 'cannot create /home/.Trash-1000' },
		});
		const rename = rewriteConfirm(unavailable, { kind: 'rename', name: 'a', newName: 'b' });
		expect(rename.message).toContain(
			'The Trash is not available (cannot create /home/.Trash-1000)',
		);
		expect(addConfirm(unavailable, '2 items').message).toContain('Undo is not available');
		expect(forGoodConfirm(remote)).toMatchObject({
			danger: true,
			confirmLabel: 'Replace for good',
		});
	});

	it('asks before a paste adds to an archive that would be replaced for good, and not otherwise', async () => {
		const forGood = preview({ container: ZIP, size: 1500, entries: 3, undo: { kind: 'remote' } });
		const kept = preview({ container: ZIP, size: 1500, entries: 3, undo: { kind: 'trash' } });
		for (const [found, asked] of [
			[forGood, 1],
			[kept, 0],
		] as const) {
			const h = await inArchive();
			vi.spyOn(h.fake, 'plan').mockResolvedValue(found);
			const request: JobRequest = {
				kind: { kind: 'copy' },
				sources: { kind: 'locations', locations: [ZIP] },
				destination: TOP,
				name: null,
				options: { conflict: null, verify: null },
				originWindow: 'main-1',
			};
			const questions: string[] = [];
			const go = await confirmIfForGood(
				{
					ops: h.ops,
					confirm: async (spec) => {
						questions.push(spec.title);
						return true;
					},
				},
				request,
			);
			expect(go).toBe(true);
			expect(questions).toEqual(asked > 0 ? ['Replace the archive for good?'] : []);
		}
	});

	it('is asked for Delete and Move to Trash alike, and the delete goes ahead once confirmed', async () => {
		for (const command of ['deletePermanently', 'moveToTrash'] as const) {
			const h = await inArchive();
			vi.spyOn(h.fake, 'plan').mockResolvedValue(REWRITES);
			await select(h, 1);
			const done = h.commands[command]();
			await vi.waitFor(() => expect(submitted(h)).toHaveLength(1));
			expect(h.confirms).toHaveLength(1);
			expect(h.confirms[0]?.message).toContain('24 MB');
			expect(submitted(h)[0]?.kind).toEqual({ kind: 'delete' });
			await h.finish();
			await done;
		}
	});

	it('submits nothing when the person declines', async () => {
		const h = await inArchive();
		vi.spyOn(h.fake, 'plan').mockResolvedValue(REWRITES);
		h.answer.value = false;
		await select(h, 1);
		await h.commands.deletePermanently();
		expect(h.confirms).toHaveLength(1);
		expect(submitted(h)).toEqual([]);
	});

	it('asks about the size limits first, and goes ahead with allowLarge once told to', async () => {
		const h = await inArchive();
		const limit: OpsError = {
			kind: 'archiveLimit',
			location: ZIP,
			limit: { kind: 'ratio', ratio: 5000, max: 1000 },
		};
		const plan = vi
			.spyOn(h.fake, 'plan')
			.mockRejectedValueOnce({ kind: 'ops', message: '', error: limit })
			.mockResolvedValue(REWRITES);
		await select(h, 1);
		const done = h.commands.deletePermanently();
		await vi.waitFor(() => expect(submitted(h)).toHaveLength(1));
		expect(h.confirms).toHaveLength(2);
		expect(h.confirms[0]?.message).toContain('expands to 5,000 times its own size');
		expect(submitted(h)[0]?.archive).toEqual({ kind: 'edit', allowLarge: true });
		expect(plan).toHaveBeenCalledTimes(2);
		await h.finish();
		await done;
	});

	it('says why an archive that cannot be changed was refused, and submits nothing', async () => {
		const h = await inArchive();
		vi.spyOn(h.fake, 'plan').mockRejectedValue({
			kind: 'ops',
			message: '',
			error: { kind: 'archiveNotWritable', location: ZIP, reason: { kind: 'encrypted' } },
		});
		await select(h, 1);
		await h.commands.deletePermanently();
		expect(h.said.join(' ')).toContain('pack.zip is encrypted');
		expect(h.confirms).toEqual([]);
		expect(submitted(h)).toEqual([]);
	});

	it('is asked before a rename inside the archive, and a declined one is said under the field', async () => {
		const h = await inArchive();
		vi.spyOn(h.fake, 'plan').mockResolvedValue(
			preview({ container: ZIP, size: 1500, entries: 3, undo: { kind: 'trash' } }),
		);
		const [entry] = await h.session.model.readRange(0, 1);
		const renaming = h.commands.renameEntry(h.session, entry!, 'z.txt');
		await vi.waitFor(() => expect(submitted(h)).toHaveLength(1));
		await h.finish();
		expect(await renaming).toEqual({ ok: true });
		expect(h.confirms[0]?.message).toContain('“docs” to “z.txt”');
		expect(h.confirms[0]?.message).toContain('1.5 kB');
		h.answer.value = false;
		const declined = await h.commands.renameEntry(h.session, entry!, 'y.txt');
		expect(declined).toEqual({ ok: false, message: 'The rename was not confirmed.' });
	});
});

describe('adding to an archive', () => {
	it('is a copy whose destination is the top of the archive file', () => {
		const items: Location[] = [{ display: '/a', uri: 'file:///a' }];
		expect(addRequest(items, ZIP, 'main-1')).toEqual({
			kind: { kind: 'copy' },
			sources: { kind: 'locations', locations: items },
			destination: { display: ZIP.display, uri: 'archive:file:///home/test/pack.zip!/' },
			name: null,
			options: { conflict: null, verify: null },
			originWindow: 'main-1',
		});
	});

	it('is offered by Compress… when the name chosen is an archive that is already there', async () => {
		const h = await commandsHarness({
			entries: [makeEntry(1, 'pack.zip'), makeEntry(2, 'notes.txt')],
		});
		const commands = (await import('./fileCommands')).createFileCommands({
			ops: h.ops,
			vfs: h.vfs,
			windowLabel: 'main-1',
			activeSession: () => h.session,
			confirm: async (spec) => {
				h.confirms.push(spec);
				return h.answer.value;
			},
			say: (text) => h.said.push(text),
			pickCompression: async () => ({ name: 'pack', format: 'zip' }),
		});
		const taken = {
			...preview(undefined),
			kind: { kind: 'compress' } as const,
			conflicts: [
				{
					source: { display: '/home/test/notes.txt', uri: 'file:///home/test/notes.txt' },
					existing: ZIP,
					name: 'pack.zip',
					kind: 'fileOverFile' as const,
					withinBatch: false,
					sourceSize: null,
					existingSize: null,
					sourceModified: null,
					existingModified: null,
				},
			],
		} as unknown as PlanPreview;
		vi.spyOn(h.fake, 'plan').mockImplementation(async (request) =>
			request.kind.kind === 'compress' ? taken : REWRITES,
		);
		await select(h, 0);
		const done = commands.compress();
		await vi.waitFor(() => expect(submitted(h)).toHaveLength(1));
		expect(h.confirms[0]?.title).toBe('Add to the existing archive?');
		expect(submitted(h)[0]?.kind).toEqual({ kind: 'copy' });
		expect(submitted(h)[0]?.destination?.uri).toBe('archive:file:///home/test/pack.zip!/');
		await h.finish();
		await done;
	});
});

describe('the words for a refusal', () => {
	const location: Location = ZIP;
	const reasons: Array<[ArchiveWriteRefusal, string]> = [
		[
			{ kind: 'readOnlyFormat', format: 'tar.zst' },
			'pack.zip is a tar.zst archive, which can be read but not changed',
		],
		[{ kind: 'encrypted' }, 'pack.zip is encrypted, so it cannot be changed'],
		[{ kind: 'nested' }, 'pack.zip is inside another archive, so it cannot be changed'],
		[
			{ kind: 'unsafeNames' },
			'pack.zip holds names that were changed to be shown safely, so changing it would rename them',
		],
		[
			{ kind: 'noAtomicReplace' },
			'Where pack.zip is kept, a file cannot be replaced in one step, so it cannot be changed',
		],
		[
			{ kind: 'containerReadOnly' },
			'Where pack.zip is kept cannot be written to, so it cannot be changed',
		],
	];

	it.each(reasons)('says %j', (reason, words) => {
		expect(archiveRefusalText(location, reason)).toBe(words);
		const error: OpsError = { kind: 'archiveNotWritable', location, reason };
		expect(errorText(error)).toBe(words);
		expect(problemText(error).message).toBe(words);
	});
});
