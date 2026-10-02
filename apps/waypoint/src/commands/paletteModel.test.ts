// Tests for what the palette lists: the visible commands, ranked, with recents first for an empty query and the history for a typed one
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { describe, expect, it } from 'vitest';
import { t } from '../i18n/messages';
import { entry, factsFor } from '../test/commandFacts';
import { historyRows } from './historyCommands';
import { countText, listable, moveActive, paletteRows, RECENT_LIMIT } from './paletteModel';
import { evaluateCommands, type CommandId } from './registry';

const NOW = new Date(2026, 9, 1, 14, 30).getTime();
const views = (state = {}) => evaluateCommands(factsFor({ selected: 1, ...state }));
const rowsFor = (query: string, extra: { recents?: CommandId[]; state?: object } = {}) =>
	paletteRows({
		commands: views(extra.state),
		history: [],
		query,
		recents: extra.recents ?? [],
		now: NOW,
		locale: 'en-CA',
	});
const idsOf = (rows: ReturnType<typeof rowsFor>) =>
	rows.map((row) => (row.target.kind === 'command' ? row.target.id : row.key));

describe('what is listed', () => {
	it('lists the visible commands and not the palette itself', () => {
		const all = listable(views());
		expect(all.every((view) => view.visible)).toBe(true);
		expect(all.map((view) => view.id)).not.toContain('commandPalette');
		expect(all.map((view) => view.id)).toContain('newFolder');
	});

	it('leaves out a command the location cannot do at all, and keeps one that needs something first', () => {
		const inTrash = idsOf(rowsFor('', { state: { trash: true, readOnly: true } }));
		expect(inTrash).not.toContain('newFolder');
		const empty = rowsFor('', { state: { selected: 0 } });
		const trash = empty.find((row) => row.label === t('menu.moveToTrash'));
		expect(trash).toMatchObject({ enabled: false, reason: t('cmd.reason.nothingSelected') });
	});

	it('never lists the same row twice', () => {
		const keys = rowsFor('').map((row) => row.key);
		expect(new Set(keys).size).toBe(keys.length);
	});
});

describe('an empty query', () => {
	it('shows the commands run last first, newest first, then every other one in the menus’ order', () => {
		const rows = rowsFor('', { recents: ['viewGrid', 'newFolder'] });
		expect(idsOf(rows).slice(0, 2)).toEqual(['viewGrid', 'newFolder']);
		expect(rows[0]!.hint).toBe(t('palette.group.recent'));
		const rest = idsOf(rows).slice(2);
		expect(rest).not.toContain('viewGrid');
		expect(rest.indexOf('newWindow')).toBeLessThan(rest.indexOf('undo'));
		expect(rest.indexOf('undo')).toBeLessThan(rest.indexOf('viewList'));
		expect(rest.indexOf('viewList')).toBeLessThan(rest.indexOf('goHome'));
		expect(rest.indexOf('goHome')).toBeLessThan(rest.indexOf('duplicateTab'));
		expect(rest.indexOf('duplicateTab')).toBeLessThan(rest.indexOf('settings'));
	});

	it('keeps at most five recents and skips one that is no longer offered', () => {
		const many: CommandId[] = [
			'newTab',
			'newFile',
			'viewGrid',
			'viewList',
			'sidebar',
			'showHidden',
		];
		const rows = rowsFor('', { recents: many });
		expect(rows.filter((row) => row.hint === t('palette.group.recent'))).toHaveLength(RECENT_LIMIT);
		const gone = rowsFor('', { recents: ['newFolder'], state: { trash: true, readOnly: true } });
		expect(gone.some((row) => row.hint === t('palette.group.recent'))).toBe(false);
	});

	it('offers no history rows until something is typed', () => {
		const rows = paletteRows({
			commands: views(),
			history: historyRows([entry(3, 'a'), entry(2, 'b')], { undoHead: 3, redoHead: null }),
			query: '',
			recents: [],
		});
		expect(rows.some((row) => row.target.kind === 'history')).toBe(false);
	});

	it('treats a blank query as empty', () => {
		expect(idsOf(rowsFor('   '))).toEqual(idsOf(rowsFor('')));
	});
});

