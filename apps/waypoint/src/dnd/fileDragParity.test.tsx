// Verifies every file drop outcome has a path without a pointer, so a new outcome cannot ship without one
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { MenuItem } from '@liminal-hq/waypoint-chrome/ContextMenu/types';
import type { Entry } from '@liminal-hq/waypoint-protocol/generated/Entry';
import { describe, expect, it } from 'vitest';
import { entryMenuItems } from '../browse/EntryContextMenu';
import { commandStates } from '../ops/fileCommands';
import { handleFileKey, type FileKeyHandlers } from '../ops/useFileShortcuts';
import { clipboardHarness } from '../test/clipboardHarness';
import {
	NATIVE_DROP_PATHS,
	NON_POINTER_PATHS,
	OUTBOUND_PATHS,
	TARGET_PATHS,
	type DropOutcome,
	type NonPointerPath,
} from './fileDragModel';
import { DROP_KINDS } from './dropTargets';

/** Every outcome, written out so that adding one to `DropOutcome` fails to compile until it is here. */
const OUTCOMES = [
	'copy',
	'move',
	'link',
	'ask',
	'trash',
	'open',
	'shelf',
] as const satisfies readonly DropOutcome[];
type Missing = Exclude<DropOutcome, (typeof OUTCOMES)[number]>;
const complete: [Missing] extends [never] ? true : never = true;

const idsOf = (items: readonly MenuItem[]): string[] =>
	items
		.flatMap((item) => [item.id, ...(item.type === 'submenu' ? idsOf(item.items) : [])])
		.filter((id): id is string => id !== undefined);

const folder = { id: 2, name: 'd', kind: 'directory', linkTarget: null } as unknown as Entry;
const everything = commandStates({
	queue: true,
	listing: true,
	readOnly: false,
	selected: 1,
	focused: true,
	undo: null,
	redo: null,
	paired: true,
	otherPaneWritable: true,
	clipboardItems: 1,
});
const menuIds = idsOf(entryMenuItems(folder, everything));

describe('non-pointer parity of a file drop', () => {
	it('has a path for every outcome, and no stale ones', () => {
		expect(complete).toBe(true);
		expect(Object.keys(NON_POINTER_PATHS).sort()).toEqual([...OUTCOMES].sort());
	});

	it('names a command that exists, a menu item that is in the menu, or says why there is none', async () => {
		const h = await clipboardHarness();
		for (const [outcome, path] of Object.entries(NON_POINTER_PATHS) as Array<
			[DropOutcome, NonPointerPath]
		>) {
			if (path.kind === 'command') {
				// A command is a method of the window's commands, and (except Link To…) a visible menu item.
				expect(
					typeof (h.commands as unknown as Record<string, unknown>)[path.command],
					outcome,
				).toBe('function');
				if (path.command !== 'linkTo') {
					expect(everything[path.command].visible, outcome).toBe(true);
				}
			} else if (path.kind === 'menu') {
				expect(menuIds, outcome).toContain(path.item);
			} else {
				expect(path.why.length, outcome).toBeGreaterThan(10);
			}
		}
	});

	it('reaches the Trash with the Delete key', () => {
		const calls: string[] = [];
		const handlers = new Proxy(
			{},
			{ get: (_, name) => () => void calls.push(String(name)) },
		) as FileKeyHandlers;
		expect(NON_POINTER_PATHS.trash).toMatchObject({ keys: 'Delete' });
		handleFileKey(
			{
				key: 'Delete',
				ctrlKey: false,
				metaKey: false,
				altKey: false,
				shiftKey: false,
				isComposing: false,
			},
			handlers,
		);
		expect(calls).toEqual(['moveToTrash']);
	});

	it('reaches every kind of target', () => {
		expect(Object.keys(TARGET_PATHS).sort()).toEqual([...DROP_KINDS].sort());
		for (const [kind, path] of Object.entries(TARGET_PATHS)) {
			expect(path in everything || menuIds.includes(path), `${kind} reaches ${path}`).toBe(true);
		}
	});

	it('has a path for every outcome of a drop from another application, naming what exists', async () => {
		expect(Object.keys(NATIVE_DROP_PATHS).sort()).toEqual([...OUTCOMES].sort());
		const h = await clipboardHarness();
		for (const [outcome, path] of Object.entries(NATIVE_DROP_PATHS) as Array<
			[DropOutcome, NonPointerPath]
		>) {
			if (path.kind === 'command') {
				expect(
					typeof (h.commands as unknown as Record<string, unknown>)[path.command],
					outcome,
				).toBe('function');
				if (path.command !== 'linkTo') expect(everything[path.command].visible, outcome).toBe(true);
			} else if (path.kind === 'menu') {
				expect(menuIds, outcome).toContain(path.item);
			} else {
				expect(path.why.length, outcome).toBeGreaterThan(10);
			}
		}
	});

	it('reaches a drag out of the window through Copy and Cut, which put the files on the system clipboard', async () => {
		const h = await clipboardHarness();
		for (const [action, path] of Object.entries(OUTBOUND_PATHS)) {
			expect(path.kind, action).toBe('command');
			if (path.kind !== 'command') continue;
			expect(typeof (h.commands as unknown as Record<string, unknown>)[path.command], action).toBe(
				'function',
			);
			expect(everything[path.command as 'copy' | 'cut'].visible, action).toBe(true);
		}
	});
});
