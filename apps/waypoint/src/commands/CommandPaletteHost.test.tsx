// Tests for the palette's host: the key and the menu open it, a chosen command runs through the registry, and an older history entry runs its chain
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { dismissNotice, useNotice } from '../app/notices';
import { t } from '../i18n/messages';
import type { HistoryStepOutcome } from '../ops/fileCommands';
import { entry, factsFor } from '../test/commandFacts';
import { CommandBridgeProvider, createCommandBridge } from './commandBridge';
import { idleActions } from './commandEnv';
import { CommandPaletteHost, isPaletteKey } from './CommandPaletteHost';
import { keyEventInit } from './shortcuts';

afterEach(() => {
	cleanup();
	dismissNotice();
});

function Notice() {
	return <p data-testid="notice">{useNotice()?.text ?? ''}</p>;
}

const history = [
	entry(5, 'Move 3 items to Trash'),
	entry(4, 'New folder'),
	entry(3, 'Rename report'),
];

function setup(options: { step?: (kind: string, id: number) => Promise<HistoryStepOutcome> } = {}) {
	const step = vi.fn(options.step ?? (async () => ({ ok: true }) as HistoryStepOutcome));
	const actions = {
		...idleActions(),
		newTab: vi.fn(),
		setViewMode: vi.fn(),
		openSettings: vi.fn(),
		files: {
			newFolder: vi.fn().mockResolvedValue(undefined),
			stepHistory: step,
		} as never,
	};
	const bridge = createCommandBridge({
		facts: factsFor(
			{ selected: 0, undo: history[0] },
			{ history, undoHead: 5, redoHead: null, undoLabel: history[0]!.label },
		),
		actions,
	});
	render(
		<CommandBridgeProvider value={bridge}>
			<button>Pane</button>
			<CommandPaletteHost />
			<Notice />
		</CommandBridgeProvider>,
	);
	const pane = screen.getByRole('button', { name: 'Pane' });
	pane.focus();
	return { actions, bridge, step, pane };
}

const open = () => fireEvent.keyDown(window, keyEventInit('Ctrl+Shift+P'));

describe('opening', () => {
	it('opens on Ctrl+Shift+P and again closes on it', async () => {
		setup();
		expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
		open();
		expect(await screen.findByRole('dialog', { name: t('palette.title') })).toBeInTheDocument();
		open();
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});

	it('opens from the key even while a text field has focus', async () => {
		setup();
		const field = document.createElement('input');
		document.body.append(field);
		field.focus();
		fireEvent.keyDown(field, keyEventInit('Ctrl+Shift+P'));
		expect(await screen.findByRole('combobox')).toBeInTheDocument();
		field.remove();
	});

	it('opens from the menu’s action, with a query typed in', async () => {
		const { bridge } = setup();
		act(() => bridge.store.getState().actions.openPalette('undo'));
		expect(await screen.findByRole('combobox')).toHaveValue('undo');
	});

	it('recognises only Ctrl+Shift+P', () => {
		const press = (init: Partial<KeyboardEvent>) =>
			isPaletteKey({
				key: 'P',
				ctrlKey: true,
				metaKey: false,
				altKey: false,
				shiftKey: true,
				...init,
			});
		expect(press({})).toBe(true);
		expect(press({ key: 'p' })).toBe(true);
		expect(press({ shiftKey: false })).toBe(false);
		expect(press({ ctrlKey: false })).toBe(false);
		expect(press({ altKey: true })).toBe(false);
		expect(press({ key: 'O' })).toBe(false);
		expect(press({ isComposing: true })).toBe(false);
	});

	it('does not stack over another open dialog', () => {
		setup();
		const other = document.createElement('dialog');
		other.setAttribute('open', '');
		document.body.append(other);
		open();
		expect(screen.queryByRole('combobox')).not.toBeInTheDocument();
		other.remove();
	});

	it('returns focus to the pane on Escape', async () => {
		const { pane } = setup();
		open();
		await screen.findByRole('combobox');
		await userEvent.keyboard('{Escape}');
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
		expect(pane).toHaveFocus();
	});
});

describe('running a command', () => {
	it('runs it through the registry, closes, and has focus back on the pane first', async () => {
		const { actions, pane } = setup();
		open();
		await userEvent.type(await screen.findByRole('combobox'), 'new tab{Enter}');
		expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
		expect(pane).toHaveFocus();
		await waitFor(() => expect(actions.newTab).toHaveBeenCalledTimes(1));
	});

	it('runs the command the menu runs: Settings and a view switch', async () => {
		const { actions } = setup();
		open();
		await userEvent.type(await screen.findByRole('combobox'), 'settings{Enter}');
		await waitFor(() => expect(actions.openSettings).toHaveBeenCalledTimes(1));
		open();
		await userEvent.type(await screen.findByRole('combobox'), 'grid{Enter}');
		await waitFor(() => expect(actions.setViewMode).toHaveBeenCalledWith('grid'));
	});

	it('puts the commands run last first the next time it opens', async () => {
		setup();
		open();
		await userEvent.type(await screen.findByRole('combobox'), 'new tab{Enter}');
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
		open();
		await screen.findByRole('combobox');
		expect(screen.getAllByRole('option')[0]).toHaveAccessibleName(/^New Tab/);
		expect(
			within(screen.getAllByRole('option')[0]!).getByText(t('palette.group.recent')),
		).toBeVisible();
	});

	it('remembers a command once however often it runs, newest first', async () => {
		setup();
		for (const query of ['new tab', 'grid', 'new tab']) {
			open();
			await userEvent.type(await screen.findByRole('combobox'), `${query}{Enter}`);
			await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
		}
		open();
		await screen.findByRole('combobox');
		const names = screen.getAllByRole('option').map((option) => option.getAttribute('aria-label'));
		expect(names[0]).toMatch(/^New Tab/);
		expect(names[1]).toMatch(/^Grid/);
		expect(names.filter((name) => name?.startsWith('New Tab'))).toHaveLength(1);
	});

	it('leaves a command that needs a selection in the list, greyed, and keeps the palette open on Enter', async () => {
		setup();
		open();
		await userEvent.type(await screen.findByRole('combobox'), 'duplicate{Enter}');
		expect(screen.getByRole('dialog')).toBeInTheDocument();
		expect(screen.getByRole('status')).toHaveTextContent(t('cmd.reason.nothingSelected'));
	});
});

