// Verifies Cut, Copy, Paste, Copy To…, Move To… and F5 / Shift+F5: where they are offered, the jobs they make and what they refuse
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { FOLDER } from '../test/browseHarness';
import {
	child,
	clipboardHarness,
	OTHER,
	selectIn,
	type ClipboardHarness,
} from '../test/clipboardHarness';
import { cutNames } from './clipboardRules';
import { commandStates, type CommandContext } from './fileCommands';

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

const submits = (h: ClipboardHarness) => h.fake.calls.filter((c) => c[0] === 'submit');

describe('where the clipboard commands are offered', () => {
	it('offers every one in a folder that can be written to, with a selection and something to paste', () => {
		const states = commandStates(
			context({ selected: 2, clipboardItems: 3, paired: true, otherPaneWritable: true }),
		);
		for (const id of [
			'cut',
			'copy',
			'paste',
			'pasteInto',
			'copyTo',
			'moveTo',
			'copyToOtherPane',
			'moveToOtherPane',
		] as const) {
			expect(states[id], id).toEqual({ visible: true, enabled: true });
		}
	});

	it('disables Paste while the clipboard is empty and the selection commands without a selection', () => {
		const states = commandStates(context({ selected: 0, clipboardItems: 0 }));
		expect(states.paste).toEqual({ visible: true, enabled: false });
		expect(states.pasteInto.enabled).toBe(false);
		for (const id of ['cut', 'copy', 'copyTo', 'moveTo'] as const) {
			expect(states[id], id).toEqual({ visible: true, enabled: false });
		}
	});

	it('hides Cut, Paste and Move To… in a read-only location but keeps Copy and Copy To…', () => {
		const states = commandStates(context({ readOnly: true, clipboardItems: 2 }));
		for (const id of ['cut', 'paste', 'pasteInto', 'moveTo', 'moveToOtherPane'] as const) {
			expect(states[id], id).toEqual({ visible: false, enabled: false });
		}
		expect(states.copy).toEqual({ visible: true, enabled: true });
		expect(states.copyTo).toEqual({ visible: true, enabled: true });
	});

	it('copies nothing out of the Trash', () => {
		const states = commandStates(context({ readOnly: true, trash: true }));
		for (const id of ['cut', 'copy', 'copyTo', 'paste'] as const) {
			expect(states[id].visible, id).toBe(false);
		}
	});

	it('shows the other-pane commands only in a pair, and disables them when the other pane cannot be written to', () => {
		const alone = commandStates(context());
		expect(alone.copyToOtherPane.visible).toBe(false);
		expect(alone.moveToOtherPane.visible).toBe(false);
		const locked = commandStates(context({ paired: true, otherPaneWritable: false }));
		expect(locked.copyToOtherPane).toEqual({ visible: true, enabled: false });
		expect(locked.moveToOtherPane).toEqual({ visible: true, enabled: false });
	});

	it('hides everything without a queue', () => {
		const states = commandStates(context({ queue: false, clipboardItems: 1 }));
		for (const id of ['cut', 'copy', 'paste', 'copyTo', 'moveTo'] as const) {
			expect(states[id].visible, id).toBe(false);
		}
	});
});

describe('Copy and Cut', () => {
	it('put the selection on the clipboard and say how many items', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 1, 2);
		await h.commands.copy();
		expect(h.said).toEqual(['Copied 2 items']);
		expect(h.clipboard.store.getState().clipboard).toMatchObject({ mode: 'copy', source: 'app' });
		expect(h.clipboard.store.getState().clipboard.items).toEqual([
			child(FOLDER, 'alpha.txt'),
			child(FOLDER, 'beta.jpg'),
		]);
		await selectIn(h.session, 0);
		await h.commands.cut();
		expect(h.said).toEqual(['Copied 2 items', 'Cut 1 item']);
		expect(h.clipboard.store.getState().clipboard.mode).toBe('cut');
	});

	it('hand the selection to Rust as a handle and a spec, never as paths', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 1, 0);
		await h.commands.copy();
		const call = h.fake.calls.find((c) => c[0] === 'setClipboardFromSelection')!;
		expect(call.slice(1)).toEqual([h.session.model.handle, { kind: 'some', ids: [1, 3] }, 'copy']);
	});

	it('also put the files on the system clipboard, a cut as a cut', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 1);
		await h.commands.copy();
		expect(h.os.files).toEqual({ uris: [child(FOLDER, 'alpha.txt').uri], cut: false });
		await h.commands.cut();
		expect(h.os.files?.cut).toBe(true);
	});

	it('dim what a cut holds until the clipboard changes', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 1, 2);
		await h.commands.cut();
		const names = () => cutNames(h.clipboard.store.getState().clipboard, FOLDER.uri);
		expect([...names()].sort()).toEqual(['alpha.txt', 'beta.jpg']);
		await h.commands.copy();
		expect(names().size).toBe(0);
	});

	it('say so when nothing is selected, and change nothing', async () => {
		const h = await clipboardHarness();
		await h.commands.copy();
		await h.commands.cut();
		expect(h.said).toEqual(['Nothing is selected.', 'Nothing is selected.']);
		expect(h.clipboard.store.getState().clipboard.revision).toBe(0);
	});

	it('Copy works in a read-only location and Cut is refused there', async () => {
		const h = await clipboardHarness({ readOnly: true });
		await selectIn(h.session, 1);
		await h.commands.copy();
		expect(h.clipboard.store.getState().clipboard.items).toHaveLength(1);
		await h.commands.cut();
		expect(h.said.at(-1)).toBe('This location cannot be changed.');
		expect(h.clipboard.store.getState().clipboard.mode).toBe('copy');
	});

	it('say why when Rust refuses', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 1);
		vi.spyOn(h.fake, 'setClipboardFromSelection').mockRejectedValue({
			kind: 'ops',
			message: 'nope',
			error: { kind: 'io', message: 'the listing is gone' },
		});
		await h.commands.copy();
		expect(h.said.at(-1)).toMatch(/^Could not finish: /);
	});
});

