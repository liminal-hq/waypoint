// Tests for the command registry: its table is complete, and availability is right in each state a window can be in
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it, vi } from 'vitest';
import { t } from '../i18n/messages';
import { commandStates } from '../ops/fileCommands';
import { entry, factsFor } from '../test/commandFacts';
import { emptyFacts, idleActions } from './commandEnv';
import {
	COMMANDS,
	commandDef,
	evaluateCommands,
	runCommand,
	viewOf,
	type CommandId,
	type CommandView,
} from './registry';

const states = (facts = factsFor()) =>
	Object.fromEntries(evaluateCommands(facts).map((view) => [view.id, view])) as Record<
		CommandId,
		CommandView
	>;

/** "shown", "hidden", or "disabled: reason" for one command, the way the availability table below reads. */
const read = (view: CommandView) =>
	!view.visible ? 'hidden' : view.enabled ? 'enabled' : `disabled: ${view.reason}`;

describe('the table', () => {
	it('has one definition per id and every id is reachable', () => {
		const ids = COMMANDS.map((command) => command.id);
		expect(new Set(ids).size).toBe(ids.length);
		for (const id of ids) expect(commandDef(id).id).toBe(id);
	});

	it('gives every command a label, an icon and a group, and no two the same key', () => {
		const keys = new Set<string>();
		for (const command of COMMANDS) {
			expect(t(command.label), command.id).not.toBe('');
			expect(command.icon, command.id).toBeDefined();
			expect(command.group, command.id).toBeDefined();
			if (command.shortcut) {
				expect(keys.has(command.shortcut), command.shortcut).toBe(false);
				keys.add(command.shortcut);
			}
		}
	});

	it('covers every file command (the paste-into-a-folder one needs an entry, so only the entry menu has it)', () => {
		const covered = new Set<string>(COMMANDS.map((command) => command.id));
		for (const id of Object.keys(factsFor().file)) {
			if (id === 'pasteInto') continue;
			expect(covered.has(id), id).toBe(true);
		}
	});

	it('resolves every command against the facts in registry order', () => {
		expect(evaluateCommands(factsFor()).map((view) => view.id)).toEqual(
			COMMANDS.map((command) => command.id),
		);
	});
});