describe('the undo history', () => {
	async function chooseOlder() {
		const view = setup();
		open();
		await userEvent.type(await screen.findByRole('combobox'), 'undo rename');
		await userEvent.keyboard('{Enter}');
		return view;
	}

	it('asks before undoing more than one change, lists them in order, and starts on Cancel', async () => {
		const { step } = await chooseOlder();
		const dialog = await screen.findByRole('dialog', { name: 'Undo 3 changes?' });
		expect(
			within(dialog)
				.getAllByRole('listitem')
				.map((item) => item.textContent),
		).toEqual(['Move 3 items to Trash', 'New folder', 'Rename report']);
		await waitFor(() =>
			expect(within(dialog).getByRole('button', { name: t('files.cancel') })).toHaveFocus(),
		);
		expect(step).not.toHaveBeenCalled();
	});

	it('does nothing when the question is cancelled', async () => {
		const { step } = await chooseOlder();
		const dialog = await screen.findByRole('dialog', { name: 'Undo 3 changes?' });
		await userEvent.click(within(dialog).getByRole('button', { name: t('files.cancel') }));
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
		expect(step).not.toHaveBeenCalled();
		expect(screen.getByTestId('notice')).toHaveTextContent('');
	});

	it('undoes the chain newest first, one job per entry, and says so', async () => {
		const { step } = await chooseOlder();
		const dialog = await screen.findByRole('dialog', { name: 'Undo 3 changes?' });
		await userEvent.click(
			within(dialog).getByRole('button', { name: t('history.confirm.undo.confirm') }),
		);
		await waitFor(() =>
			expect(screen.getByTestId('notice')).toHaveTextContent(
				'Undid 3 changes, back to: Rename report',
			),
		);
		expect(step.mock.calls).toEqual([
			['undo', 5],
			['undo', 4],
			['undo', 3],
		]);
	});

	it('stops at the first refusal and reports what was undone and why it stopped', async () => {
		const { step } = setup({
			step: async (_kind, id) =>
				id === 4 ? { ok: false, reason: 'New folder has been changed since' } : { ok: true },
		});
		open();
		await userEvent.type(await screen.findByRole('combobox'), 'undo rename{Enter}');
		const dialog = await screen.findByRole('dialog', { name: 'Undo 3 changes?' });
		await userEvent.click(
			within(dialog).getByRole('button', { name: t('history.confirm.undo.confirm') }),
		);
		await waitFor(() =>
			expect(screen.getByTestId('notice')).toHaveTextContent(
				'Undid 1 of 3 changes. Stopped at New folder: New folder has been changed since',
			),
		);
		expect(step.mock.calls.map(([, id]) => id)).toEqual([5, 4]);
	});

	it('asks to finish an undo that stopped part way', async () => {
		const stalled = [entry(2, 'Newest'), entry(1, 'Stopped', { partlyUndone: true })];
		const actions = {
			...idleActions(),
			files: { stepHistory: vi.fn(async () => ({ ok: true })) } as never,
		};
		const bridge = createCommandBridge({
			facts: factsFor({}, { history: stalled, undoHead: 2, redoHead: null }),
			actions,
		});
		render(
			<CommandBridgeProvider value={bridge}>
				<CommandPaletteHost />
			</CommandBridgeProvider>,
		);
		open();
		await userEvent.type(await screen.findByRole('combobox'), 'undo stopped{Enter}');
		expect(await screen.findByRole('dialog', { name: 'Undo 2 changes?' })).toBeInTheDocument();
		expect(screen.getByText(t('history.confirm.partly.note'))).toBeInTheDocument();
	});

	it('redoes the mirror: oldest first', async () => {
		const redoable = [
			entry(7, 'Newest', { undoable: false, redoable: true }),
			entry(6, 'Middle', { undoable: false, redoable: true }),
			entry(5, 'Oldest', { undoable: false, redoable: true }),
		];
		const step = vi.fn(async () => ({ ok: true }) as HistoryStepOutcome);
		const bridge = createCommandBridge({
			facts: factsFor({}, { history: redoable, undoHead: null, redoHead: 5 }),
			actions: { ...idleActions(), files: { stepHistory: step } as never },
		});
		render(
			<CommandBridgeProvider value={bridge}>
				<CommandPaletteHost />
				<Notice />
			</CommandBridgeProvider>,
		);
		open();
		await userEvent.type(await screen.findByRole('combobox'), 'redo newest{Enter}');
		const dialog = await screen.findByRole('dialog', { name: 'Redo 3 changes?' });
		await userEvent.click(
			within(dialog).getByRole('button', { name: t('history.confirm.redo.confirm') }),
		);
		await waitFor(() => expect(screen.getByTestId('notice')).toHaveTextContent('Redid 3 changes'));
		expect(step.mock.calls).toEqual([
			['redo', 5],
			['redo', 6],
			['redo', 7],
		]);
	});
});
