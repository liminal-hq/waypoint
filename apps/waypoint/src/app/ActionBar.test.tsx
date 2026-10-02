// Tests for the Action bar: rendering, availability, running commands, labels, the toolbar's keys, its menu and overflow
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandBridgeProvider, createCommandBridge } from '../commands/commandBridge';
import { idleActions, type CommandFacts } from '../commands/commandEnv';
import { t } from '../i18n/messages';
import { entry, factsFor, type WindowState } from '../test/commandFacts';
import { ActionBar } from './ActionBar';

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
	// Layout mocks of the overflow tests.
	delete (HTMLElement.prototype as { offsetWidth?: number }).offsetWidth;
	delete (HTMLElement.prototype as { clientWidth?: number }).clientWidth;
});

function files() {
	return {
		newFolder: vi.fn().mockResolvedValue(undefined),
		newFile: vi.fn().mockResolvedValue(undefined),
		cut: vi.fn().mockResolvedValue(undefined),
		copy: vi.fn().mockResolvedValue(undefined),
		paste: vi.fn().mockResolvedValue(undefined),
		rename: vi.fn(),
		moveToTrash: vi.fn().mockResolvedValue(undefined),
		undo: vi.fn().mockResolvedValue(undefined),
		redo: vi.fn().mockResolvedValue(undefined),
	};
}

function setup(state: WindowState = {}, extra: Partial<CommandFacts> = {}) {
	const fake = files();
	const actions = {
		...idleActions(),
		files: fake as never,
		setViewMode: vi.fn(),
		changeSort: vi.fn(),
		setActionBar: vi.fn(),
		setActionBarLabels: vi.fn(),
	};
	const bridge = createCommandBridge({ facts: factsFor(state, extra), actions });
	const utils = render(
		<CommandBridgeProvider value={bridge}>
			<ActionBar />
		</CommandBridgeProvider>,
	);
	const button = (action: string) =>
		utils.container.querySelector(`[data-action="${action}"]`) as HTMLElement;
	return { ...utils, bridge, actions, fake, button };
}

const selected = { selected: 2, focused: true, clipboardItems: 1 };