describe('availability', () => {
	it('with a folder open and nothing selected, creation works and what needs a selection says so', () => {
		const result = states();
		const needsSelection = `disabled: ${t('cmd.reason.nothingSelected')}`;
		expect(read(result.newFolder)).toBe('enabled');
		expect(read(result.newFile)).toBe('enabled');
		expect(read(result.selectAll)).toBe('enabled');
		expect(read(result.rename)).toBe(`disabled: ${t('cmd.reason.nothingFocused')}`);
		for (const id of [
			'duplicate',
			'moveToTrash',
			'deletePermanently',
			'cut',
			'copy',
			'copyTo',
			'moveTo',
			'batchRename',
		] as const) {
			expect(read(result[id]), id).toBe(needsSelection);
		}
		expect(read(result.paste)).toBe(`disabled: ${t('cmd.reason.clipboardEmpty')}`);
		expect(read(result.undo)).toBe(`disabled: ${t('cmd.reason.nothingToUndo')}`);
		expect(read(result.redo)).toBe(`disabled: ${t('cmd.reason.nothingToRedo')}`);
	});

	it('with a selection, the commands that act on it are enabled', () => {
		const result = states(factsFor({ selected: 2, focused: true }));
		for (const id of [
			'rename',
			'batchRename',
			'duplicate',
			'moveToTrash',
			'deletePermanently',
			'cut',
			'copy',
			'copyTo',
			'moveTo',
		] as const) {
			expect(read(result[id]), id).toBe('enabled');
		}
	});

	it('hides what writes in a read-only location and keeps the commands that only read', () => {
		const result = states(factsFor({ readOnly: true, selected: 1 }));
		for (const id of [
			'newFolder',
			'newFile',
			'rename',
			'batchRename',
			'duplicate',
			'moveToTrash',
			'deletePermanently',
			'cut',
			'paste',
			'moveTo',
		] as const) {
			expect(read(result[id]), id).toBe('hidden');
		}
		expect(read(result.copy)).toBe('enabled');
		expect(read(result.copyTo)).toBe('enabled');
		expect(read(result.undo).startsWith('disabled')).toBe(true);
	});

	it('in the Trash hides new, paste and copy, has the Date deleted sort and no hidden-files toggle', () => {
		const result = states(factsFor({ readOnly: true, trash: true, selected: 1 }));
		for (const id of ['newFolder', 'newFile', 'paste', 'copy', 'copyTo', 'showHidden'] as const) {
			expect(read(result[id]), id).toBe('hidden');
		}
		expect(read(result.sortDeleted)).toBe('enabled');
		expect(read(result.sortModified)).toBe('hidden');
		expect(read(result.sortKind)).toBe('hidden');
		expect(read(result.sortName)).toBe('enabled');
	});

	it('outside the Trash the Date deleted sort is hidden and Modified and Kind are offered', () => {
		const result = states();
		expect(read(result.sortDeleted)).toBe('hidden');
		expect(read(result.sortModified)).toBe('enabled');
		expect(read(result.sortKind)).toBe('enabled');
		expect(read(result.showHidden)).toBe('enabled');
	});

	it('offers the other-pane commands only in a pair, and says when the other pane cannot be written to', () => {
		expect(read(states(factsFor({ selected: 1 })).copyToOtherPane)).toBe('hidden');
		const writable = states(factsFor({ selected: 1, paired: true, otherPaneWritable: true }));
		expect(read(writable.copyToOtherPane)).toBe('enabled');
		expect(read(writable.moveToOtherPane)).toBe('enabled');
		const readOnly = states(factsFor({ selected: 1, paired: true, otherPaneWritable: false }));
		expect(read(readOnly.copyToOtherPane)).toBe(`disabled: ${t('cmd.reason.otherPaneReadOnly')}`);
		const none = states(factsFor({ selected: 0, paired: true, otherPaneWritable: true }));
		expect(read(none.copyToOtherPane)).toBe(`disabled: ${t('cmd.reason.nothingSelected')}`);
	});

	it('enables Paste when the clipboard holds something', () => {
		expect(read(states(factsFor({ clipboardItems: 2 })).paste)).toBe('enabled');
		expect(read(states(factsFor({ clipboardItems: 0 })).paste).startsWith('disabled')).toBe(true);
	});

	it('enables Undo and Redo when the history has something, and names it', () => {
		const facts = factsFor({
			undo: entry(2, 'Move 3 items to Trash'),
			redo: entry(1, 'New folder', { undoable: false, redoable: true }),
		});
		const result = states(facts);
		expect(read(result.undo)).toBe('enabled');
		expect(read(result.redo)).toBe('enabled');
		expect(result.undo.label).toBe('Undo Move 3 items to Trash');
		expect(result.redo.label).toBe('Redo New folder');
		expect(states().undo.label).toBe('Undo');
	});

	it('hides the file commands and the history in a window with no queue', () => {
		const result = states(factsFor({ queue: false }));
		for (const id of ['newFolder', 'rename', 'cut', 'paste', 'undo', 'redo'] as const) {
			expect(read(result[id]), id).toBe('hidden');
		}
		// Windows and tabs do not need a queue.
		expect(read(result.newWindow)).toBe('enabled');
		expect(read(result.newTab)).toBe('enabled');
	});

	it('hides what needs a listing when none is open', () => {
		const result = states(factsFor({ listing: false }));
		for (const id of ['selectAll', 'invertSelection', 'sortName', 'sortDescending'] as const) {
			expect(read(result[id]), id).toBe('hidden');
		}
	});

	it('disables the tab commands with no tab, and shows Split View checked for a pair', () => {
		const none = states(factsFor({}, { tab: false, tabCount: 0 }));
		for (const id of ['closeTab', 'duplicateTab', 'moveTabToNewWindow', 'splitView'] as const) {
			expect(read(none[id]), id).toBe(`disabled: ${t('cmd.reason.noTab')}`);
		}
		expect(read(none.reopenClosedTab)).toBe('enabled');
		expect(states().splitView.checked).toBe(false);
		expect(states(factsFor({ paired: true })).splitView.checked).toBe(true);
	});

	it('reports the settings that are on as checked', () => {
		const result = states(
			factsFor({}, { viewMode: 'grid', showHidden: true, sidebarOpen: false, actionBar: true }),
		);
		expect(result.viewGrid.checked).toBe(true);
		expect(result.viewList.checked).toBe(false);
		expect(result.showHidden.checked).toBe(true);
		expect(result.sidebar.checked).toBe(false);
		expect(result.actionBar.checked).toBe(true);
		expect(result.sortName.checked).toBe(true);
		expect(result.sortSize.checked).toBe(false);
		expect(result.sortFoldersFirst.checked).toBe(true);
		expect(result.sortDescending.checked).toBe(false);
	});

	it('offers Always on Top only where the window manager can do it', () => {
		expect(read(states().alwaysOnTop)).toBe('hidden');
		const supported = states(factsFor({}, { alwaysOnTop: { supported: true, on: true } }));
		expect(read(supported.alwaysOnTop)).toBe('enabled');
		expect(supported.alwaysOnTop.checked).toBe(true);
	});

	it('always offers the window and settings commands', () => {
		const result = states(emptyFacts());
		for (const id of ['newWindow', 'newTab', 'settings', 'closeWindow'] as const) {
			expect(read(result[id]), id).toBe('enabled');
		}
	});

	it('offers Link To… for a selection of local items, and hides it where a link cannot be relied on', () => {
		expect(read(states(factsFor({ selected: 1 }, { local: true })).linkTo)).toBe('enabled');
		expect(read(states(factsFor({ selected: 0 }, { local: true })).linkTo)).toBe(
			`disabled: ${t('cmd.reason.nothingSelected')}`,
		);
		// Not on Windows (a link needs a privilege), not for a remote or virtual listing, not in the Trash.
		expect(read(states(factsFor({ selected: 1 }, { linkSupported: false })).linkTo)).toBe('hidden');
		expect(read(states(factsFor({ selected: 1 }, { local: false })).linkTo)).toBe('hidden');
		expect(read(states(factsFor({ selected: 1, trash: true, readOnly: true })).linkTo)).toBe(
			'hidden',
		);
		expect(read(states(factsFor({ queue: false, selected: 1 })).linkTo)).toBe('hidden');
	});

	it('offers a Go to command for each place the sidebar has, and for a tab to open it in', () => {
		const ids = [
			'goHome',
			'goDesktop',
			'goDocuments',
			'goDownloads',
			'goPictures',
			'goMusic',
			'goVideos',
			'goTrash',
		] as const;
		for (const id of ids) expect(read(states()[id]), id).toBe('enabled');
		const some = states(factsFor({}, { places: ['home', 'downloads'] }));
		expect(read(some.goHome)).toBe('enabled');
		expect(read(some.goDownloads)).toBe('enabled');
		expect(read(some.goMusic)).toBe('hidden');
		expect(read(states(factsFor({}, { tab: false })).goHome)).toBe(
			`disabled: ${t('cmd.reason.noTab')}`,
		);
		expect(states().goDownloads.label).toBe('Go to Downloads');
	});

	it('always offers the command palette, with its key', () => {
		expect(read(states(emptyFacts()).commandPalette)).toBe('enabled');
		expect(commandDef('commandPalette').shortcut).toBe('Ctrl+Shift+P');
	});

	it('agrees with `commandStates` for every file command', () => {
		const context = {
			queue: true,
			listing: true,
			readOnly: false,
			selected: 2,
			focused: true,
			undo: null,
			redo: null,
			clipboardItems: 1,
		};
		const expected = commandStates(context);
		const facts = factsFor({ selected: 2, focused: true, clipboardItems: 1 });
		for (const id of Object.keys(expected) as Array<keyof typeof expected>) {
			if (id === 'pasteInto') continue;
			const view = viewOf(commandDef(id), facts);
			expect(view.visible, id).toBe(expected[id].visible);
			expect(view.enabled, id).toBe(expected[id].enabled);
		}
	});
});

