// Verifies the file commands: the requests they build, where they are offered, and each command's flow over a fake queue
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { emptySelection, everything, selectIds } from '../browse/selection';
import { FOLDER } from '../test/browseHarness';
import { commandsHarness, select } from '../test/fileCommandsHarness';
import { makeEntry } from '../services/fakeVfsClient';
import {
	canCreateHere,
	commandStates,
	createRequest,
	renameFailureText,
	renameRequest,
	selectionRequest,
	selectionSpec,
	type CommandContext,
} from './fileCommands';

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

describe('the requests', () => {
	it('send a selection as a handle and the ids, never as paths', () => {
		expect(selectionSpec(selectIds([3, 1]))).toEqual({ kind: 'some', ids: [3, 1] });
		expect(selectionSpec(everything)).toEqual({ kind: 'allExcept', ids: [] });
		expect(selectionSpec({ kind: 'allExcept', ids: new Set([5]) })).toEqual({
			kind: 'allExcept',
			ids: [5],
		});
	});

	it('make a duplicate, a trash and a delete of the selection from the window that asked', () => {
		for (const kind of ['duplicate', 'trash', 'delete'] as const) {
			expect(selectionRequest(kind, 7, selectIds([2]), 'main-2')).toEqual({
				kind: { kind },
				sources: { kind: 'selection', handle: 7, spec: { kind: 'some', ids: [2] } },
				destination: null,
				name: null,
				options: { conflict: null, verify: null },
				originWindow: 'main-2',
			});
		}
	});

	it('make a create in the open folder that takes the first free name rather than refusing a clash', () => {
		expect(createRequest('createFolder', FOLDER, 'untitled folder', 'main-1')).toEqual({
			kind: { kind: 'createFolder' },
			sources: { kind: 'locations', locations: [] },
			destination: FOLDER,
			name: 'untitled folder',
			options: { conflict: 'keepBoth', verify: null },
			originWindow: 'main-1',
		});
		expect(createRequest('createFile', FOLDER, 'untitled file', 'main-1').kind).toEqual({
			kind: 'createFile',
		});
	});

	it('make a rename of one entry that refuses a clash (no conflict policy)', () => {
		const request = renameRequest(4, 9, 'new.txt', 'main-1');
		expect(request.kind).toEqual({ kind: 'rename' });
		expect(request.sources).toEqual({
			kind: 'selection',
			handle: 4,
			spec: { kind: 'some', ids: [9] },
		});
		expect(request.name).toBe('new.txt');
		expect(request.options.conflict).toBeNull();
	});
});

describe('where the commands are offered', () => {
	it('shows New in a folder that can be written to', () => {
		expect(canCreateHere(context())).toBe(true);
		const states = commandStates(context());
		expect(states.newFolder).toEqual({ visible: true, enabled: true });
		expect(states.newFile).toEqual({ visible: true, enabled: true });
	});

	it('hides every command that writes in a read-only location, by the listing’s own flag', () => {
		const states = commandStates(context({ readOnly: true }));
		for (const id of [
			'newFolder',
			'newFile',
			'rename',
			'duplicate',
			'moveToTrash',
			'deletePermanently',
		] as const) {
			expect(states[id], id).toEqual({ visible: false, enabled: false });
		}
		// The history is global, so it stays (disabled when there is nothing to do).
		expect(states.undo.visible).toBe(true);
	});

	it('enables the selection commands only with a selection, and Rename only with a focused entry', () => {
		const none = commandStates(context({ selected: 0, focused: false }));
		for (const id of ['rename', 'duplicate', 'moveToTrash', 'deletePermanently'] as const) {
			expect(none[id], id).toEqual({ visible: true, enabled: false });
		}
		expect(none.newFolder.enabled).toBe(true);
		const some = commandStates(context({ selected: 2 }));
		expect(some.duplicate.enabled && some.moveToTrash.enabled && some.rename.enabled).toBe(true);
	});

	it('hides everything where there is no queue or no listing', () => {
		expect(canCreateHere(context({ queue: false }))).toBe(false);
		expect(canCreateHere(context({ listing: false }))).toBe(false);
		expect(commandStates(context({ queue: false })).undo.visible).toBe(false);
	});

	it('enables Undo and Redo by what the history holds', () => {
		const entry = {
			id: 1,
			label: 'Move 3 items to Trash',
			atMs: 0,
			undoable: true,
			redoable: false,
		};
		expect(commandStates(context({ undo: entry })).undo.enabled).toBe(true);
		expect(commandStates(context()).undo.enabled).toBe(false);
		expect(commandStates(context({ redo: entry })).redo.enabled).toBe(true);
	});
});