describe('the Action bar', () => {
	it('is a toolbar of labelled buttons', () => {
		setup(selected);
		const toolbar = screen.getByRole('toolbar', { name: t('actionBar.label') });
		expect(toolbar).toHaveAttribute('aria-orientation', 'horizontal');
		const names = within(toolbar)
			.getAllByRole('button')
			.map((button) => button.getAttribute('aria-label'));
		expect(names).toEqual([
			'New',
			'Cut',
			'Copy',
			'Paste',
			'Rename',
			'Delete',
			'Sort',
			'View',
			'Undo',
			'Redo',
		]);
		// The label is shown as text.
		expect(within(toolbar).getByRole('button', { name: 'Paste' })).toHaveTextContent('Paste');
	});

	it('renders nothing when the Action bar is off', () => {
		setup({}, { actionBar: false });
		expect(screen.queryByRole('toolbar')).not.toBeInTheDocument();
	});

	it('appears and disappears with the setting', () => {
		const { bridge } = setup();
		expect(screen.getByRole('toolbar')).toBeInTheDocument();
		act(() => bridge.patchFacts({ actionBar: false }));
		expect(screen.queryByRole('toolbar')).not.toBeInTheDocument();
		act(() => bridge.patchFacts({ actionBar: true }));
		expect(screen.getByRole('toolbar')).toBeInTheDocument();
	});

	it('enables the buttons that have something to act on and announces the rest as disabled', () => {
		const { button } = setup({});
		expect(button('cut')).toHaveAttribute('aria-disabled', 'true');
		expect(button('paste')).toHaveAttribute('aria-disabled', 'true');
		expect(button('undo')).toHaveAttribute('aria-disabled', 'true');
		expect(button('new')).not.toHaveAttribute('aria-disabled');
		// A disabled button says why, to the pointer and to a screen reader.
		expect(button('cut')).toHaveAttribute('title', `Cut — ${t('cmd.reason.nothingSelected')}`);
		const described = button('cut').getAttribute('aria-describedby')!;
		expect(document.getElementById(described)).toHaveTextContent(t('cmd.reason.nothingSelected'));
	});

	it('follows the selection', () => {
		const { button, bridge } = setup({});
		act(() => bridge.patchFacts(factsFor(selected)));
		expect(button('cut')).not.toHaveAttribute('aria-disabled');
		expect(button('paste')).not.toHaveAttribute('aria-disabled');
	});

	it('gives each button a tooltip with its key', () => {
		const { button } = setup(selected);
		expect(button('copy')).toHaveAttribute('title', 'Copy (Ctrl+C)');
		expect(button('moveToTrash')).toHaveAttribute('title', 'Move to Trash (Delete)');
		expect(button('view')).toHaveAttribute('title', 'View: switch to Grid (Ctrl+1)');
	});

	it('runs the command a button stands for through the file commands', async () => {
		const { button, fake } = setup(selected);
		await userEvent.click(button('cut'));
		await userEvent.click(button('copy'));
		await userEvent.click(button('paste'));
		await userEvent.click(button('moveToTrash'));
		await userEvent.click(button('rename'));
		expect(fake.cut).toHaveBeenCalledTimes(1);
		expect(fake.copy).toHaveBeenCalledTimes(1);
		expect(fake.paste).toHaveBeenCalledTimes(1);
		expect(fake.moveToTrash).toHaveBeenCalledTimes(1);
		expect(fake.rename).toHaveBeenCalledTimes(1);
	});

	it('runs Undo and Redo', async () => {
		const { button, fake } = setup({
			undo: entry(2, 'x'),
			redo: entry(1, 'y', { undoable: false, redoable: true }),
		});
		await userEvent.click(button('undo'));
		await userEvent.click(button('redo'));
		expect(fake.undo).toHaveBeenCalledTimes(1);
		expect(fake.redo).toHaveBeenCalledTimes(1);
	});

	it('does nothing when a disabled button is clicked', async () => {
		const { button, fake } = setup({});
		await userEvent.click(button('cut'));
		await userEvent.click(button('moveToTrash'));
		expect(fake.cut).not.toHaveBeenCalled();
		expect(fake.moveToTrash).not.toHaveBeenCalled();
	});

	it('switches the view with the View button', async () => {
		const { button, actions } = setup({}, { viewMode: 'list' });
		await userEvent.click(button('view'));
		expect(actions.setViewMode).toHaveBeenCalledWith('grid');
	});

	it('sizes each button to the 28 px target in its stylesheet', () => {
		// jsdom does not lay out, so check the rule the build ships instead.
		const css = readCss();
		expect(css).toMatch(/\.button\s*\{[^}]*min-width: 28px;[^}]*height: 28px;/s);
	});

	it('only transitions when motion is allowed', () => {
		const css = readCss();
		const transition = css.split('\n').filter((line) => line.includes('transition'));
		expect(transition.length).toBeGreaterThan(0);
		expect(css).toMatch(/@media \(prefers-reduced-motion: no-preference\)\s*\{[^}]*transition/s);
		expect(
			css.replace(/@media \(prefers-reduced-motion: no-preference\)\s*\{[\s\S]*?\n\}\n/, ''),
		).not.toContain('transition');
	});
});