describe('running', () => {
	it('runs an enabled command through its action and says so', () => {
		const actions = {
			...idleActions(),
			newTab: vi.fn(),
			setViewMode: vi.fn(),
			toggleHidden: vi.fn(),
		};
		const facts = factsFor();
		expect(runCommand('newTab', actions, facts)).toBe(true);
		expect(actions.newTab).toHaveBeenCalledTimes(1);
		expect(runCommand('viewGrid', actions, facts)).toBe(true);
		expect(actions.setViewMode).toHaveBeenCalledWith('grid');
		expect(runCommand('showHidden', actions, facts)).toBe(true);
		expect(actions.toggleHidden).toHaveBeenCalledTimes(1);
	});

	it('does nothing for a command that is disabled or hidden', () => {
		const actions = { ...idleActions(), selectAll: vi.fn(), batchRename: vi.fn() };
		expect(runCommand('batchRename', actions, factsFor())).toBe(false);
		expect(runCommand('selectAll', actions, factsFor({ listing: false }))).toBe(false);
		expect(actions.batchRename).not.toHaveBeenCalled();
		expect(actions.selectAll).not.toHaveBeenCalled();
	});

	it("hands the file commands to the window's `FileCommands`", async () => {
		const files = {
			newFolder: vi.fn().mockResolvedValue(undefined),
			paste: vi.fn().mockResolvedValue(undefined),
			undo: vi.fn().mockResolvedValue(undefined),
		};
		const actions = { ...idleActions(), files: files as never };
		const facts = factsFor({ clipboardItems: 1, undo: entry(1, 'New folder') });
		expect(runCommand('newFolder', actions, facts)).toBe(true);
		expect(runCommand('paste', actions, facts)).toBe(true);
		expect(runCommand('undo', actions, facts)).toBe(true);
		expect(files.newFolder).toHaveBeenCalledTimes(1);
		expect(files.paste).toHaveBeenCalledTimes(1);
		expect(files.undo).toHaveBeenCalledTimes(1);
	});

	it('sends a Go to command to the place it names, Link To… to the file commands, and the palette to its action', async () => {
		const files = { linkTo: vi.fn().mockResolvedValue(undefined) };
		const actions = {
			...idleActions(),
			files: files as never,
			goToPlace: vi.fn(),
			openPalette: vi.fn(),
		};
		const facts = factsFor({ selected: 1 }, { local: true });
		expect(runCommand('goDownloads', actions, facts)).toBe(true);
		expect(actions.goToPlace).toHaveBeenCalledWith('downloads');
		expect(runCommand('goTrash', actions, facts)).toBe(true);
		expect(actions.goToPlace).toHaveBeenLastCalledWith('trash');
		expect(runCommand('linkTo', actions, facts)).toBe(true);
		expect(files.linkTo).toHaveBeenCalledTimes(1);
		expect(runCommand('commandPalette', actions, facts)).toBe(true);
		expect(actions.openPalette).toHaveBeenCalledTimes(1);
	});

	it('changes the sort the way each sort command says', () => {
		const changes: Array<(sort: ReturnType<typeof factsFor>['sort'] & object) => unknown> = [];
		const actions = {
			...idleActions(),
			changeSort: (change: (typeof changes)[number]) => changes.push(change),
		};
		const facts = factsFor();
		runCommand('sortSize', actions as never, facts);
		runCommand('sortName', actions as never, facts);
		runCommand('sortDescending', actions as never, facts);
		runCommand('sortFoldersFirst', actions as never, facts);
		const base = { key: 'name', descending: true, directoriesFirst: true } as const;
		// A new key starts ascending; the key already in force is left as it is.
		expect(changes[0]!(base)).toEqual({ key: 'size', descending: false, directoriesFirst: true });
		expect(changes[1]!(base)).toEqual(base);
		expect(changes[2]!(base)).toEqual({ ...base, descending: false });
		expect(changes[3]!(base)).toEqual({ ...base, directoriesFirst: false });
	});
});