describe('Paste', () => {
	async function copied(h: ClipboardHarness, mode: 'copy' | 'cut', ...positions: number[]) {
		await selectIn(h.session, ...positions);
		await (mode === 'copy' ? h.commands.copy() : h.commands.cut());
		h.said.length = 0;
	}

	it('copies the clipboard into the folder it is pasted in, from another folder', async () => {
		const h = await clipboardHarness({ paired: true });
		await selectIn(h.other.session!, 0);
		const remote = h.commands;
		await remote.copy(h.other.session);
		const done = h.commands.paste();
		await h.finish();
		await done;
		expect(h.lastRequest()).toEqual({
			kind: { kind: 'copy' },
			sources: { kind: 'locations', locations: [child(OTHER, 'omega.txt')] },
			destination: FOLDER,
			name: null,
			options: { conflict: null, verify: null },
			originWindow: 'main-1',
		});
	});

	it('moves a cut, and spends it: the clipboard empties once the job is queued', async () => {
		const h = await clipboardHarness({ paired: true });
		await selectIn(h.other.session!, 0);
		await h.commands.cut(h.other.session);
		const done = h.commands.paste();
		await h.finish();
		await done;
		expect(h.lastRequest()).toMatchObject({
			kind: { kind: 'move' },
			sources: { kind: 'locations', locations: [child(OTHER, 'omega.txt')] },
			destination: FOLDER,
		});
		expect(h.clipboard.store.getState().clipboard.items).toEqual([]);
		// A second paste has nothing to move.
		await h.commands.paste();
		expect(h.said.at(-1)).toBe('There is nothing to paste.');
		expect(submits(h)).toHaveLength(1);
	});

	it('pastes into the current folder, even when a folder is selected, and into the folder from its own menu', async () => {
		const h = await clipboardHarness({ paired: true });
		await selectIn(h.other.session!, 0);
		await h.commands.copy(h.other.session);
		// A folder (gamma) is selected: Ctrl+V still pastes into the folder being shown.
		await selectIn(h.session, 0);
		let done = h.commands.paste();
		await h.finish(1);
		await done;
		expect(h.lastRequest()).toMatchObject({ destination: FOLDER });
		// Paste Into Folder, from that folder's menu, goes into it.
		const gamma = h.session.model.entryAt(0)!;
		done = h.commands.paste(h.session, gamma);
		await h.finish(2);
		await done;
		expect(h.lastRequest()).toMatchObject({ destination: child(FOLDER, 'gamma') });
	});

	it('refuses a folder pasted into itself, with the planner’s words, without queueing a job', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 0);
		await h.commands.copy();
		h.said.length = 0;
		await h.commands.paste(h.session, h.session.model.entryAt(0)!);
		expect(h.said).toEqual(['A folder cannot be put inside itself']);
		expect(submits(h)).toHaveLength(0);
	});

	it('refuses a cut pasted where it already is', async () => {
		const h = await clipboardHarness();
		await copied(h, 'cut', 0, 1);
		await h.commands.paste();
		expect(h.said).toEqual(['The items are already in that folder']);
		expect(submits(h)).toHaveLength(0);
		// The cut is still waiting: nothing was spent.
		expect(h.clipboard.store.getState().clipboard.mode).toBe('cut');
	});

	it('duplicates a copy pasted beside its originals', async () => {
		const h = await clipboardHarness();
		await copied(h, 'copy', 1);
		const done = h.commands.paste();
		await h.finish();
		await done;
		expect(h.lastRequest()).toMatchObject({
			kind: { kind: 'duplicate' },
			destination: null,
			sources: { kind: 'locations', locations: [child(FOLDER, 'alpha.txt')] },
		});
	});

	it('says there is nothing to paste when the clipboard is empty', async () => {
		const h = await clipboardHarness();
		await h.commands.paste();
		expect(h.said).toEqual(['There is nothing to paste.']);
	});

	it('is refused in a read-only location', async () => {
		const h = await clipboardHarness({ readOnly: true });
		await selectIn(h.session, 0);
		await h.commands.copy();
		await h.commands.paste();
		expect(h.said.at(-1)).toBe('This location cannot be changed.');
		expect(submits(h)).toHaveLength(0);
	});

	it('pastes files another application copied, as a copy from the system clipboard', async () => {
		const h = await clipboardHarness();
		h.os.external({ uris: ['file:///home/elsewhere/z.txt'], cut: false });
		const done = h.commands.paste();
		await h.finish();
		await done;
		expect(h.lastRequest()).toMatchObject({
			kind: { kind: 'copy' },
			destination: FOLDER,
			sources: { kind: 'locations', locations: [{ uri: 'file:///home/elsewhere/z.txt' }] },
		});
	});

	it('moves files another application cut', async () => {
		const h = await clipboardHarness();
		h.os.external({ uris: ['file:///home/elsewhere/z.txt'], cut: true });
		const done = h.commands.paste();
		await h.finish();
		await done;
		expect(h.lastRequest()).toMatchObject({ kind: { kind: 'move' } });
		expect(h.clipboard.store.getState().clipboard.items).toEqual([]);
	});

	it('ignores non-file content on the system clipboard and pastes Waypoint’s own', async () => {
		const h = await clipboardHarness({ paired: true });
		await selectIn(h.other.session!, 0);
		await h.commands.copy(h.other.session);
		h.os.external(null);
		const done = h.commands.paste();
		await h.finish();
		await done;
		expect(h.lastRequest()).toMatchObject({
			sources: { kind: 'locations', locations: [child(OTHER, 'omega.txt')] },
		});
	});

	it('says why a job fails, and leaves the queue to say the rest', async () => {
		const h = await clipboardHarness({ paired: true });
		await selectIn(h.other.session!, 0);
		await h.commands.copy(h.other.session);
		const done = h.commands.paste();
		await vi.waitFor(() => expect(h.fake.jobs()).toHaveLength(1));
		const job = h.fake.jobs()[0]!;
		h.fake.start(job.id);
		h.fake.fail(job.id, { kind: 'notEnoughSpace', needed: 10, free: 1 });
		await done;
		expect(h.said.at(-1)).toMatch(/^Could not finish: /);
	});

	it('works without a system clipboard at all', async () => {
		const h = await clipboardHarness({ paired: true, os: null });
		await selectIn(h.other.session!, 0);
		await h.commands.copy(h.other.session);
		const done = h.commands.paste();
		await h.finish();
		await done;
		expect(h.lastRequest()).toMatchObject({ kind: { kind: 'copy' } });
	});
});

