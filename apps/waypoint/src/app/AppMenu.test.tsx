// Tests for the application menu in the title bar: it opens by pointer and keyboard and runs the registry's commands
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandBridgeProvider, createCommandBridge } from '../commands/commandBridge';
import { idleActions } from '../commands/commandEnv';
import { t } from '../i18n/messages';
import { entry, factsFor } from '../test/commandFacts';
import { AppMenu } from './AppMenu';

afterEach(cleanup);

function setup(state = {}, extra = {}) {
	const actions = {
		...idleActions(),
		newTab: vi.fn(),
		newWindow: vi.fn(),
		setViewMode: vi.fn(),
		openSettings: vi.fn(),
		undoEntry: vi.fn(),
		openPalette: vi.fn(),
		redoEntry: vi.fn(),
		toggleSidebar: vi.fn(),
		files: {
			newFolder: vi.fn().mockResolvedValue(undefined),
			moveToTrash: vi.fn().mockResolvedValue(undefined),
		} as never,
	};
	const bridge = createCommandBridge({
		facts: factsFor(
			{
				selected: 1,
				undo: entry(3, 'Move 1 item to Trash'),
				redo: entry(1, 'New folder', { undoable: false, redoable: true }),
				...state,
			},
			extra,
		),
		actions,
	});
	render(
		<CommandBridgeProvider value={bridge}>
			<AppMenu />
		</CommandBridgeProvider>,
	);
	return { actions, bridge, button: screen.getByRole('button', { name: t('app.name') }) };
}

