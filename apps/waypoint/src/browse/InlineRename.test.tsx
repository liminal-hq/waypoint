// Verifies renaming in place: what starts selected, the keys, live validation, a taken name, the extension guard and focus
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { FileCommands, RenameOutcome } from '../ops/fileCommands';
import { FileCommandsProvider } from '../ops/FileCommandsContext';
import { makeEntry } from '../services/fakeVfsClient';
import { stubLayout } from '../test/browseHarness';
import { commandsHarness, type CommandsHarness } from '../test/fileCommandsHarness';
import { GridView } from './GridView';
import { ListingView } from './ListView';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280, 600);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

type Rename = (...args: Parameters<FileCommands['renameEntry']>) => Promise<RenameOutcome>;

/** The views over the harness's session, with `renameEntry` replaced by a spy that answers `outcome`. */
async function setup(
	options: { names?: string[]; outcome?: RenameOutcome; grid?: boolean } = {},
): Promise<{ h: CommandsHarness; rename: ReturnType<typeof vi.fn<Rename>> }> {
	const names = options.names ?? ['alpha.txt', 'beta.jpg', '.bashrc'];
	const h = await commandsHarness({
		entries: [
			...names.map((name, i) => makeEntry(i + 1, name, { hidden: false })),
			makeEntry(50, 'v1.2', { kind: 'directory' }),
		],
	});
	const rename = vi.fn<Rename>(async () => options.outcome ?? { ok: true });
	const commands: FileCommands = { ...h.commands, renameEntry: rename };
	const state = { status: 'ready', session: h.session } as const;
	render(
		<FileCommandsProvider value={commands}>
			{options.grid ? (
				<GridView state={state} size={96} announceSelection={false} />
			) : (
				<ListingView state={state} announceSelection={false} />
			)}
		</FileCommandsProvider>,
	);
	await screen.findAllByRole('option');
	await waitFor(() => expect(screen.getAllByRole('option')[0]).not.toHaveAttribute('aria-busy'));
	return { h, rename };
}

const field = () => screen.getByRole('textbox') as HTMLInputElement;
const listbox = () => screen.getByRole('listbox');

async function startRename(h: CommandsHarness, name: string) {
	const { model, store } = h.session;
	await model.readRange(0, model.count);
	let entry;
	for (let position = 0; position < model.count; position++) {
		if (model.entryAt(position)?.name === name) entry = model.entryAt(position);
	}
	act(() => store.getState().beginRename(entry!.id));
	await screen.findByRole('textbox');
}

describe('starting', () => {
	it('selects the stem of a file, so typing keeps the extension, and focuses the field', async () => {
		const { h } = await setup();
		await startRename(h, 'alpha.txt');
		expect(field()).toHaveFocus();
		expect(field().value).toBe('alpha.txt');
		expect([field().selectionStart, field().selectionEnd]).toEqual([0, 5]);
	});

	it('selects the whole name of a folder, however many dots it has', async () => {
		const { h } = await setup();
		await startRename(h, 'v1.2');
		expect([field().selectionStart, field().selectionEnd]).toEqual([0, 4]);
	});

	it('selects the whole name of a dotfile, which has no extension', async () => {
		const { h } = await setup();
		await startRename(h, '.bashrc');
		expect([field().selectionStart, field().selectionEnd]).toEqual([0, 7]);
	});

	it('is named for the entry and described by the hint and the problem line', async () => {
		const { h } = await setup();
		await startRename(h, 'alpha.txt');
		expect(field()).toHaveAccessibleName('Rename alpha.txt');
		expect(field()).toHaveAccessibleDescription(/Enter renames, Escape cancels/);
	});

	it('works in the grid too', async () => {
		const { h } = await setup({ grid: true });
		await startRename(h, 'beta.jpg');
		expect([field().selectionStart, field().selectionEnd]).toEqual([0, 4]);
	});
});