describe('a typed query', () => {
	it('keeps only what matches, best first, and reports the ranges to highlight', () => {
		const rows = rowsFor('new f');
		expect(rows[0]).toMatchObject({ label: 'New Folder' });
		expect(rows[0]!.ranges).toEqual([
			[0, 3],
			[4, 5],
		]);
		expect(idsOf(rows)).toContain('newFile');
		expect(idsOf(rows)).not.toContain('viewGrid');
	});

	it('ranks a word-start match above a scattered one', () => {
		const rows = rowsFor('trash');
		expect(rows[0]!.label).toBe(t('menu.moveToTrash'));
	});

	it('breaks a tie for a recent command, then by group, then by the registry’s order', () => {
		// "New Tab" and "New Window" and "New Folder" and "New File" all match "new" from the start.
		const tied = rowsFor('new');
		const labels = tied.slice(0, 4).map((row) => row.label);
		expect(labels[0]).toBe('New Window');
		const recent = rowsFor('new', { recents: ['newFile'] });
		expect(recent[0]!.label).toBe('New File');
		const again = rowsFor('new');
		expect(again.map((row) => row.key)).toEqual(tied.map((row) => row.key));
	});

	it('finds a disabled command and says why it cannot run', () => {
		const rows = rowsFor('rename', { state: { selected: 0 } });
		const rename = rows.find((row) => row.label === t('menu.rename'))!;
		expect(rename.enabled).toBe(false);
		expect(rename.reason).toBe(t('cmd.reason.nothingFocused'));
		expect(rename.name).toContain(t('cmd.reason.nothingFocused'));
	});

	it('is case-insensitive', () => {
		expect(idsOf(rowsFor('NEW FOLDER'))[0]).toBe('newFolder');
	});

	it('is empty when nothing matches', () => {
		expect(rowsFor('qzx')).toEqual([]);
	});
});

describe('the words of a row', () => {
	it('names the command, its group, its key and its state for a screen reader', () => {
		const trash = rowsFor('trash')[0]!;
		expect(trash.name).toBe(
			`${t('menu.moveToTrash')}, ${t('palette.group.file')}, shortcut Delete`,
		);
		const hidden = rowsFor('hidden').find((row) => row.label === t('menu.showHidden'))!;
		expect(hidden.name).toBe(
			`${t('menu.showHidden')}, ${t('palette.group.view')}, shortcut Ctrl+H, ${t('palette.unchecked')}`,
		);
		expect(hidden.checked).toBe(false);
		const checked = rowsFor('hidden', { state: {} });
		expect(checked.length).toBeGreaterThan(0);
	});

	it('shows the group as the hint', () => {
		expect(rowsFor('new folder')[0]!.hint).toBe(t('palette.group.file'));
		expect(rowsFor('go to downloads')[0]!.hint).toBe(t('palette.group.go'));
	});
});

describe('history rows', () => {
	const history = [
		entry(5, 'Move 3 items to Trash', { atMs: NOW - 2 * 60_000 }),
		entry(4, 'New folder', { atMs: NOW - 3_600_000 }),
		entry(3, 'Rename report', { atMs: NOW - 86_400_000 }),
	];
	const rows = historyRows(history, { undoHead: 5, redoHead: null });
	const typed = (query: string) =>
		paletteRows({
			commands: views({ undo: history[0] }),
			history: rows,
			query,
			recents: [],
			now: NOW,
			locale: 'en-CA',
		});

	it('brings every entry in once the query starts with undo, after the registry’s own Undo', () => {
		const result = typed('undo');
		expect(result.map((row) => row.label)).toEqual([
			'Undo Move 3 items to Trash',
			'Undo 2 changes back to: New folder',
			'Undo 3 changes back to: Rename report',
		]);
	});

	it('words each with a relative time and a tooltip with the exact one', () => {
		const result = typed('undo').filter((row) => row.target.kind === 'history');
		expect(result.map((row) => row.detail)).toEqual(['1 hour ago', 'yesterday']);
		expect(result[0]!.detailTitle).toMatch(/2026/);
		expect(result[0]!.name).toBe('Undo 2 changes back to: New folder, 1 hour ago');
		expect(result[0]!.hint).toBe(t('palette.group.history'));
	});

	it('filters them by what is typed after "undo"', () => {
		const result = typed('undo rename');
		expect(result[0]!.label).toBe('Undo 3 changes back to: Rename report');
		expect(result.some((row) => row.label.includes('New folder'))).toBe(false);
	});

	it('also finds an entry by what it did', () => {
		expect(typed('rename report').some((row) => row.target.kind === 'history')).toBe(true);
	});

	it('marks a partly undone entry in its row', () => {
		const partly = historyRows(
			[entry(2, 'Newest'), entry(1, 'Half done', { partlyUndone: true })],
			{ undoHead: 2, redoHead: null },
		);
		const result = paletteRows({
			commands: [],
			history: partly,
			query: 'undo',
			recents: [],
			now: NOW,
		});
		expect(result[0]!.detail).toContain(t('history.row.partly'));
	});
});

describe('counting and moving', () => {
	it('words the count for the live region', () => {
		expect(countText(0)).toBe('No commands match');
		expect(countText(1)).toBe('1 command');
		expect(countText(12)).toBe('12 commands');
	});

	it('moves the active row, wrapping for a step and stopping at the ends for a page', () => {
		expect(moveActive(0, 1, 5, true)).toBe(1);
		expect(moveActive(4, 1, 5, true)).toBe(0);
		expect(moveActive(0, -1, 5, true)).toBe(4);
		expect(moveActive(3, 8, 5)).toBe(4);
		expect(moveActive(3, -8, 5)).toBe(0);
		expect(moveActive(2, Infinity, 5)).toBe(4);
		expect(moveActive(2, -Infinity, 5)).toBe(0);
		expect(moveActive(0, 1, 0)).toBe(-1);
	});
});
