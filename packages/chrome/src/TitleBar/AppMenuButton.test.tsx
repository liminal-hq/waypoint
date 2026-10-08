// Tests for the app menu button: opening by pointer, F10, a lone Alt and Alt plus a mnemonic, and focus on close
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { ContextMenuProps } from '../ContextMenu/ContextMenu';
import type { MenuItem } from '../ContextMenu/types';
import { AppMenuButton } from './AppMenuButton';

const items: MenuItem[] = [
	{
		type: 'submenu',
		id: 'menu:file',
		label: 'File',
		items: [
			{ type: 'action', id: 'new', label: 'New Tab', shortcut: 'Ctrl+T' },
			{ type: 'action', id: 'close', label: 'Close Tab' },
		],
	},
	{
		type: 'submenu',
		id: 'menu:edit',
		label: 'Edit',
		items: [{ type: 'action', id: 'undo', label: 'Undo' }],
	},
];

const mnemonics = { f: 'menu:file', e: 'menu:edit' };

function setup(props: Partial<Parameters<typeof AppMenuButton>[0]> = {}) {
	const onSelect = vi.fn();
	render(
		<AppMenuButton
			label="Waypoint"
			items={items}
			onSelect={onSelect}
			mnemonics={mnemonics}
			{...props}
		/>,
	);
	return { onSelect, button: screen.getByRole('button', { name: 'Waypoint' }) };
}

const key = (init: KeyboardEventInit) => fireEvent.keyDown(window, init);