describe('Copy To… and Move To…', () => {
	it('ask in the dialog, naming the count, then copy the selection there', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 1, 2);
		h.pick.value = OTHER;
		const done = h.commands.copyTo();
		await h.finish();
		await done;
		expect(h.asked).toEqual([
			{
				title: 'Copy 2 items to…',
				confirmLabel: 'Copy',
				base: FOLDER,
				origin: FOLDER,
				forbidOrigin: false,
			},
		]);
		expect(h.lastRequest()).toEqual({
			kind: { kind: 'copy' },
			sources: {
				kind: 'selection',
				handle: h.session.model.handle,
				spec: { kind: 'some', ids: [1, 2] },
			},
			destination: OTHER,
			name: null,
			options: { conflict: null, verify: null },
			originWindow: 'main-1',
		});
	});

	it('move, with the dialog refusing the folder the items are already in', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 0);
		h.pick.value = OTHER;
		const done = h.commands.moveTo();
		await h.finish();
		await done;
		expect(h.asked[0]).toMatchObject({
			title: 'Move 1 item to…',
			confirmLabel: 'Move',
			forbidOrigin: true,
		});
		expect(h.lastRequest()).toMatchObject({ kind: { kind: 'move' }, destination: OTHER });
	});

	it('do nothing when the dialog is cancelled', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 0);
		h.pick.value = null;
		await h.commands.copyTo();
		await h.commands.moveTo();
		expect(submits(h)).toHaveLength(0);
		expect(h.said).toEqual([]);
	});

	it('are not asked for with nothing selected', async () => {
		const h = await clipboardHarness();
		await h.commands.copyTo();
		await h.commands.moveTo();
		expect(h.asked).toEqual([]);
		expect(h.said).toEqual(['Nothing is selected.', 'Nothing is selected.']);
	});

	it('copy into the folder they are in as a duplicate, and refuse a move there', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 0);
		h.pick.value = FOLDER;
		const done = h.commands.copyTo();
		await h.finish();
		await done;
		expect(h.lastRequest()).toMatchObject({ kind: { kind: 'duplicate' }, destination: null });
		await h.commands.moveTo();
		expect(h.said.at(-1)).toBe('The items are already in that folder');
		expect(submits(h)).toHaveLength(1);
	});

	it('Copy To… works from a read-only location and Move To… does not', async () => {
		const h = await clipboardHarness({ readOnly: true });
		await selectIn(h.session, 0);
		h.pick.value = OTHER;
		const done = h.commands.copyTo();
		await h.finish();
		await done;
		await h.commands.moveTo();
		expect(h.said.at(-1)).toBe('This location cannot be changed.');
		expect(h.asked).toHaveLength(1);
	});
});