describe('the menu buttons', () => {
	it('opens New with Folder and File and runs the choice', async () => {
		const { button, fake } = setup({});
		expect(button('new')).toHaveAttribute('aria-haspopup', 'menu');
		await userEvent.click(button('new'));
		expect(button('new')).toHaveAttribute('aria-expanded', 'true');
		const menu = screen.getByRole('menu');
		expect(
			within(menu)
				.getAllByRole('menuitem')
				.map((item) => item.textContent),
		).toEqual([expect.stringContaining('New Folder'), expect.stringContaining('New File')]);
		await userEvent.click(within(menu).getByRole('menuitem', { name: /New Folder/ }));
		expect(fake.newFolder).toHaveBeenCalledTimes(1);
		await waitFor(() => expect(screen.queryByRole('menu')).not.toBeInTheDocument());
		expect(button('new')).toHaveFocus();
	});

	it('opens Sort with the current sort checked and re-sorts from a choice', async () => {
		const { button, actions } = setup({});
		await userEvent.click(button('sort'));
		const name = screen.getByRole('menuitemcheckbox', { name: /^Name$/ });
		expect(name).toHaveAttribute('aria-checked', 'true');
		expect(screen.getByRole('menuitemcheckbox', { name: /^Size$/ })).toHaveAttribute(
			'aria-checked',
			'false',
		);
		await userEvent.click(screen.getByRole('menuitemcheckbox', { name: /^Size$/ }));
		expect(actions.changeSort).toHaveBeenCalledTimes(1);
		const change = actions.changeSort.mock.calls[0]![0] as (sort: object) => object;
		expect(
			change({ key: 'name', descending: true, directoriesFirst: true, groupBy: 'none' }),
		).toEqual({
			key: 'size',
			descending: false,
			directoriesFirst: true,
			groupBy: 'none',
		});
	});

	it('opens a menu button with the Down arrow and focuses its first row', async () => {
		const { button } = setup({});
		button('new').focus();
		fireEvent.keyDown(button('new'), { key: 'ArrowDown' });
		expect(await screen.findByRole('menuitem', { name: /New Folder/ })).toHaveFocus();
	});
});

describe('the toolbar keys', () => {
	it('has one tab stop, and the arrows, Home and End move between the buttons', async () => {
		const { container, button } = setup(selected);
		const buttons = [...container.querySelectorAll<HTMLElement>('[data-action]')];
		expect(buttons.filter((item) => item.tabIndex === 0)).toHaveLength(1);
		expect(buttons[0]).toHaveAttribute('tabindex', '0');
		button('new').focus();
		fireEvent.keyDown(button('new'), { key: 'ArrowRight' });
		expect(button('cut')).toHaveFocus();
		expect(button('cut')).toHaveAttribute('tabindex', '0');
		expect(button('new')).toHaveAttribute('tabindex', '-1');
		fireEvent.keyDown(button('cut'), { key: 'End' });
		expect(button('redo')).toHaveFocus();
		fireEvent.keyDown(button('redo'), { key: 'ArrowRight' });
		expect(button('redo')).toHaveFocus();
		fireEvent.keyDown(button('redo'), { key: 'Home' });
		expect(button('new')).toHaveFocus();
		fireEvent.keyDown(button('new'), { key: 'ArrowLeft' });
		expect(button('new')).toHaveFocus();
	});

	it('keeps a disabled button in the focus order', () => {
		const { button } = setup({});
		button('new').focus();
		fireEvent.keyDown(button('new'), { key: 'ArrowRight' });
		expect(button('cut')).toHaveFocus();
		expect(button('cut')).toHaveAttribute('aria-disabled', 'true');
	});

	it('remembers where focus was when it comes back', () => {
		const { button } = setup(selected);
		act(() => button('copy').focus());
		expect(button('copy')).toHaveAttribute('tabindex', '0');
		expect(button('new')).toHaveAttribute('tabindex', '-1');
	});

	it('opens its menu from the keyboard with the Menu key', async () => {
		const { button } = setup(selected);
		button('copy').focus();
		fireEvent.keyDown(button('copy'), { key: 'ContextMenu' });
		expect(await screen.findByRole('menuitem', { name: t('actionBar.hideLabels') })).toHaveFocus();
	});
});