describe('rename refusals in words', () => {
	it('says a taken name is taken, with the name', () => {
		const error = { kind: 'ops', message: 'x', error: { kind: 'nameInUse', location: FOLDER } };
		expect(renameFailureText(error, 'a.txt')).toBe('A file named “a.txt” already exists.');
	});

	it('gives the reason Rust refused an invalid name for', () => {
		const error = { error: { kind: 'invalidName', name: 'a', reason: 'it is wrong' } };
		expect(renameFailureText(error, 'a')).toBe('That name is not allowed: it is wrong');
	});

	it('falls back to the message of any other failure', () => {
		expect(renameFailureText({ kind: 'queue', message: 'the queue is full' }, 'a')).toBe(
			'Could not rename: the queue is full',
		);
	});
});

describe('New Folder and New File', () => {
	it('create in the open folder under the default name and then put the new entry into inline rename', async () => {
		const h = await commandsHarness();
		const done = h.commands.newFolder();
		const id = await h.finish();
		// The listing's watcher reports the folder a moment after the job ends.
		h.vfs.addEntries(FOLDER, [makeEntry(9, 'untitled folder', { kind: 'directory' })]);
		await done;
		const request = h.fake.calls.find((c) => c[0] === 'submit')![1] as { name: string };
		expect(request.name).toBe('untitled folder');
		expect(id).toBe(1);
		const state = h.session.store.getState();
		expect(state.renaming).toBe(9);
		expect(state.selection).toEqual(selectIds([9]));
		expect(state.scrollRequest).not.toBeNull();
	});

	it('say so, and submit nothing, in a read-only location', async () => {
		const h = await commandsHarness({ readOnly: true });
		await h.commands.newFolder();
		expect(h.fake.calls.some((c) => c[0] === 'submit')).toBe(false);
		expect(h.said).toEqual(['This location cannot be changed.']);
	});

	it('say why when the job fails', async () => {
		const h = await commandsHarness();
		const done = h.commands.newFolder();
		await vi.waitFor(() => expect(h.fake.jobs()).toHaveLength(1));
		const job = h.fake.jobs()[0]!;
		h.fake.start(job.id);
		h.fake.fail(job.id, { kind: 'permissionDenied', location: FOLDER });
		await done;
		expect(h.said).toEqual(['Could not finish: Permission denied for test']);
	});
});

describe('Rename', () => {
	it('puts the focused entry into inline rename, and says so when nothing is focused', async () => {
		const h = await commandsHarness();
		h.commands.rename();
		expect(h.said).toEqual(['Nothing to rename. Move to an item first.']);
		await select(h, 1);
		h.commands.rename();
		expect(h.session.store.getState().renaming).toBe(1);
	});

	it('renames the entry it was given, even where the keyboard is elsewhere', async () => {
		const h = await commandsHarness();
		await select(h, 1);
		h.commands.rename(h.session, h.session.model.entryAt(2)!);
		expect(h.session.store.getState().renaming).toBe(2);
	});

	it('does nothing in a read-only location', async () => {
		const h = await commandsHarness({ readOnly: true });
		await select(h, 1);
		h.commands.rename();
		expect(h.session.store.getState().renaming).toBeNull();
		expect(h.said).toEqual(['This location cannot be changed.']);
	});

	it('plans first, so a clash is reported without a failed job in the queue', async () => {
		const h = await commandsHarness();
		const entry = h.session.model.entryAt(0) ?? (await h.session.model.readRange(0, 1))[0]!;
		vi.spyOn(h.fake, 'plan').mockRejectedValue({
			kind: 'ops',
			message: 'taken',
			error: { kind: 'nameInUse', location: FOLDER },
		});
		const outcome = await h.commands.renameEntry(h.session, entry, 'alpha.txt');
		expect(outcome).toEqual({ ok: false, message: 'A file named “alpha.txt” already exists.' });
		expect(h.fake.jobs()).toHaveLength(0);
	});

	it('submits the rename, waits for it, then selects and focuses the renamed entry', async () => {
		const h = await commandsHarness();
		await h.session.model.readRange(0, 3);
		const entry = h.session.model.entryAt(1)!; // alpha.txt
		const pending = h.commands.renameEntry(h.session, entry, 'omega.txt');
		await h.finish();
		h.vfs.updateEntries(FOLDER, new Map([[entry.id, { name: 'omega.txt' }]]));
		expect(await pending).toEqual({ ok: true });
		const request = h.fake.calls.find((c) => c[0] === 'submit')![1] as {
			kind: { kind: string };
			name: string;
		};
		expect(request.kind.kind).toBe('rename');
		expect(request.name).toBe('omega.txt');
		expect(h.session.store.getState().selection).toEqual(selectIds([entry.id]));
		expect(h.said.at(-1)).toBe('Renamed alpha.txt to omega.txt');
	});

	it('reports a job that fails on a taken name and removes it from the queue', async () => {
		const h = await commandsHarness();
		await h.session.model.readRange(0, 3);
		const entry = h.session.model.entryAt(1)!;
		const pending = h.commands.renameEntry(h.session, entry, 'beta.jpg');
		await vi.waitFor(() => expect(h.fake.jobs()).toHaveLength(1));
		const job = h.fake.jobs()[0]!;
		h.fake.start(job.id);
		h.fake.fail(job.id, { kind: 'nameInUse', location: FOLDER });
		expect(await pending).toEqual({
			ok: false,
			message: 'A file named “beta.jpg” already exists.',
		});
		await vi.waitFor(() => expect(h.fake.jobs()).toHaveLength(0));
	});
});

