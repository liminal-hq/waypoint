// Tests for the menu bar: roles, roving focus, opening, switching between open menus, Alt and F10, and overflow
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { ContextMenuProps } from '../ContextMenu/ContextMenu';
import type { MenuItem } from '../ContextMenu/types';
import { MenuBar } from './MenuBar';

const items: MenuItem[] = [
	{
		type: 'submenu',
		id: 'menu:file',
		label: 'File',
		items: [
			{ type: 'action', id: 'new', label: 'New Tab' },
			{ type: 'action', id: 'close', label: 'Close Tab' },
		],
	},
	{
		type: 'submenu',
		id: 'menu:edit',
		label: 'Edit',
		items: [{ type: 'action', id: 'undo', label: 'Undo' }],
	},
	{
		type: 'submenu',
		id: 'menu:view',
		label: 'View',
		items: [{ type: 'action', id: 'grid', label: 'Grid' }],
	},
];
const mnemonics = { f: 'menu:file', e: 'menu:edit', v: 'menu:view' };

function setup(extra: { before?: boolean } = {}) {
	const onSelect = vi.fn();
	render(
		<>
			{extra.before ? <button type="button">Before</button> : null}
			<MenuBar
				items={items}
				onSelect={onSelect}
				label="Menu bar"
				moreLabel="More"
				mnemonics={mnemonics}
			/>
		</>,
	);
	return {
		onSelect,
		bar: screen.getByRole('menubar', { name: 'Menu bar' }),
		file: screen.getByRole('menuitem', { name: 'File' }),
		edit: screen.getByRole('menuitem', { name: 'Edit' }),
		view: screen.getByRole('menuitem', { name: 'View' }),
	};
}

const windowKey = (init: KeyboardEventInit) => fireEvent.keyDown(window, init);
const loneAlt = (target: Element | Window = window) => {
	fireEvent.keyDown(target, { key: 'Alt', altKey: true });
	fireEvent.keyUp(target, { key: 'Alt' });
};

afterEach(() => vi.restoreAllMocks());