describe('labels and hiding', () => {
	it('shows only the icons, with the names still available, when labels are off', () => {
		const { button } = setup(selected, { actionBarLabels: false });
		expect(button('paste')).toHaveTextContent('');
		expect(button('paste')).toHaveAttribute('aria-label', 'Paste');
		expect(screen.getByRole('toolbar')).toHaveAttribute('data-labels', 'false');
	});

	it('offers Hide Labels and Hide Action Bar on right-click, and applies them', async () => {
		const { actions } = setup(selected);
		fireEvent.contextMenu(screen.getByRole('toolbar'));
		const menu = screen.getByRole('menu', { name: t('actionBar.menu.label') });
		expect(
			within(menu)
				.getAllByRole('menuitem')
				.map((item) => item.textContent),
		).toEqual([t('actionBar.hideLabels'), t('actionBar.hide')]);
		await userEvent.click(within(menu).getByRole('menuitem', { name: t('actionBar.hideLabels') }));
		expect(actions.setActionBarLabels).toHaveBeenCalledWith(false);

		fireEvent.contextMenu(screen.getByRole('toolbar'));
		await userEvent.click(screen.getByRole('menuitem', { name: t('actionBar.hide') }));
		expect(actions.setActionBar).toHaveBeenCalledWith(false);
	});

	it('offers Show Labels while they are hidden', async () => {
		const { actions } = setup(selected, { actionBarLabels: false });
		fireEvent.contextMenu(screen.getByRole('toolbar'));
		await userEvent.click(screen.getByRole('menuitem', { name: t('actionBar.showLabels') }));
		expect(actions.setActionBarLabels).toHaveBeenCalledWith(true);
	});
});

describe('overflow', () => {
	/** Gives every button cell the same width and the bar a fixed width, as a browser's layout would. */
	function lay(widthOfBar: number, cell = 100) {
		Object.defineProperty(HTMLElement.prototype, 'offsetWidth', {
			configurable: true,
			get(this: HTMLElement) {
				return this.className.includes('cell') ? cell : 0;
			},
		});
		Object.defineProperty(HTMLElement.prototype, 'clientWidth', {
			configurable: true,
			get(this: HTMLElement) {
				return this.getAttribute('role') === 'toolbar' ? widthOfBar : 0;
			},
		});
	}

	it('shows everything when it fits', () => {
		lay(2000);
		const { container } = setup(selected);
		expect(container.querySelector('[data-action="more"]')).toBeNull();
		expect(container.querySelectorAll('[data-overflow]')).toHaveLength(0);
	});

	it('collapses the trailing buttons into a More menu when it does not', async () => {
		lay(350);
		const { container, button, fake } = setup({
			...selected,
			undo: entry(2, 'x'),
		});
		const hidden = [...container.querySelectorAll('[data-overflow] [data-action]')].map((item) =>
			item.getAttribute('data-action'),
		);
		// 36 for More, then 100 each: three buttons (New, Cut, Copy) fit, the rest are out of the bar.
		expect(hidden).toEqual(['paste', 'rename', 'moveToTrash', 'sort', 'view', 'undo', 'redo']);
		expect(button('more')).toBeInTheDocument();
		expect(button('more')).toHaveAttribute('aria-haspopup', 'menu');

		await userEvent.click(button('more'));
		const menu = screen.getByRole('menu', { name: t('actionBar.label') });
		const rows = within(menu)
			.getAllByRole('menuitem')
			.map((row) => row.textContent);
		expect(rows).toEqual([
			expect.stringContaining('Paste'),
			expect.stringContaining('Rename'),
			expect.stringContaining('Delete'),
			expect.stringContaining('Sort'),
			expect.stringContaining('View'),
			expect.stringContaining('Undo'),
			expect.stringContaining('Redo'),
		]);
		await userEvent.click(within(menu).getByRole('menuitem', { name: /Undo/ }));
		expect(fake.undo).toHaveBeenCalledTimes(1);
	});

	it('puts More last in the focus order and leaves the hidden buttons out of it', () => {
		lay(350);
		const { button } = setup(selected);
		button('copy').focus();
		fireEvent.keyDown(button('copy'), { key: 'ArrowRight' });
		expect(button('more')).toHaveFocus();
		fireEvent.keyDown(button('more'), { key: 'ArrowRight' });
		expect(button('more')).toHaveFocus();
		fireEvent.keyDown(button('more'), { key: 'Home' });
		expect(button('new')).toHaveFocus();
	});

	it('shows disabled buttons in More as disabled', async () => {
		lay(350);
		const { button } = setup({});
		await userEvent.click(button('more'));
		const undo = screen.getByRole('menuitem', { name: /Undo/ });
		expect(undo).toHaveAttribute('aria-disabled', 'true');
	});
});

function readCss(): string {
	return readFileSync(join(import.meta.dirname, 'ActionBar.module.css'), 'utf8');
}
