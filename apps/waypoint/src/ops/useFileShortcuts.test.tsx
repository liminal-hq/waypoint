// Verifies the file keys: which key runs which command, and where the keys are left to a field, a menu or another area
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ListingSession } from '../browse/useListingSession';
import type { FileCommands } from './fileCommands';
import {
	handleFileKey,
	inFileArea,
	ownsKeys,
	useFileShortcuts,
	type FileShortcutOptions,
	type FileKeyHandlers,
} from './useFileShortcuts';

afterEach(cleanup);

const spies = () =>
	({
		newFolder: vi.fn(),
		newFile: vi.fn(),
		rename: vi.fn(),
		duplicate: vi.fn(),
		moveToTrash: vi.fn(),
		deletePermanently: vi.fn(),
		cut: vi.fn(),
		copy: vi.fn(),
		paste: vi.fn(),
		copyToOtherPane: vi.fn(),
		moveToOtherPane: vi.fn(),
		undo: vi.fn(),
		redo: vi.fn(),
	}) satisfies FileKeyHandlers;

const key = (name: string, mods: Partial<KeyboardEventInit> = {}) => ({
	key: name,
	ctrlKey: false,
	metaKey: false,
	altKey: false,
	shiftKey: false,
	isComposing: false,
	...mods,
});

describe('handleFileKey', () => {
	const cases: Array<[string, Partial<KeyboardEventInit>, keyof FileKeyHandlers]> = [
		['F7', {}, 'newFolder'],
		['F7', { shiftKey: true }, 'newFile'],
		['F2', {}, 'rename'],
		['Delete', {}, 'moveToTrash'],
		['Delete', { shiftKey: true }, 'deletePermanently'],
		['D', { ctrlKey: true, shiftKey: true }, 'duplicate'],
		['d', { metaKey: true, shiftKey: true }, 'duplicate'],
		['x', { ctrlKey: true }, 'cut'],
		['c', { ctrlKey: true }, 'copy'],
		['v', { ctrlKey: true }, 'paste'],
		['V', { metaKey: true }, 'paste'],
		['F5', {}, 'copyToOtherPane'],
		['F5', { shiftKey: true }, 'moveToOtherPane'],
		['z', { ctrlKey: true }, 'undo'],
		['Z', { ctrlKey: true, shiftKey: true }, 'redo'],
	];

	it.each(cases)('runs the right command for %s %o', (name, mods, expected) => {
		const handlers = spies();
		expect(handleFileKey(key(name, mods), handlers)).toBe(true);
		for (const [id, spy] of Object.entries(handlers)) {
			expect(spy.mock.calls.length, id).toBe(id === expected ? 1 : 0);
		}
	});

	it('leaves New Window (Ctrl+Shift+N), batch rename (Ctrl+F2), Alt+Delete and plain letters alone', () => {
		const handlers = spies();
		for (const event of [
			key('N', { ctrlKey: true, shiftKey: true }),
			key('F2', { ctrlKey: true }),
			key('Delete', { altKey: true }),
			key('d'),
			key('z'),
			key('F2', { shiftKey: true }),
			key('F7', { ctrlKey: true }),
			key('C', { ctrlKey: true, shiftKey: true }),
			key('v', { ctrlKey: true, altKey: true }),
			key('c'),
			key('F5', { ctrlKey: true }),
		]) {
			expect(handleFileKey(event, handlers), event.key).toBe(false);
		}
		expect(Object.values(handlers).every((spy) => spy.mock.calls.length === 0)).toBe(true);
	});

	it('ignores a key typed during IME composition', () => {
		const handlers = spies();
		expect(handleFileKey(key('F2', { isComposing: true }), handlers)).toBe(false);
	});
});