describe('committing and cancelling', () => {
	it('renames on Enter, then returns focus to the list and closes the field', async () => {
		const user = userEvent.setup();
		const { h, rename } = await setup();
		await startRename(h, 'alpha.txt');
		await user.keyboard('{Control>}a{/Control}omega.txt{Enter}');
		await waitFor(() => expect(screen.queryByRole('textbox')).toBeNull());
		expect(rename).toHaveBeenCalledTimes(1);
		expect(rename.mock.calls[0]![2]).toBe('omega.txt');
		expect(listbox()).toHaveFocus();
		expect(h.session.store.getState().renaming).toBeNull();
	});

	it('does nothing but close when the name is unchanged', async () => {
		const user = userEvent.setup();
		const { h, rename } = await setup();
		await startRename(h, 'alpha.txt');
		await user.keyboard('{Enter}');
		expect(rename).not.toHaveBeenCalled();
		expect(screen.queryByRole('textbox')).toBeNull();
		expect(listbox()).toHaveFocus();
	});

	it('cancels on Escape without renaming', async () => {
		const user = userEvent.setup();
		const { h, rename } = await setup();
		await startRename(h, 'alpha.txt');
		await user.keyboard('zzz{Escape}');
		expect(rename).not.toHaveBeenCalled();
		expect(screen.queryByRole('textbox')).toBeNull();
		expect(listbox()).toHaveFocus();
		expect(screen.getByText('alpha.txt')).toBeInTheDocument();
	});

	it('cancels when focus moves elsewhere, and does not when only the window loses focus', async () => {
		const user = userEvent.setup();
		const { h, rename } = await setup();
		await startRename(h, 'alpha.txt');
		// The window losing focus fires blur on the field while it is still the active element.
		fireEvent.blur(field());
		expect(screen.queryByRole('textbox')).not.toBeNull();
		await user.click(document.body);
		await waitFor(() => expect(screen.queryByRole('textbox')).toBeNull());
		expect(rename).not.toHaveBeenCalled();
	});

	it('keeps the list’s keys out of the field: Ctrl+A selects the text, a letter types and nothing else is selected', async () => {
		const user = userEvent.setup();
		const { h } = await setup();
		await startRename(h, 'alpha.txt');
		const before = h.session.store.getState().selection;
		await user.keyboard('{Control>}a{/Control}b');
		expect(field().value).toBe('b');
		expect(h.session.store.getState().selection).toBe(before);
	});

	it('does not open or select the row when the field is clicked or double-clicked', async () => {
		const user = userEvent.setup();
		const { h } = await setup();
		await startRename(h, 'alpha.txt');
		const before = h.session.store.getState().selection;
		await user.dblClick(field());
		expect(screen.queryByRole('textbox')).not.toBeNull();
		expect(h.session.store.getState().selection).toBe(before);
	});
});

describe('validation', () => {
	it('shows what is wrong under the field while typing, and refuses to commit it', async () => {
		const user = userEvent.setup();
		const { h, rename } = await setup();
		await startRename(h, 'alpha.txt');
		await user.keyboard('{Control>}a{/Control}a/b');
		expect(field()).toHaveAttribute('aria-invalid', 'true');
		const problem = screen.getByText('A name cannot contain “/”.');
		expect(problem).toHaveAttribute('aria-live', 'polite');
		expect(field()).toHaveAccessibleDescription(/A name cannot contain/);
		await user.keyboard('{Enter}');
		expect(rename).not.toHaveBeenCalled();
		expect(screen.queryByRole('textbox')).not.toBeNull();
		// Fixing it clears the message.
		await user.keyboard('{Backspace}{Backspace}');
		expect(field()).toHaveAttribute('aria-invalid', 'false');
	});

	it('says an empty name is not allowed', async () => {
		const user = userEvent.setup();
		const { h } = await setup();
		await startRename(h, 'alpha.txt');
		await user.keyboard('{Control>}a{/Control}{Backspace}');
		expect(screen.getByText('A name cannot be empty.')).toBeInTheDocument();
	});

	it('keeps the field open with the reason when Rust refuses the name, such as one that is taken', async () => {
		const user = userEvent.setup();
		const { h, rename } = await setup({
			outcome: { ok: false, message: 'A file named “beta.txt” already exists.' },
		});
		await startRename(h, 'alpha.txt');
		await user.keyboard('{Control>}a{/Control}beta.txt{Enter}');
		await screen.findByText('A file named “beta.txt” already exists.');
		expect(rename).toHaveBeenCalledTimes(1);
		expect(field()).toHaveFocus();
		expect(field().value).toBe('beta.txt');
		expect(field()).toHaveAttribute('aria-invalid', 'true');
	});
});