describe('Duplicate', () => {
	it('duplicates the selection and then selects the entries the listing gained', async () => {
		const h = await commandsHarness();
		await select(h, 1, 2);
		const done = h.commands.duplicate();
		await h.finish(1, 'Duplicate 2 items');
		h.vfs.addEntries(FOLDER, [makeEntry(10, 'alpha (2).txt'), makeEntry(11, 'beta (2).jpg')]);
		await done;
		const request = h.fake.calls.find((c) => c[0] === 'submit')![1] as {
			kind: { kind: string };
			sources: { spec: { ids: number[] } };
		};
		expect(request.kind.kind).toBe('duplicate');
		expect(request.sources.spec.ids.sort()).toEqual([1, 2]);
		expect(h.session.store.getState().selection).toEqual(selectIds([10, 11]));
	});

	it('says so when nothing is selected', async () => {
		const h = await commandsHarness();
		await h.commands.duplicate();
		expect(h.said).toEqual(['Nothing is selected.']);
		expect(h.fake.calls.some((c) => c[0] === 'submit')).toBe(false);
	});
});

describe('Move to Trash', () => {
	it('moves the selection without asking by default (D104)', async () => {
		const h = await commandsHarness();
		await select(h, 1);
		const done = h.commands.moveToTrash();
		await h.finish(1, 'Move “alpha.txt” to Trash');
		await done;
		expect(h.confirms).toEqual([]);
		const request = h.fake.calls.find((c) => c[0] === 'submit')![1] as { kind: { kind: string } };
		expect(request.kind.kind).toBe('trash');
	});

	it('asks first when the setting says to, naming the item, and stops on Cancel', async () => {
		const h = await commandsHarness();
		await h.fake.setSettings({ ...(await h.fake.getSettings()), confirmTrash: true });
		await select(h, 1);
		h.answer.value = false;
		await h.commands.moveToTrash();
		expect(h.confirms).toHaveLength(1);
		expect(h.confirms[0]).toMatchObject({
			title: 'Move to the Trash?',
			message: 'Move “alpha.txt” to the Trash? You can undo this.',
			danger: false,
		});
		expect(h.fake.calls.some((c) => c[0] === 'submit')).toBe(false);
	});

	it('offers Delete Permanently, explaining why, when the Trash is unavailable, and deletes only on Yes', async () => {
		const h = await commandsHarness();
		await select(h, 1);
		const done = h.commands.moveToTrash();
		await vi.waitFor(() => expect(h.fake.jobs()).toHaveLength(1));
		const trash = h.fake.jobs()[0]!;
		h.fake.start(trash.id);
		h.fake.fail(trash.id, { kind: 'trashUnavailable', reason: 'this drive has no trash folder' });
		await vi.waitFor(() => expect(h.confirms).toHaveLength(1));
		expect(h.confirms[0]).toMatchObject({
			title: 'The Trash is not available here',
			danger: true,
			confirmLabel: 'Delete Permanently',
		});
		expect(h.confirms[0]!.message).toContain('this drive has no trash folder');
		expect(h.confirms[0]!.message).toContain('cannot be undone');
		await vi.waitFor(() => expect(h.fake.jobs().some((j) => j.kind.kind === 'delete')).toBe(true));
		const del = h.fake.jobs().find((j) => j.kind.kind === 'delete')!;
		h.fake.start(del.id);
		h.fake.done(del.id);
		await done;
	});

	it('does not delete when the fallback is declined', async () => {
		const h = await commandsHarness();
		await select(h, 1);
		h.answer.value = false;
		const done = h.commands.moveToTrash();
		await vi.waitFor(() => expect(h.fake.jobs()).toHaveLength(1));
		const trash = h.fake.jobs()[0]!;
		h.fake.start(trash.id);
		h.fake.fail(trash.id, { kind: 'trashUnavailable', reason: 'no trash' });
		await done;
		expect(h.fake.jobs().some((j) => j.kind.kind === 'delete')).toBe(false);
	});
});

