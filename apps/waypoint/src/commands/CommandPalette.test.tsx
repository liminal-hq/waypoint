// Tests for the palette component: its combobox and listbox pattern, keyboard model, announcements, focus and motion
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { t } from '../i18n/messages';
import { entry, factsFor } from '../test/commandFacts';
import { CommandPalette, type CommandPaletteProps } from './CommandPalette';
import { historyRows } from './historyCommands';
import { evaluateCommands } from './registry';

afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

function setup(props: Partial<CommandPaletteProps> = {}, state = {}) {
	const onChoose = vi.fn();
	const onClose = vi.fn();
	const commands = evaluateCommands(factsFor({ selected: 1, ...state }));
	render(
		<CommandPalette
			commands={commands}
			history={[]}
			recents={[]}
			onChoose={onChoose}
			onClose={onClose}
			{...props}
		/>,
	);
	return {
		onChoose,
		onClose,
		input: screen.getByRole('combobox'),
		list: screen.getByRole('listbox'),
	};
}

const active = (input: HTMLElement) =>
	document.getElementById(input.getAttribute('aria-activedescendant')!);
const labelOf = (option: HTMLElement | null) => option?.getAttribute('aria-label') ?? '';

describe('the combobox and listbox pattern', () => {
	it('is a modal dialog with a labelled combobox that controls a listbox of options', () => {
		const { input, list } = setup();
		const dialog = screen.getByRole('dialog', { name: t('palette.title') });
		expect(dialog).toContainElement(input);
		expect(input).toHaveAttribute('aria-expanded', 'true');
		expect(input).toHaveAttribute('aria-controls', list.id);
		expect(input).toHaveAttribute('aria-autocomplete', 'list');
		expect(input).toHaveAccessibleName(t('palette.placeholder'));
		expect(input).toHaveAttribute('placeholder', t('palette.placeholder'));
		expect(list).toHaveAccessibleName(t('palette.list.label'));
		const options = within(list).getAllByRole('option');
		expect(options.length).toBeGreaterThan(10);
		for (const option of options) expect(option).toHaveAttribute('id');
	});

	it('focuses the input and makes the first option the active one', () => {
		const { input } = setup();
		expect(input).toHaveFocus();
		const first = within(screen.getByRole('listbox')).getAllByRole('option')[0]!;
		expect(input).toHaveAttribute('aria-activedescendant', first.id);
		expect(first).toHaveAttribute('aria-selected', 'true');
		const others = within(screen.getByRole('listbox')).getAllByRole('option').slice(1);
		for (const option of others) expect(option).toHaveAttribute('aria-selected', 'false');
	});

	it('names an option for a screen reader with its group and key', () => {
		setup();
		expect(
			screen.getByRole('option', {
				name: `${t('menu.moveToTrash')}, ${t('palette.group.file')}, shortcut Delete`,
			}),
		).toBeInTheDocument();
	});

	it('shows each shortcut on the right and the group as a muted hint', () => {
		setup();
		const option = screen.getByRole('option', { name: /^New Folder, File/ });
		expect(within(option).getByText('F7')).toBeInTheDocument();
		expect(within(option).getByText(t('palette.group.file'))).toBeInTheDocument();
	});

	it('is the same for an empty list: collapsed, with nothing active', async () => {
		const { input } = setup();
		await userEvent.type(input, 'qzx');
		expect(input).toHaveAttribute('aria-expanded', 'false');
		expect(input).not.toHaveAttribute('aria-activedescendant');
		expect(screen.queryAllByRole('option')).toHaveLength(0);
		expect(screen.getByRole('status')).toHaveTextContent(t('palette.count.none'));
	});
});

describe('filtering and the live region', () => {
	it('filters as you type and announces how many rows are left', async () => {
		const { input } = setup();
		await userEvent.type(input, 'new f');
		const options = screen.getAllByRole('option');
		expect(screen.getByRole('status')).toHaveTextContent(`${options.length} commands`);
		expect(labelOf(options[0]!)).toMatch(/^New Folder/);
		expect(screen.getByRole('status')).toHaveAttribute('aria-live', 'polite');
	});

	it('says "1 command" for a single match', async () => {
		const { input } = setup();
		await userEvent.type(input, 'batch rename');
		expect(screen.getByRole('status')).toHaveTextContent('1 command');
	});

	it('draws the matched characters bold, so a match is not shown by colour alone', async () => {
		const { input } = setup();
		await userEvent.type(input, 'nf');
		const first = screen.getAllByRole('option')[0]!;
		const bold = [...first.querySelectorAll('b')].map((node) => node.textContent);
		expect(bold).toEqual(['N', 'F']);
	});

	it('starts with a prefilled query, caret at its end', () => {
		const { input } = setup({ initialQuery: 'undo' });
		expect(input).toHaveValue('undo');
		expect((input as HTMLInputElement).selectionStart).toBe(4);
	});

	it('lists the commands run last first for an empty query', () => {
		setup({ recents: ['viewGrid', 'newTab'] });
		const options = screen.getAllByRole('option');
		expect(labelOf(options[0]!)).toMatch(/^Grid/);
		expect(labelOf(options[1]!)).toMatch(/^New Tab/);
		expect(within(options[0]!).getByText(t('palette.group.recent'))).toBeInTheDocument();
	});
});