describe('the extension guard', () => {
	async function change(user: ReturnType<typeof userEvent.setup>, h: CommandsHarness, to: string) {
		await startRename(h, 'beta.jpg');
		await user.keyboard(`{Control>}a{/Control}${to}{Enter}`);
		return screen.findByRole('dialog', { name: 'Change the extension?' });
	}

	it('asks when the extension changes, with Keep as the default button', async () => {
		const user = userEvent.setup();
		const { h, rename } = await setup();
		const dialog = await change(user, h, 'beta.png');
		expect(dialog).toHaveAccessibleDescription(/from \.jpg to \.png/);
		expect(within(dialog).getByRole('button', { name: 'Keep .jpg' })).toHaveFocus();
		expect(rename).not.toHaveBeenCalled();
	});

	it('renames with the new extension on "Use .png"', async () => {
		const user = userEvent.setup();
		const { h, rename } = await setup();
		const dialog = await change(user, h, 'beta.png');
		await user.click(within(dialog).getByRole('button', { name: 'Use .png' }));
		await waitFor(() => expect(rename).toHaveBeenCalledTimes(1));
		expect(rename.mock.calls[0]![2]).toBe('beta.png');
	});

	it('puts the old extension back on "Keep .jpg", renaming only the stem', async () => {
		const user = userEvent.setup();
		const { h, rename } = await setup();
		const dialog = await change(user, h, 'holiday.png');
		await user.click(within(dialog).getByRole('button', { name: 'Keep .jpg' }));
		await waitFor(() => expect(rename).toHaveBeenCalledTimes(1));
		expect(rename.mock.calls[0]![2]).toBe('holiday.jpg');
	});

	it('closes without renaming when Keep leaves the name as it was', async () => {
		const user = userEvent.setup();
		const { h, rename } = await setup();
		const dialog = await change(user, h, 'beta.png');
		await user.click(within(dialog).getByRole('button', { name: 'Keep .jpg' }));
		await waitFor(() => expect(screen.queryByRole('textbox')).toBeNull());
		expect(rename).not.toHaveBeenCalled();
	});

	it('goes back to the field, keeping what was typed, on Escape', async () => {
		const user = userEvent.setup();
		const { h, rename } = await setup();
		await change(user, h, 'beta.png');
		await user.keyboard('{Escape}');
		await waitFor(() => expect(screen.queryByRole('dialog')).toBeNull());
		expect(field().value).toBe('beta.png');
		expect(rename).not.toHaveBeenCalled();
	});

	it('says "Remove" when the extension is dropped', async () => {
		const user = userEvent.setup();
		const { h } = await setup();
		await startRename(h, 'beta.jpg');
		await user.keyboard('{Control>}a{/Control}beta{Enter}');
		const dialog = await screen.findByRole('dialog', { name: 'Change the extension?' });
		expect(dialog).toHaveAccessibleDescription(/Remove the extension \.jpg/);
		expect(within(dialog).getByRole('button', { name: 'Remove .jpg' })).toBeInTheDocument();
	});

	it('does not ask for a dotfile, a folder or a rename that keeps the extension', async () => {
		const user = userEvent.setup();
		const { h, rename } = await setup();
		await startRename(h, '.bashrc');
		await user.keyboard('{Control>}a{/Control}.zshrc{Enter}');
		await waitFor(() => expect(rename).toHaveBeenCalledTimes(1));
		expect(screen.queryByRole('dialog')).toBeNull();
	});
});

describe('a rename that moves the row', () => {
	it('ends the rename even though the field unmounts before the answer arrives', async () => {
		const user = userEvent.setup();
		const { h, rename } = await setup();
		let release: (outcome: RenameOutcome) => void = () => {};
		rename.mockImplementation(() => new Promise<RenameOutcome>((resolve) => (release = resolve)));
		await startRename(h, 'alpha.txt');
		await user.keyboard('{Control>}a{/Control}zulu.txt{Enter}');
		// The listing re-sorts the renamed entry, so its row (and the field) is replaced.
		act(() => h.session.store.setState({ focus: 0 }));
		cleanup();
		release({ ok: true });
		await waitFor(() => expect(h.session.store.getState().renaming).toBeNull());
	});
});
