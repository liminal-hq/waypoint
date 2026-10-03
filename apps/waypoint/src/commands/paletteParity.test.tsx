// Verifies the palette lists the registry's own commands: the menu, the Action bar, the keys and the palette share one definition each
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import { describe, expect, it, vi } from 'vitest';
import { actionBarItems } from '../app/actionBarModel';
import { t } from '../i18n/messages';
import { entry, factsFor } from '../test/commandFacts';
import { appMenuItems } from './appMenuModel';
import { idleActions } from './commandEnv';
import { listable, paletteRows } from './paletteModel';
import { COMMANDS, commandDef, evaluateCommands, runCommand, type CommandId } from './registry';

const facts = factsFor(
	{ selected: 2, focused: true, clipboardItems: 1, undo: entry(2, 'New folder') },
	{ alwaysOnTop: { supported: true, on: false } },
);
const views = evaluateCommands(facts);
const rows = paletteRows({ commands: views, history: [], query: '', recents: [] });
const rowByCommand = new Map(
	rows.flatMap((row) => (row.target.kind === 'command' ? [[row.target.id, row] as const] : [])),
);

type CommandItem = Extract<MenuItem, { type: 'action' | 'checkbox' }>;

function menuCommandItems(items: readonly MenuItem[]): CommandItem[] {
	return items.flatMap((item): CommandItem[] => {
		if (item.type === 'submenu') return menuCommandItems(item.items);
		return item.type === 'action' || item.type === 'checkbox' ? [item] : [];
	});
}

describe('the palette and the registry', () => {
	it('lists every visible command once, and only the registry’s', () => {
		const wanted = views.filter((view) => view.visible && view.id !== 'commandPalette');
		expect(
			rows.map((row) => (row.target.kind === 'command' ? row.target.id : null)).sort(),
		).toEqual(wanted.map((view) => view.id).sort());
		for (const id of rowByCommand.keys()) expect(COMMANDS.map((c) => c.id)).toContain(id);
	});

	it('shows each command with the label, key, state and reason the registry gives it', () => {
		for (const view of listable(views)) {
			const row = rowByCommand.get(view.id)!;
			expect(row.label, view.id).toBe(view.label);
			expect(row.shortcut, view.id).toBe(commandDef(view.id).shortcut);
			expect(row.enabled, view.id).toBe(view.enabled);
			expect(row.reason, view.id).toBe(view.reason);
			expect(row.checked, view.id).toBe(view.checked);
		}
	});

	it('runs a chosen row through `runCommand`, the call the menu makes', () => {
		const actions = { ...idleActions(), newTab: vi.fn(), openPalette: vi.fn() };
		expect(runCommand('newTab', actions, facts)).toBe(true);
		expect(actions.newTab).toHaveBeenCalledTimes(1);
		expect(runCommand('commandPalette', actions, facts)).toBe(true);
		expect(actions.openPalette).toHaveBeenCalledTimes(1);
	});
});

describe('the menu', () => {
	const items = menuCommandItems(appMenuItems(views, facts)).filter(
		(item) => !String(item.id).startsWith('menu:') && !String(item.id).startsWith('history:'),
	);

	it('has only registry commands, each worded as the palette words it', () => {
		expect(items.length).toBeGreaterThan(20);
		for (const item of items) {
			const id = item.id as CommandId;
			expect(
				COMMANDS.map((c) => c.id),
				id,
			).toContain(id);
			const row = rowByCommand.get(id);
			if (id === 'commandPalette') continue;
			expect(row, id).toBeDefined();
			expect(row!.label, id).toBe(item.label);
			expect('shortcut' in item ? item.shortcut : undefined, id).toBe(row!.shortcut);
		}
	});

	it('has the palette’s own entry, which the palette leaves out because it is open', () => {
		expect(items.map((item) => item.id)).toContain('commandPalette');
		const entryForPalette = items.find((item) => item.id === 'commandPalette')!;
		expect(entryForPalette.label).toBe(t('cmd.commandPalette'));
		expect('shortcut' in entryForPalette && entryForPalette.shortcut).toBe('Ctrl+Shift+P');
	});
});

describe('the Action bar', () => {
	it('stands for registry commands the palette also lists, with the same label and key', () => {
		const api = { get: (id: CommandId) => views.find((view) => view.id === id)! };
		const ids = actionBarItems(api).flatMap((item) => (item.command ? [item.command] : []));
		expect(ids.length).toBeGreaterThan(5);
		for (const id of ids) {
			const row = rowByCommand.get(id)!;
			expect(row, id).toBeDefined();
			expect(row.shortcut, id).toBe(commandDef(id).shortcut);
		}
	});
});

describe('the Shelf commands', () => {
	it('are listed with the labels and the key the registry gives them', () => {
		expect(rowByCommand.get('toggleShelf')).toMatchObject({
			label: t('cmd.shelf'),
			shortcut: 'Ctrl+B',
			enabled: true,
		});
		expect(rowByCommand.get('addToShelf')).toMatchObject({
			label: t('cmd.addToShelf'),
			shortcut: commandDef('addToShelf').shortcut,
			enabled: true,
		});
		expect(rowByCommand.get('focusShelf')).toMatchObject({
			label: t('cmd.focusShelf'),
			shortcut: commandDef('focusShelf').shortcut,
			enabled: true,
		});
	});

	it('are found by what the palette is typed', () => {
		const found = (query: string) =>
			paletteRows({ commands: views, history: [], query, recents: [] }).map((row) => row.label);
		expect(found('shelf')).toEqual(
			expect.arrayContaining([t('cmd.shelf'), t('cmd.addToShelf'), t('cmd.focusShelf')]),
		);
	});

	it('are in the View and Edit menus alongside the palette’s and the Link To… entries', () => {
		const ids = menuCommandItems(appMenuItems(views, facts)).map((item) => item.id);
		expect(ids).toEqual(expect.arrayContaining(['toggleShelf', 'addToShelf', 'linkTo']));
	});
});

describe('commands only the palette reaches', () => {
	it('lists Link To… where a link can be made, hidden where it cannot', () => {
		expect(rowByCommand.get('linkTo')).toMatchObject({ label: t('cmd.linkTo'), enabled: true });
		const windows = paletteRows({
			commands: evaluateCommands({ ...facts, linkSupported: false }),
			history: [],
			query: 'link to',
			recents: [],
		});
		expect(windows.some((row) => row.label === t('cmd.linkTo'))).toBe(false);
	});

	it('lists one Go to command for each place the sidebar has, and Open Overview for the page', () => {
		const labels = rows.filter((row) => row.hint === t('palette.group.go')).map((row) => row.label);
		expect(labels).toEqual([
			'Go to Home',
			'Go to Desktop',
			'Go to Documents',
			'Go to Downloads',
			'Go to Pictures',
			'Go to Music',
			'Go to Videos',
			'Go to Trash',
			'Open Overview',
		]);
	});
});