describe('AppMenuButton', () => {
	it('is a menu button that reports whether its menu is open', async () => {
		const { button } = setup();
		expect(button).toHaveAttribute('aria-haspopup', 'menu');
		expect(button).toHaveAttribute('aria-expanded', 'false');
		await userEvent.click(button);
		expect(button).toHaveAttribute('aria-expanded', 'true');
		expect(screen.getByRole('menu', { name: 'Waypoint' })).toBeInTheDocument();
		await userEvent.click(button);
		expect(screen.queryByRole('menu')).not.toBeInTheDocument();
	});

	it('opens on F10 with the first item focused, and Escape returns focus to the button', async () => {
		const { button } = setup();
		key({ key: 'F10' });
		const first = await screen.findByRole('menuitem', { name: 'File' });
		expect(first).toHaveFocus();
		fireEvent.keyDown(first, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument());
		expect(button).toHaveFocus();
	});

	it('opens on a lone Alt press but not when Alt is part of a chord', async () => {
		setup();
		key({ key: 'Alt', altKey: true });
		fireEvent.keyUp(window, { key: 'Alt' });
		expect(await screen.findByRole('menu')).toBeInTheDocument();
	});

	it('closes on a second lone Alt press and returns focus to where it was', async () => {
		const { button } = setup();
		key({ key: 'Alt', altKey: true });
		fireEvent.keyUp(window, { key: 'Alt' });
		const first = await screen.findByRole('menuitem', { name: 'File' });
		fireEvent.keyDown(first, { key: 'Alt', altKey: true });
		fireEvent.keyUp(first, { key: 'Alt' });
		await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument());
		expect(button).toHaveFocus();
	});

	it('closes on a second F10 and reopens on a third', async () => {
		const { button } = setup();
		key({ key: 'F10' });
		const first = await screen.findByRole('menuitem', { name: 'File' });
		fireEvent.keyDown(first, { key: 'F10' });
		await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument());
		expect(button).toHaveFocus();
		key({ key: 'F10' });
		expect(await screen.findByRole('menu')).toBeInTheDocument();
	});

	it('keeps a menu open when Alt is used as a chord inside it', async () => {
		setup();
		key({ key: 'f', altKey: true });
		const newTab = await screen.findByRole('menuitem', { name: /New Tab/ });
		fireEvent.keyDown(newTab, { key: 'Alt', altKey: true });
		fireEvent.keyDown(newTab, { key: '3', altKey: true });
		fireEvent.keyUp(newTab, { key: 'Alt' });
		expect(screen.getByRole('menu', { name: 'File' })).toBeInTheDocument();
	});

	it('Alt plus the same mnemonic again leaves that menu open with its first item focused', async () => {
		setup();
		key({ key: 'f', altKey: true });
		const newTab = await screen.findByRole('menuitem', { name: /New Tab/ });
		fireEvent.keyDown(newTab, { key: 'f', altKey: true });
		expect(await screen.findByRole('menuitem', { name: /New Tab/ })).toHaveFocus();
		expect(screen.getAllByRole('menu')).toHaveLength(2);
	});

	it('does not open on Alt used with another key', () => {
		setup();
		key({ key: 'Alt', altKey: true });
		key({ key: '3', altKey: true });
		fireEvent.keyUp(window, { key: 'Alt' });
		expect(screen.queryByRole('menu')).not.toBeInTheDocument();
	});

	it('opens the menu named by an Alt mnemonic with its first item focused', async () => {
		setup();
		key({ key: 'f', altKey: true });
		const newTab = await screen.findByRole('menuitem', { name: /New Tab/ });
		expect(newTab).toHaveFocus();
		expect(screen.getByRole('menu', { name: 'File' })).toBeInTheDocument();
	});

	it('opens a different menu for each mnemonic, in either case', async () => {
		setup();
		key({ key: 'E', altKey: true });
		expect(await screen.findByRole('menuitem', { name: 'Undo' })).toHaveFocus();
	});

	it('leaves Alt plus an unbound letter, and the chords with Ctrl or Shift, alone', () => {
		setup();
		key({ key: 'h', altKey: true });
		key({ key: 'f', altKey: true, ctrlKey: true });
		key({ key: 'f', altKey: true, shiftKey: true });
		expect(screen.queryByRole('menu')).not.toBeInTheDocument();
	});

	it('ignores every accelerator when they are switched off', () => {
		setup({ acceleratorKeys: false });
		key({ key: 'F10' });
		key({ key: 'f', altKey: true });
		expect(screen.queryByRole('menu')).not.toBeInTheDocument();
	});

	it('reports the chosen item and closes', async () => {
		const { onSelect } = setup();
		key({ key: 'f', altKey: true });
		await userEvent.click(await screen.findByRole('menuitem', { name: /Close Tab/ }));
		expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: 'close' }));
		expect(screen.queryByRole('menu')).not.toBeInTheDocument();
	});

	it('navigates into a submenu with ArrowRight and back with ArrowLeft', async () => {
		setup();
		key({ key: 'F10' });
		const file = await screen.findByRole('menuitem', { name: 'File' });
		fireEvent.keyDown(file, { key: 'ArrowRight' });
		const inner = await screen.findByRole('menuitem', { name: /New Tab/ });
		expect(inner).toHaveFocus();
		fireEvent.keyDown(inner, { key: 'ArrowLeft' });
		await waitFor(() => expect(screen.getByRole('menuitem', { name: 'File' })).toHaveFocus());
	});

	describe('with a host that draws the menu', () => {
		const hosted = vi.fn((props: ContextMenuProps) => (
			<button type="button" onClick={props.onClose}>
				hosted menu
			</button>
		));

		it('hands a press on the button to the host, with the menu and its position', async () => {
			hosted.mockClear();
			const { button } = setup({ renderPointerMenu: hosted });
			await userEvent.click(button);
			expect(screen.getByRole('button', { name: 'hosted menu' })).toBeInTheDocument();
			expect(screen.queryByRole('menu')).not.toBeInTheDocument();
			expect(hosted).toHaveBeenCalledWith(
				expect.objectContaining({ items, ariaLabel: 'Waypoint', openedWithKeyboard: false }),
			);
		});

		it('closes when the host closes it', async () => {
			const { button } = setup({ renderPointerMenu: hosted });
			await userEvent.click(button);
			await userEvent.click(screen.getByRole('button', { name: 'hosted menu' }));
			expect(screen.queryByRole('button', { name: 'hosted menu' })).not.toBeInTheDocument();
			expect(button).toHaveAttribute('aria-expanded', 'false');
		});

		it('keeps the keyboard openings on the chrome menu, which has the arrow keys and the mnemonics', async () => {
			hosted.mockClear();
			setup({ renderPointerMenu: hosted });
			key({ key: 'F10' });
			expect(await screen.findByRole('menu')).toBeInTheDocument();
			key({ key: 'F10' });
			key({ key: 'f', altKey: true });
			expect(await screen.findByRole('menuitem', { name: /New Tab/ })).toBeInTheDocument();
			expect(hosted).not.toHaveBeenCalled();
		});
	});
});