describe('the application menu button', () => {
	it('is the app-name button, a menu button in the title bar', () => {
		const { button } = setup();
		expect(button).toHaveAttribute('aria-haspopup', 'menu');
		expect(button).toHaveAttribute('aria-expanded', 'false');
	});

	it('opens a menu of File, Edit, View and Window', async () => {
		const { button } = setup();
		await userEvent.click(button);
		const menu = screen.getByRole('menu', { name: t('app.name') });
		expect(
			within(menu)
				.getAllByRole('menuitem')
				.map((item) => item.textContent),
		).toEqual([
			expect.stringContaining('File'),
			expect.stringContaining('Edit'),
			expect.stringContaining('View'),
			expect.stringContaining('Window'),
		]);
	});

	it('runs a command from a submenu and closes', async () => {
		const { button, actions } = setup();
		await userEvent.click(button);
		await userEvent.click(screen.getByRole('menuitem', { name: /File/ }));
		await userEvent.click(await screen.findByRole('menuitem', { name: /New Tab/ }));
		expect(actions.newTab).toHaveBeenCalledTimes(1);
		await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument());
		expect(button).toHaveFocus();
	});

	it('shows the shortcut beside each item', async () => {
		const { button } = setup();
		await userEvent.click(button);
		await userEvent.click(screen.getByRole('menuitem', { name: /File/ }));
		const item = await screen.findByRole('menuitem', { name: /New Window/ });
		expect(within(item).getByText('Ctrl+Shift+N')).toBeInTheDocument();
	});

	it('does not run a disabled item and says why in its tooltip', async () => {
		const { button, actions } = setup({ selected: 0 });
		await userEvent.click(button);
		await userEvent.click(screen.getByRole('menuitem', { name: /File/ }));
		const trash = await screen.findByRole('menuitem', { name: /Move to Trash/ });
		expect(trash).toHaveAttribute('aria-disabled', 'true');
		expect(trash).toHaveAttribute('title', t('cmd.reason.nothingSelected'));
		await userEvent.click(trash);
		expect(
			(actions.files as unknown as { moveToTrash: () => void }).moveToTrash,
		).not.toHaveBeenCalled();
	});

	it('opens on F10 with the first menu focused, and on Alt plus a mnemonic with that menu open', async () => {
		setup();
		fireEvent.keyDown(window, { key: 'F10' });
		expect(await screen.findByRole('menuitem', { name: /File/ })).toHaveFocus();
		fireEvent.keyDown(document.activeElement!, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument());

		fireEvent.keyDown(window, { key: 'v', altKey: true });
		const list = await screen.findByRole('menuitemcheckbox', { name: /List/ });
		expect(list).toHaveFocus();
		expect(screen.getByRole('menu', { name: t('appMenu.view') })).toBeInTheDocument();
	});

	it('toggles on a lone Alt and on F10: a second press closes the menu and returns focus', async () => {
		const { button } = setup();
		fireEvent.keyDown(window, { key: 'Alt', altKey: true });
		fireEvent.keyUp(window, { key: 'Alt' });
		const file = await screen.findByRole('menuitem', { name: /File/ });
		fireEvent.keyDown(file, { key: 'Alt', altKey: true });
		fireEvent.keyUp(file, { key: 'Alt' });
		await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument());
		expect(button).toHaveFocus();

		fireEvent.keyDown(window, { key: 'F10' });
		fireEvent.keyDown(await screen.findByRole('menuitem', { name: /File/ }), { key: 'F10' });
		await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument());
		expect(button).toHaveFocus();
	});

	it('is plain text that opens nothing while the menu bar carries the menus', async () => {
		render(
			<CommandBridgeProvider
				value={createCommandBridge({ facts: factsFor({}, {}), actions: idleActions() })}
			>
				<AppMenu menuBar />
			</CommandBridgeProvider>,
		);
		expect(screen.queryByRole('button', { name: t('app.name') })).not.toBeInTheDocument();
		expect(screen.getByText(t('app.name'))).toBeInTheDocument();
		fireEvent.keyDown(window, { key: 'F10' });
		fireEvent.keyDown(window, { key: 'f', altKey: true });
		expect(screen.queryByRole('menu')).not.toBeInTheDocument();
	});

	it('runs a view command with the keyboard', async () => {
		const { actions } = setup();
		fireEvent.keyDown(window, { key: 'v', altKey: true });
		const grid = await screen.findByRole('menuitemcheckbox', { name: /Grid/ });
		fireEvent.keyDown(document.activeElement!, { key: 'ArrowDown' });
		expect(grid).toHaveFocus();
		fireEvent.keyDown(grid, { key: 'Enter' });
		expect(actions.setViewMode).toHaveBeenCalledWith('grid');
	});

	it('opens Settings from the Edit menu', async () => {
		const { actions } = setup();
		fireEvent.keyDown(window, { key: 'e', altKey: true });
		await userEvent.click(await screen.findByRole('menuitem', { name: /Settings/ }));
		expect(actions.openSettings).toHaveBeenCalledTimes(1);
	});

	async function openHistory() {
		fireEvent.keyDown(window, { key: 'e', altKey: true });
		await userEvent.click(
			await screen.findByRole('menuitem', { name: new RegExp(t('appMenu.history')) }),
		);
		return within(await screen.findByRole('menu', { name: t('appMenu.history') }));
	}

	it('lists the history under Edit and undoes only its newest entry from there', async () => {
		const { actions } = setup();
		const history = await openHistory();
		const head = history.getByRole('menuitem', { name: /Move 1 item to Trash/ });
		const redoHead = history.getByRole('menuitem', { name: /New folder/ });
		expect(head).not.toHaveAttribute('aria-disabled');
		expect(redoHead).not.toHaveAttribute('aria-disabled');
		await userEvent.click(head);
		expect(actions.undoEntry).toHaveBeenCalledWith(3);
		expect(actions.redoEntry).not.toHaveBeenCalled();
	});

	it('redoes the redo head from the history', async () => {
		const { actions } = setup();
		const history = await openHistory();
		await userEvent.click(history.getByRole('menuitem', { name: /New folder/ }));
		expect(actions.redoEntry).toHaveBeenCalledWith(1);
	});

	it('ends the history with More in Command Palette…, which opens the palette on "undo"', async () => {
		const { actions } = setup();
		const history = await openHistory();
		const items = history.getAllByRole('menuitem');
		expect(items.at(-1)).toHaveTextContent(t('appMenu.history.more'));
		await userEvent.click(items.at(-1)!);
		expect(actions.openPalette).toHaveBeenCalledWith('undo');
		expect(actions.undoEntry).not.toHaveBeenCalled();
	});

	it('opens the palette from View → Command Palette…, with its key shown', async () => {
		const { actions } = setup();
		fireEvent.keyDown(window, { key: 'v', altKey: true });
		const item = await screen.findByRole('menuitem', { name: /Command Palette…/ });
		expect(within(item).getByText('Ctrl+Shift+P')).toBeInTheDocument();
		await userEvent.click(item);
		expect(actions.openPalette).toHaveBeenCalledWith();
	});

	it('lists older entries read-only', async () => {
		setup({}, { history: [entry(3, 'Move 1 item to Trash'), entry(2, 'New file')], undoHead: 3 });
		const history = await openHistory();
		expect(history.getByRole('menuitem', { name: /New file/ })).toHaveAttribute(
			'aria-disabled',
			'true',
		);
	});

	it('follows the window: the menu rebuilds when the facts change', async () => {
		const { button, bridge } = setup({ selected: 0 });
		act(() => bridge.patchFacts(factsFor({ selected: 2 })));
		await userEvent.click(button);
		await userEvent.click(screen.getByRole('menuitem', { name: /File/ }));
		const trash = await screen.findByRole('menuitem', { name: /Move to Trash/ });
		expect(trash).not.toHaveAttribute('aria-disabled');
	});
});