describe('Delete Permanently', () => {
	it('always asks, lists up to five names, says how many more and the size, and warns it cannot be undone', async () => {
		const entries = Array.from({ length: 8 }, (_, i) =>
			makeEntry(i + 1, `file-${i + 1}.txt`, { size: 1024 }),
		);
		const h = await commandsHarness({ entries });
		await select(h, 0, 1, 2, 3, 4, 5, 6, 7);
		h.answer.value = false;
		await h.commands.deletePermanently();
		expect(h.confirms).toHaveLength(1);
		const [spec] = h.confirms;
		expect(spec).toMatchObject({
			title: 'Delete permanently?',
			message: 'This permanently deletes 8 items. It cannot be undone.',
			confirmLabel: 'Delete Permanently',
			danger: true,
		});
		expect(spec!.items).toHaveLength(5);
		expect(spec!.note).toBe('and 3 more · Total size: 8.2 kB');
		expect(h.fake.calls.some((c) => c[0] === 'submit')).toBe(false);
	});

	it('asks even when the Trash confirmation is off, then submits a delete job', async () => {
		const h = await commandsHarness();
		await select(h, 1);
		const done = h.commands.deletePermanently();
		await h.finish();
		await done;
		expect(h.confirms).toHaveLength(1);
		const request = h.fake.calls.find((c) => c[0] === 'submit')![1] as { kind: { kind: string } };
		expect(request.kind.kind).toBe('delete');
	});

	it('sends exactly what the dialog listed: a file that arrives while it is open is left alone', async () => {
		const h = await commandsHarness();
		await h.session.model.readRange(0, h.session.model.count);
		h.session.store.getState().selectAll();
		h.onConfirm.run = () =>
			h.vfs.addEntries(FOLDER, [makeEntry(9, 'download.iso', { kind: 'file' })]);
		const done = h.commands.deletePermanently();
		await h.finish();
		await done;
		const request = h.fake.calls.find((c) => c[0] === 'submit')![1] as {
			sources: { spec: { kind: string; ids: number[] } };
		};
		expect(request.sources.spec.kind).toBe('some');
		expect([...request.sources.spec.ids].sort()).toEqual([1, 2, 3]);
	});

	it('is not offered where the listing is read-only', async () => {
		const h = await commandsHarness({ readOnly: true });
		await select(h, 0);
		await h.commands.deletePermanently();
		expect(h.confirms).toEqual([]);
		expect(h.said).toEqual(['This location cannot be changed.']);
	});
});

describe('Undo and Redo', () => {
	it('say there is nothing to undo or redo, instead of failing silently', async () => {
		const h = await commandsHarness();
		await h.commands.undo();
		await h.commands.redo();
		expect(h.said).toEqual(['There is nothing to undo.', 'There is nothing to redo.']);
		expect(h.fake.calls.some((c) => c[0] === 'undo' || c[0] === 'redo')).toBe(false);
	});

	it('undo the newest entry, and redo it after', async () => {
		const h = await commandsHarness();
		await select(h, 1);
		const trash = h.commands.moveToTrash();
		await h.finish(1, 'Move “alpha.txt” to Trash');
		await trash;
		await vi.waitFor(() =>
			expect(h.commands.history().undo?.label).toBe('Move “alpha.txt” to Trash'),
		);
		await h.commands.undo();
		expect(h.fake.calls.some((c) => c[0] === 'undo')).toBe(true);
		const undoJob = h.fake.jobs().find((j) => j.kind.kind === 'undo')!;
		h.fake.start(undoJob.id);
		h.fake.done(undoJob.id);
		await vi.waitFor(() => expect(h.commands.history().redo).not.toBeNull());
		await h.commands.redo();
		expect(h.fake.calls.some((c) => c[0] === 'redo')).toBe(true);
	});
});

describe('states of the active pane', () => {
	it('follow the selection and the listing', async () => {
		const h = await commandsHarness();
		expect(h.commands.states().duplicate.enabled).toBe(false);
		await select(h, 1);
		expect(h.commands.states().duplicate.enabled).toBe(true);
		h.session.store.getState().selectAll();
		expect(h.commands.states().moveToTrash.enabled).toBe(true);
		h.session.store.getState().deselectAll();
		expect(h.commands.states().moveToTrash.enabled).toBe(false);
		expect(emptySelection.kind).toBe('some');
	});
});