describe('the keyboard', () => {
	it('moves the active option with the arrow keys, wrapping at the ends, while focus stays in the input', async () => {
		const { input } = setup();
		const count = screen.getAllByRole('option').length;
		const first = active(input)!;
		await userEvent.keyboard('{ArrowDown}');
		expect(active(input)).toBe(screen.getAllByRole('option')[1]);
		expect(active(input)).toHaveAttribute('aria-selected', 'true');
		expect(first).toHaveAttribute('aria-selected', 'false');
		await userEvent.keyboard('{ArrowUp}{ArrowUp}');
		expect(active(input)).toBe(screen.getAllByRole('option')[count - 1]);
		await userEvent.keyboard('{ArrowDown}');
		expect(active(input)).toBe(first);
		expect(input).toHaveFocus();
	});

	it('goes to the ends with Home and End and by a page with PageUp and PageDown', async () => {
		const { input } = setup();
		const options = screen.getAllByRole('option');
		await userEvent.keyboard('{End}');
		expect(active(input)).toBe(options[options.length - 1]);
		await userEvent.keyboard('{Home}');
		expect(active(input)).toBe(options[0]);
		await userEvent.keyboard('{PageDown}');
		expect(active(input)).toBe(options[8]);
		await userEvent.keyboard('{PageUp}');
		expect(active(input)).toBe(options[0]);
		await userEvent.keyboard('{PageUp}');
		expect(active(input)).toBe(options[0]);
	});

	it('keeps typing in the input whatever is active, and the first match active after each change', async () => {
		const { input } = setup();
		await userEvent.keyboard('{ArrowDown}{ArrowDown}');
		await userEvent.keyboard('fold');
		expect(input).toHaveValue('fold');
		expect(active(input)).toBe(screen.getAllByRole('option')[0]);
	});

	it('runs the active command on Enter', async () => {
		const { input, onChoose } = setup();
		await userEvent.type(input, 'new folder{Enter}');
		expect(onChoose).toHaveBeenCalledWith({ kind: 'command', id: 'newFolder' });
	});

	it('closes on Escape', async () => {
		const { onClose, onChoose } = setup();
		await userEvent.keyboard('{Escape}');
		expect(onClose).toHaveBeenCalledTimes(1);
		expect(onChoose).not.toHaveBeenCalled();
	});

	it('closes on the platform’s own cancel too, without letting it close behind React', () => {
		const { onClose } = setup();
		const dialog = screen.getByRole('dialog');
		const cancel = new Event('cancel', { cancelable: true });
		act(() => {
			dialog.dispatchEvent(cancel);
		});
		expect(cancel.defaultPrevented).toBe(true);
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('keeps Tab inside: it moves nothing', async () => {
		const { input } = setup();
		await userEvent.tab();
		expect(input).toHaveFocus();
	});

	it('does nothing on Enter with no row', async () => {
		const { input, onChoose } = setup();
		await userEvent.type(input, 'qzx{Enter}');
		expect(onChoose).not.toHaveBeenCalled();
	});
});

describe('a command that cannot run', () => {
	it('is shown greyed with its reason, and Enter announces the reason and does nothing', async () => {
		const { input, onChoose, onClose } = setup({}, { selected: 0 });
		await userEvent.type(input, 'move to trash');
		const option = screen.getAllByRole('option')[0]!;
		expect(option).toHaveAttribute('aria-disabled', 'true');
		expect(within(option).getByText(t('cmd.reason.nothingSelected'))).toBeInTheDocument();
		expect(option).toHaveAccessibleName(
			expect.stringContaining(`unavailable: ${t('cmd.reason.nothingSelected')}`),
		);
		await userEvent.keyboard('{Enter}');
		expect(onChoose).not.toHaveBeenCalled();
		expect(onClose).not.toHaveBeenCalled();
		expect(screen.getByRole('status')).toHaveTextContent(
			`${t('menu.moveToTrash')} is unavailable: ${t('cmd.reason.nothingSelected')}`,
		);
	});

	it('does nothing when clicked, either, and the announcement gives way to the count on the next keystroke', async () => {
		const { input, onChoose } = setup({}, { selected: 0 });
		await userEvent.type(input, 'duplicate');
		await userEvent.click(screen.getAllByRole('option')[0]!);
		expect(onChoose).not.toHaveBeenCalled();
		expect(screen.getByRole('status')).toHaveTextContent('unavailable');
		await userEvent.type(input, 'x');
		expect(screen.getByRole('status')).not.toHaveTextContent('unavailable');
	});
});

describe('the pointer', () => {
	it('runs a row on click without taking focus from the input', async () => {
		const { input, onChoose } = setup();
		await userEvent.click(screen.getByRole('option', { name: /^New Tab/ }));
		expect(onChoose).toHaveBeenCalledWith({ kind: 'command', id: 'newTab' });
		expect(input).toHaveFocus();
	});

	it('makes the row under the pointer the active one', () => {
		const { input } = setup();
		const option = screen.getAllByRole('option')[3]!;
		fireEvent.mouseMove(option);
		expect(input).toHaveAttribute('aria-activedescendant', option.id);
		expect(option).toHaveAttribute('aria-selected', 'true');
	});

	it('closes on a click on the backdrop but not inside the panel', () => {
		const { onClose } = setup();
		fireEvent.click(screen.getByRole('combobox'));
		expect(onClose).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole('dialog'));
		expect(onClose).toHaveBeenCalledTimes(1);
	});
});

describe('history rows', () => {
	const history = [entry(3, 'Move 3 items to Trash'), entry(2, 'New folder'), entry(1, 'Rename')];
	const rows = historyRows(history, { undoHead: 3, redoHead: null });

	it('lists every older entry for "undo" and chooses it as a history target', async () => {
		const { input, onChoose } = setup(
			{ history: rows, initialQuery: 'undo' },
			{ undo: history[0] },
		);
		const labels = screen.getAllByRole('option').map((option) => labelOf(option));
		expect(labels[0]).toMatch(/^Undo Move 3 items to Trash/);
		expect(labels.some((label) => label.startsWith('Undo 2 changes back to: New folder, '))).toBe(
			true,
		);
		await userEvent.click(screen.getByRole('option', { name: /Undo 3 changes back to: Rename/ }));
		expect(onChoose).toHaveBeenCalledWith({
			kind: 'history',
			row: expect.objectContaining({ key: 'history:undo:1' }),
		});
		expect(input).toHaveFocus();
	});
});

describe('focus and the window', () => {
	function Harness() {
		const [open, setOpen] = useState(true);
		return (
			<>
				<button onClick={() => setOpen(true)}>Open</button>
				{open && (
					<CommandPalette
						commands={evaluateCommands(factsFor())}
						history={[]}
						recents={[]}
						onChoose={() => setOpen(false)}
						onClose={() => setOpen(false)}
					/>
				)}
			</>
		);
	}

	it('gives focus back to where it was when it closes', async () => {
		const outer = render(<button>Before</button>);
		const target = outer.getByRole('button', { name: 'Before' });
		target.focus();
		const inner = render(<Harness />);
		expect(screen.getByRole('combobox')).toHaveFocus();
		await userEvent.keyboard('{Escape}');
		expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
		expect(target).toHaveFocus();
		inner.unmount();
	});

	it('closes when the window loses focus', () => {
		const { onClose } = setup();
		act(() => {
			window.dispatchEvent(new FocusEvent('blur'));
		});
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('opens a native modal dialog', () => {
		const show = vi.spyOn(HTMLDialogElement.prototype, 'showModal');
		setup();
		expect(show).toHaveBeenCalledTimes(1);
		show.mockRestore();
	});
});

describe('motion', () => {
	it('notes a reduced-motion preference so the stylesheet and the test can see it', () => {
		vi.stubGlobal('matchMedia', (query: string) => ({
			matches: query.includes('prefers-reduced-motion'),
			media: query,
			addEventListener: () => {},
			removeEventListener: () => {},
		}));
		setup();
		expect(screen.getByRole('dialog')).toHaveAttribute('data-reduced-motion');
	});

	it('animates otherwise', () => {
		setup();
		expect(screen.getByRole('dialog')).not.toHaveAttribute('data-reduced-motion');
	});
});