describe('where the keys belong to something else', () => {
	it('leaves keys to a text field, a menu and a dialog', () => {
		document.body.innerHTML = `<input id="i"><div role="menu"><button id="m"></button></div><dialog open><button id="d"></button></dialog><div contenteditable="true" id="c"></div><div id="p" data-pane="1"><span id="row"></span></div><div id="side"></div>`;
		for (const id of ['i', 'm', 'd', 'c']) {
			expect(ownsKeys(document.getElementById(id)), id).toBe(true);
		}
		expect(ownsKeys(document.getElementById('row'))).toBe(false);
		expect(inFileArea(document.getElementById('row'))).toBe(true);
		expect(inFileArea(document.getElementById('side'))).toBe(false);
		expect(inFileArea(document.body)).toBe(true);
	});
});

describe('useFileShortcuts', () => {
	function mount(commands: Partial<FileCommands> | null, options: FileShortcutOptions = {}) {
		function Host() {
			useFileShortcuts(commands as FileCommands | null, options);
			return (
				<div>
					<input aria-label="field" />
					<div data-pane="1">
						<button>row</button>
					</div>
					<aside>
						<button>sidebar</button>
					</aside>
				</div>
			);
		}
		return render(<Host />);
	}
	const commands = () => ({
		newFolder: vi.fn(() => Promise.resolve()),
		newFile: vi.fn(() => Promise.resolve()),
		rename: vi.fn(),
		duplicate: vi.fn(() => Promise.resolve()),
		moveToTrash: vi.fn(() => Promise.resolve()),
		deletePermanently: vi.fn(() => Promise.resolve()),
		cut: vi.fn(() => Promise.resolve()),
		copy: vi.fn(() => Promise.resolve()),
		paste: vi.fn(() => Promise.resolve()),
		copyToOtherPane: vi.fn(() => Promise.resolve()),
		moveToOtherPane: vi.fn(() => Promise.resolve()),
		undo: vi.fn(() => Promise.resolve()),
		redo: vi.fn(() => Promise.resolve()),
	});

	it('runs a command and stops the browser’s own handling of the key', () => {
		const spy = commands();
		const { getByText } = mount(spy);
		const row = getByText('row');
		expect(fireEvent.keyDown(row, { key: 'F7' })).toBe(false);
		expect(spy.newFolder).toHaveBeenCalledTimes(1);
		fireEvent.keyDown(row, { key: 'Delete', shiftKey: true });
		expect(spy.deletePermanently).toHaveBeenCalledTimes(1);
		fireEvent.keyDown(row, { key: 'z', ctrlKey: true });
		expect(spy.undo).toHaveBeenCalledTimes(1);
	});

	it('lets a text field keep its own undo, delete and F2', () => {
		const spy = commands();
		const { getByLabelText } = mount(spy);
		const field = getByLabelText('field');
		fireEvent.keyDown(field, { key: 'z', ctrlKey: true });
		fireEvent.keyDown(field, { key: 'Delete' });
		fireEvent.keyDown(field, { key: 'F2' });
		fireEvent.keyDown(field, { key: 'F7' });
		fireEvent.keyDown(field, { key: 'c', ctrlKey: true });
		fireEvent.keyDown(field, { key: 'v', ctrlKey: true });
		fireEvent.keyDown(field, { key: 'F5' });
		expect(Object.values(spy).every((fn) => fn.mock.calls.length === 0)).toBe(true);
	});

	it('acts on the selection only in the file area, not in the sidebar, while F7 and Undo work anywhere', () => {
		const spy = commands();
		const { getByText } = mount(spy);
		const side = getByText('sidebar');
		fireEvent.keyDown(side, { key: 'Delete' });
		fireEvent.keyDown(side, { key: 'F2' });
		fireEvent.keyDown(side, { key: 'D', ctrlKey: true, shiftKey: true });
		fireEvent.keyDown(side, { key: 'c', ctrlKey: true });
		fireEvent.keyDown(side, { key: 'v', ctrlKey: true });
		fireEvent.keyDown(side, { key: 'F5' });
		expect(spy.copy).not.toHaveBeenCalled();
		expect(spy.paste).not.toHaveBeenCalled();
		expect(spy.copyToOtherPane).not.toHaveBeenCalled();
		expect(spy.moveToTrash).not.toHaveBeenCalled();
		expect(spy.rename).not.toHaveBeenCalled();
		expect(spy.duplicate).not.toHaveBeenCalled();
		fireEvent.keyDown(side, { key: 'F7' });
		fireEvent.keyDown(side, { key: 'z', ctrlKey: true });
		expect(spy.newFolder).toHaveBeenCalledTimes(1);
		expect(spy.undo).toHaveBeenCalledTimes(1);
	});

	it('means Delete Permanently in the Trash, and leaves the write keys alone there', () => {
		const spy = commands();
		const deleteInTrash = vi.fn();
		const trash = { model: { layout: 'trash' } } as unknown as ListingSession;
		mount(spy, { activeSession: () => trash, deleteInTrash });
		const pane = document.querySelector('[data-pane] button') as HTMLElement;
		fireEvent.keyDown(pane, { key: 'Delete' });
		fireEvent.keyDown(pane, { key: 'Delete', shiftKey: true });
		expect(deleteInTrash).toHaveBeenCalledTimes(2);
		expect(deleteInTrash).toHaveBeenCalledWith(trash);
		fireEvent.keyDown(pane, { key: 'F7' });
		fireEvent.keyDown(pane, { key: 'F7', shiftKey: true });
		fireEvent.keyDown(pane, { key: 'F2' });
		fireEvent.keyDown(pane, { key: 'd', ctrlKey: true, shiftKey: true });
		fireEvent.keyDown(pane, { key: 'c', ctrlKey: true });
		fireEvent.keyDown(pane, { key: 'x', ctrlKey: true });
		fireEvent.keyDown(pane, { key: 'v', ctrlKey: true });
		fireEvent.keyDown(pane, { key: 'F5' });
		for (const id of [
			'newFolder',
			'newFile',
			'rename',
			'duplicate',
			'moveToTrash',
			'cut',
			'copy',
			'paste',
			'copyToOtherPane',
		] as const) {
			expect(spy[id], id).not.toHaveBeenCalled();
		}
		expect(spy.deletePermanently).not.toHaveBeenCalled();
		fireEvent.keyDown(pane, { key: 'z', ctrlKey: true });
		expect(spy.undo).toHaveBeenCalled();
	});

	it('copies, cuts and pastes with the clipboard keys, and F5 and Shift+F5 go to the other pane, from the file area', () => {
		const spy = commands();
		const { getByText } = mount(spy);
		const row = getByText('row');
		expect(fireEvent.keyDown(row, { key: 'c', ctrlKey: true })).toBe(false);
		fireEvent.keyDown(row, { key: 'x', ctrlKey: true });
		fireEvent.keyDown(row, { key: 'v', ctrlKey: true });
		fireEvent.keyDown(row, { key: 'F5' });
		fireEvent.keyDown(row, { key: 'F5', shiftKey: true });
		expect(spy.copy).toHaveBeenCalledTimes(1);
		expect(spy.cut).toHaveBeenCalledTimes(1);
		expect(spy.paste).toHaveBeenCalledTimes(1);
		expect(spy.copyToOtherPane).toHaveBeenCalledTimes(1);
		expect(spy.moveToOtherPane).toHaveBeenCalledTimes(1);
	});

	it('moves to the Trash with Delete in an ordinary folder', () => {
		const spy = commands();
		const deleteInTrash = vi.fn();
		const folder = { model: { layout: 'folder' } } as unknown as ListingSession;
		mount(spy, { activeSession: () => folder, deleteInTrash });
		fireEvent.keyDown(document.querySelector('[data-pane] button') as HTMLElement, {
			key: 'Delete',
		});
		expect(spy.moveToTrash).toHaveBeenCalledTimes(1);
		expect(deleteInTrash).not.toHaveBeenCalled();
	});

	it('does nothing without commands (a window with no queue)', () => {
		const { getByText } = mount(null);
		expect(fireEvent.keyDown(getByText('row'), { key: 'F7' })).toBe(true);
	});
});