describe('MenuBar', () => {
	it('is a menubar of menu buttons with one tab stop', () => {
		const { file, edit, view } = setup();
		for (const button of [file, edit, view]) {
			expect(button).toHaveAttribute('aria-haspopup', 'menu');
			expect(button).toHaveAttribute('aria-expanded', 'false');
		}
		expect([file, edit, view].map((b) => b.tabIndex)).toEqual([0, -1, -1]);
		expect(file).toHaveAttribute('aria-keyshortcuts', 'Alt+F');
	});

	it('moves along the bar with Left, Right, Home and End, wrapping at the ends', () => {
		const { file, edit, view } = setup();
		file.focus();
		fireEvent.keyDown(file, { key: 'ArrowRight' });
		expect(edit).toHaveFocus();
		expect(edit.tabIndex).toBe(0);
		expect(file.tabIndex).toBe(-1);
		fireEvent.keyDown(edit, { key: 'End' });
		expect(view).toHaveFocus();
		fireEvent.keyDown(view, { key: 'ArrowRight' });
		expect(file).toHaveFocus();
		fireEvent.keyDown(file, { key: 'ArrowLeft' });
		expect(view).toHaveFocus();
		fireEvent.keyDown(view, { key: 'Home' });
		expect(file).toHaveFocus();
	});

	it.each(['ArrowDown', 'Enter', ' '])(
		'opens a menu with %s and focuses its first row',
		async (k) => {
			const { file } = setup();
			file.focus();
			fireEvent.keyDown(file, { key: k });
			expect(await screen.findByRole('menuitem', { name: 'New Tab' })).toHaveFocus();
			expect(file).toHaveAttribute('aria-expanded', 'true');
			expect(screen.getByRole('menu', { name: 'File' })).toBeInTheDocument();
		},
	);

	it('opens by click, and a second click on the same button closes it', async () => {
		const { file } = setup();
		await userEvent.click(file);
		expect(screen.getByRole('menu', { name: 'File' })).toBeInTheDocument();
		await userEvent.click(file);
		expect(screen.queryByRole('menu')).not.toBeInTheDocument();
	});

	it('closes on Escape with focus back on the menu button', async () => {
		const { file } = setup();
		file.focus();
		fireEvent.keyDown(file, { key: 'Enter' });
		fireEvent.keyDown(await screen.findByRole('menuitem', { name: 'New Tab' }), { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument());
		expect(file).toHaveFocus();
	});

	it('reports the chosen row and closes', async () => {
		const { onSelect, file } = setup();
		await userEvent.click(file);
		await userEvent.click(screen.getByRole('menuitem', { name: 'Close Tab' }));
		expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: 'close' }));
		expect(screen.queryByRole('menu')).not.toBeInTheDocument();
	});

	it('switches to the neighbouring menu with Right and Left while one is open', async () => {
		const { file, edit, view } = setup();
		file.focus();
		fireEvent.keyDown(file, { key: 'Enter' });
		fireEvent.keyDown(await screen.findByRole('menuitem', { name: 'New Tab' }), {
			key: 'ArrowRight',
		});
		expect(await screen.findByRole('menuitem', { name: 'Undo' })).toHaveFocus();
		expect(edit).toHaveAttribute('aria-expanded', 'true');
		expect(file).toHaveAttribute('aria-expanded', 'false');
		fireEvent.keyDown(screen.getByRole('menuitem', { name: 'Undo' }), { key: 'ArrowLeft' });
		expect(await screen.findByRole('menuitem', { name: 'New Tab' })).toHaveFocus();
		fireEvent.keyDown(screen.getByRole('menuitem', { name: 'New Tab' }), { key: 'ArrowLeft' });
		expect(await screen.findByRole('menuitem', { name: 'Grid' })).toHaveFocus();
		expect(view).toHaveAttribute('aria-expanded', 'true');
		expect(screen.getAllByRole('menu')).toHaveLength(1);
	});

	it('opens another menu on hover while one is open, and not when none is', async () => {
		const { file, edit } = setup();
		await userEvent.hover(edit);
		expect(screen.queryByRole('menu')).not.toBeInTheDocument();
		await userEvent.click(file);
		await userEvent.hover(edit);
		expect(await screen.findByRole('menu', { name: 'Edit' })).toBeInTheDocument();
		expect(screen.queryByRole('menu', { name: 'File' })).not.toBeInTheDocument();
	});

	it('opens the menu named by Alt plus a letter, from anywhere', async () => {
		setup({ before: true });
		windowKey({ key: 'e', altKey: true });
		expect(await screen.findByRole('menuitem', { name: 'Undo' })).toHaveFocus();
		windowKey({ key: 'h', altKey: true });
		expect(screen.getAllByRole('menu')).toHaveLength(1);
	});

	it('focuses the first menu on a lone Alt and returns focus on a second one', () => {
		const { file } = setup({ before: true });
		const before = screen.getByRole('button', { name: 'Before' });
		before.focus();
		loneAlt();
		expect(file).toHaveFocus();
		loneAlt();
		expect(before).toHaveFocus();
	});

	it('does the same with F10, and Escape also returns focus', () => {
		const { file, bar } = setup({ before: true });
		const before = screen.getByRole('button', { name: 'Before' });
		before.focus();
		windowKey({ key: 'F10' });
		expect(file).toHaveFocus();
		windowKey({ key: 'F10' });
		expect(before).toHaveFocus();
		windowKey({ key: 'F10' });
		expect(file).toHaveFocus();
		fireEvent.keyDown(file, { key: 'Escape' });
		expect(before).toHaveFocus();
		expect(bar).toBeInTheDocument();
	});

	it('does not move focus for Alt used as part of a chord', () => {
		const { file } = setup({ before: true });
		const before = screen.getByRole('button', { name: 'Before' });
		before.focus();
		windowKey({ key: 'Alt', altKey: true });
		windowKey({ key: '3', altKey: true });
		fireEvent.keyUp(window, { key: 'Alt' });
		expect(before).toHaveFocus();
		expect(file).not.toHaveFocus();
	});

	it('closes an open menu on a lone Alt, leaving focus on its button', async () => {
		const { file } = setup();
		file.focus();
		fireEvent.keyDown(file, { key: 'Enter' });
		const first = await screen.findByRole('menuitem', { name: 'New Tab' });
		loneAlt(first);
		await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument());
		expect(file).toHaveFocus();
	});

	it('collapses the trailing menus into More when the bar is narrow', async () => {
		const widths = new Map([
			['File', 50],
			['Edit', 50],
			['View', 50],
			['More', 40],
		]);
		vi.spyOn(HTMLElement.prototype, 'offsetWidth', 'get').mockImplementation(function (
			this: HTMLElement,
		) {
			return widths.get(this.textContent ?? '') ?? 0;
		});
		vi.spyOn(HTMLElement.prototype, 'clientWidth', 'get').mockImplementation(function (
			this: HTMLElement,
		) {
			return this.getAttribute('role') === 'menubar' ? 110 : 0;
		});
		render(
			<MenuBar
				items={items}
				onSelect={vi.fn()}
				label="Menu bar"
				moreLabel="More"
				mnemonics={mnemonics}
			/>,
		);
		const more = await screen.findByRole('menuitem', { name: 'More' });
		expect(more).toHaveAttribute('aria-haspopup', 'menu');
		await userEvent.click(more);
		const menu = await screen.findByRole('menu', { name: 'More' });
		expect(menu).toHaveTextContent('Edit');
		expect(menu).toHaveTextContent('View');
		expect(menu).not.toHaveTextContent('File');
	});

	describe('with a host that draws the menus', () => {
		const hosted = vi.fn((props: ContextMenuProps) => (
			<button type="button" onClick={props.onClose}>
				{`hosted ${props.ariaLabel ?? ''}`}
			</button>
		));
		const renderHosted = () =>
			render(
				<MenuBar
					items={items}
					onSelect={vi.fn()}
					label="Menu bar"
					moreLabel="More"
					mnemonics={mnemonics}
					renderMenu={hosted}
				/>,
			);

		it('hands a click on a menu to the host, with that menu’s rows and its position', async () => {
			hosted.mockClear();
			renderHosted();
			await userEvent.click(screen.getByRole('menuitem', { name: 'Edit' }));
			expect(screen.getByRole('button', { name: 'hosted Edit' })).toBeInTheDocument();
			expect(screen.queryByRole('menu')).not.toBeInTheDocument();
			expect(hosted).toHaveBeenCalledWith(
				expect.objectContaining({
					items: [expect.objectContaining({ id: 'undo' })],
					openedWithKeyboard: false,
				}),
			);
		});

		it('hands the keyboard openings to the host too, each menu on its own', async () => {
			hosted.mockClear();
			renderHosted();
			windowKey({ key: 'f', altKey: true });
			expect(await screen.findByRole('button', { name: 'hosted File' })).toBeInTheDocument();
			expect(hosted).toHaveBeenLastCalledWith(
				expect.objectContaining({
					openedWithKeyboard: true,
					items: [expect.objectContaining({ id: 'new' }), expect.objectContaining({ id: 'close' })],
				}),
			);
			expect(screen.queryByRole('menu')).not.toBeInTheDocument();
		});
	});
});