describe('F5 and Shift+F5 in a pair', () => {
	it('F5 copies the selection to the other pane’s folder without asking', async () => {
		const h = await clipboardHarness({ paired: true });
		await selectIn(h.session, 1, 2);
		const done = h.commands.copyToOtherPane();
		await h.finish();
		await done;
		expect(h.asked).toEqual([]);
		expect(h.lastRequest()).toMatchObject({
			kind: { kind: 'copy' },
			destination: OTHER,
			sources: { kind: 'selection', spec: { kind: 'some', ids: [1, 2] } },
		});
	});

	it('Shift+F5 moves it', async () => {
		const h = await clipboardHarness({ paired: true });
		await selectIn(h.session, 0);
		const done = h.commands.moveToOtherPane();
		await h.finish();
		await done;
		expect(h.lastRequest()).toMatchObject({ kind: { kind: 'move' }, destination: OTHER });
		expect(h.asked).toEqual([]);
	});

	it('open Copy To… and Move To… without a pair', async () => {
		const h = await clipboardHarness();
		await selectIn(h.session, 0);
		h.pick.value = OTHER;
		let done = h.commands.copyToOtherPane();
		await h.finish(1);
		await done;
		expect(h.asked[0]).toMatchObject({ confirmLabel: 'Copy' });
		done = h.commands.moveToOtherPane();
		await h.finish(2);
		await done;
		expect(h.asked[1]).toMatchObject({ confirmLabel: 'Move' });
		expect(submits(h)).toHaveLength(2);
	});

	it('open the dialog when the other pane cannot be written to', async () => {
		const h = await clipboardHarness({ paired: true, otherReadOnly: true });
		await selectIn(h.session, 0);
		h.pick.value = null;
		await h.commands.copyToOtherPane();
		await h.commands.moveToOtherPane();
		expect(h.asked.map((a) => a.confirmLabel)).toEqual(['Copy', 'Move']);
		expect(submits(h)).toHaveLength(0);
	});

	it('say so and do nothing with nothing selected', async () => {
		const h = await clipboardHarness({ paired: true });
		await h.commands.copyToOtherPane();
		await h.commands.moveToOtherPane();
		expect(h.said).toEqual(['Nothing is selected.', 'Nothing is selected.']);
		expect(h.asked).toEqual([]);
		expect(submits(h)).toHaveLength(0);
	});

	it('copy from a read-only pane, and refuse to move from one', async () => {
		const h = await clipboardHarness({ paired: true, readOnly: true });
		await selectIn(h.session, 0);
		const done = h.commands.copyToOtherPane();
		await h.finish();
		await done;
		expect(h.lastRequest()).toMatchObject({ kind: { kind: 'copy' }, destination: OTHER });
		await h.commands.moveToOtherPane();
		expect(h.said.at(-1)).toBe('This location cannot be changed.');
		expect(submits(h)).toHaveLength(1);
	});

	it('reports a pair’s own folder as the same folder for a move, and duplicates a copy', async () => {
		const h = await clipboardHarness({ paired: true });
		// Both panes show the same folder.
		h.other.session = h.session;
		await selectIn(h.session, 0);
		const done = h.commands.copyToOtherPane();
		await h.finish();
		await done;
		expect(h.lastRequest()).toMatchObject({ kind: { kind: 'duplicate' } });
	});
});
