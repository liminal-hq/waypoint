// Tests for the menu bar under the title bar: the app menu's menus, rows and commands laid across it
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandBridgeProvider, createCommandBridge } from '../commands/commandBridge';
import { idleActions } from '../commands/commandEnv';
import { t } from '../i18n/messages';
import { entry, factsFor } from '../test/commandFacts';
import { AppMenuBar } from './AppMenuBar';

afterEach(cleanup);

function setup() {
	const actions = { ...idleActions(), newTab: vi.fn(), setViewMode: vi.fn(), undoEntry: vi.fn() };
	const bridge = createCommandBridge({
		facts: factsFor({ selected: 1, undo: entry(3, 'Move 1 item to Trash') }, {}),
		actions,
	});
	render(
		<CommandBridgeProvider value={bridge}>
			<AppMenuBar />
		</CommandBridgeProvider>,
	);
	return { actions, bar: screen.getByRole('menubar', { name: t('menuBar.label') }) };
}

describe('the menu bar', () => {
	it('has a button for each of the app menu’s menus', () => {
		const { bar } = setup();
		expect(
			within(bar)
				.getAllByRole('menuitem')
				.map((item) => item.textContent),
		).toEqual(['File', 'Edit', 'View', 'Window']);
	});

	it('opens a menu with the same rows as the app menu and runs the command', async () => {
		const { actions } = setup();
		await userEvent.click(screen.getByRole('menuitem', { name: 'File' }));
		const menu = screen.getByRole('menu', { name: 'File' });
		expect(within(menu).getByRole('menuitem', { name: /New Folder/ })).toBeInTheDocument();
		await userEvent.click(within(menu).getByRole('menuitem', { name: /New Tab/ }));
		expect(actions.newTab).toHaveBeenCalled();
		expect(screen.queryByRole('menu')).not.toBeInTheDocument();
	});

	it('opens View with Alt+V and lists the view modes as checkable rows', async () => {
		setup();
		fireEvent.keyDown(window, { key: 'v', altKey: true });
		expect(await screen.findByRole('menuitemcheckbox', { name: /List/ })).toBeInTheDocument();
		expect(screen.getByRole('menu', { name: 'View' })).toBeInTheDocument();
	});

	it('runs an Undo History entry from the Edit menu', async () => {
		const { actions } = setup();
		await userEvent.click(screen.getByRole('menuitem', { name: 'Edit' }));
		await userEvent.click(screen.getByRole('menuitem', { name: /Undo History/ }));
		const history = await screen.findByRole('menu', { name: /Undo History/ });
		await userEvent.click(within(history).getByRole('menuitem', { name: /Move 1 item to Trash/ }));
		expect(actions.undoEntry).toHaveBeenCalledWith(3);
	});
});
