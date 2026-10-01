// Verifies the conflict dialog: nothing pre-selected, what is legal for each clash, bulk answers, the row cap, formatting and the keys
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Conflict } from '@liminal-hq/waypoint-protocol/generated/Conflict';
import type { JobSnapshot } from '@liminal-hq/waypoint-protocol/generated/JobSnapshot';
import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TimeFormatProvider } from '../browse/TimeFormatContext';
import { FakeTimeFormatClient } from '../services/fakeTimeFormatClient';
import { fileLocation } from '../services/fakeVfsClient';
import { fakeJobSnapshot } from '../trash/fakeTrashClient';
import { conflictFor } from '../test/opsHarness';
import { ConflictDialog, type ConflictDialogProps } from './ConflictDialog';
import { ROW_CAP } from './conflictModel';

afterEach(cleanup);

function copyJob(overrides: Partial<JobSnapshot> = {}): JobSnapshot {
	return {
		...fakeJobSnapshot(1, { kind: 'copy' }, { state: 'queued' }),
		destination: fileLocation('/dest'),
		...overrides,
	};
}

function mount(
	conflicts: Conflict[],
	props: Partial<ConflictDialogProps> = {},
	hour?: 'h12' | 'h23',
) {
	const handlers = { onContinue: vi.fn(), onCancelJob: vi.fn(), onLater: vi.fn() };
	const clock = new FakeTimeFormatClient(hour);
	render(
		<TimeFormatProvider client={clock}>
			<ConflictDialog job={copyJob()} conflicts={conflicts} {...handlers} {...props} />
		</TimeFormatProvider>,
	);
	return handlers;
}

const dialog = () => screen.getByRole('dialog');
const continueButton = () => within(dialog()).getByRole('button', { name: 'Continue' });
const choiceFor = (name: string) =>
	within(dialog()).getByRole('combobox', { name: `Choice for ${name}` });
const bulk = () => within(dialog()).getByRole('combobox', { name: 'Apply to all remaining' });
const optionNames = (select: HTMLElement) =>
	within(select)
		.getAllByRole('option')
		.map((o) => o.textContent);

describe('what the dialog shows', () => {
	it('says how many items exist in the destination', () => {
		mount([conflictFor('a.txt'), conflictFor('b.txt')]);
		expect(
			screen.getByRole('dialog', { name: '2 items already exist in /dest' }),
		).toBeInTheDocument();
	});

	it('uses the singular for one', () => {
		mount([conflictFor('a.txt')]);
		expect(
			screen.getByRole('dialog', { name: '1 item already exists in /dest' }),
		).toBeInTheDocument();
	});

	it('words a restore by the original folders', () => {
		mount([conflictFor('a.txt')], {
			job: copyJob({ kind: { kind: 'restore' }, destination: null }),
		});
		expect(
			screen.getByRole('dialog', { name: '1 item already exists in its original folder' }),
		).toBeInTheDocument();
		expect(
			within(dialog()).getByRole('columnheader', { name: 'In the Trash' }),
		).toBeInTheDocument();
	});

	it('lists each clash with the existing and incoming size and date side by side', () => {
		mount([conflictFor('a.txt')], {}, 'h12');
		const row = within(dialog()).getByRole('row', { name: /a\.txt/ });
		const cells = within(row).getAllByRole('cell');
		expect(cells[0]).toHaveTextContent('File · 1 kB');
		expect(cells[0]).toHaveTextContent(/Jun 1, 2026/);
		expect(cells[1]).toHaveTextContent('File · 2 kB');
		expect(cells[1]).toHaveTextContent(/Jun 2, 2026/);
	});

	it('says in words whether the incoming entry is newer, older or the same', () => {
		mount([
			conflictFor('new.txt'),
			conflictFor('old.txt', { sourceModifiedMs: 1_000, existingModifiedMs: 9_000 }),
			conflictFor('same.txt', { sourceModifiedMs: 5_000, existingModifiedMs: 5_000 }),
		]);
		const text = (name: string) =>
			within(screen.getByRole('row', { name: new RegExp(name) })).getAllByRole('cell')[1]!;
		expect(text('new.txt')).toHaveTextContent('Newer than the existing one');
		expect(text('old.txt')).toHaveTextContent('Older than the existing one');
		expect(text('same.txt')).toHaveTextContent('Same date as the existing one');
	});

	it('formats times with the system clock: 12-hour or 24-hour', async () => {
		const { unmount } = render(<div />);
		unmount();
		mount([conflictFor('a.txt')], {}, 'h23');
		await act(async () => {});
		const row24 = within(dialog()).getByRole('row', { name: /a\.txt/ });
		expect(row24).toHaveTextContent(/14:30/);
		cleanup();
		mount([conflictFor('a.txt')], {}, 'h12');
		await act(async () => {});
		const row12 = within(dialog()).getByRole('row', { name: /a\.txt/ });
		expect(row12.textContent).toMatch(/2:30\s?(PM|p\.m\.)/i);
	});

	it('shows folders without a made-up size and says a name is shared inside the batch', () => {
		mount([
			conflictFor('docs', {
				kind: 'folderOverFolder',
				sourceSize: null,
				existingSize: null,
			}),
			conflictFor('dup.txt', { withinBatch: true, existingSize: null, existingModifiedMs: null }),
		]);
		const folder = within(screen.getByRole('row', { name: /docs/ })).getAllByRole('cell')[0]!;
		expect(folder).toHaveTextContent(/^Folder · /);
		expect(folder).not.toHaveTextContent('Size unknown');
		expect(screen.getByRole('row', { name: /dup\.txt/ })).toHaveTextContent(
			'Not there yet: another item in this job has the same name',
		);
	});
});

describe('answering', () => {
	it('starts with nothing chosen and Continue off', () => {
		mount([conflictFor('a.txt'), conflictFor('b.txt')]);
		expect(choiceFor('a.txt')).toHaveValue('');
		expect(choiceFor('b.txt')).toHaveValue('');
		expect(bulk()).toHaveValue('');
		expect(continueButton()).toBeDisabled();
		expect(within(dialog()).getByRole('status')).toHaveTextContent('0 of 2 answered');
	});

	it('offers a file over a file Replace, Skip, Keep both and Replace if newer', () => {
		mount([conflictFor('a.txt')]);
		expect(optionNames(choiceFor('a.txt'))).toEqual([
			'Choose…',
			'Replace',
			'Skip',
			'Keep both',
			'Replace if newer',
		]);
	});

	it('offers a folder over a folder Merge, Skip, Keep both and Replace', () => {
		mount([conflictFor('docs', { kind: 'folderOverFolder' })]);
		expect(optionNames(choiceFor('docs'))).toEqual([
			'Choose…',
			'Merge folders',
			'Skip',
			'Keep both',
			'Replace',
		]);
	});

	it('offers a file and a folder only Skip and Keep both, and says why', () => {
		mount([
			conflictFor('x', { kind: 'fileOverFolder' }),
			conflictFor('y', { kind: 'folderOverFile' }),
		]);
		expect(optionNames(choiceFor('x'))).toEqual(['Choose…', 'Skip', 'Keep both']);
		expect(optionNames(choiceFor('y'))).toEqual(['Choose…', 'Skip', 'Keep both']);
		expect(dialog()).toHaveTextContent(/A file has the name of an existing folder/);
		expect(dialog()).toHaveTextContent(/A folder has the name of an existing file/);
	});

	it('enables Continue once every row is answered and sends one decision per row', async () => {
		const user = userEvent.setup();
		const a = conflictFor('a.txt');
		const b = conflictFor('b.txt');
		const handlers = mount([a, b]);
		await user.selectOptions(choiceFor('a.txt'), 'Skip');
		expect(continueButton()).toBeDisabled();
		await user.selectOptions(choiceFor('b.txt'), 'Keep both');
		expect(within(dialog()).getByRole('status')).toHaveTextContent('2 of 2 answered');
		await user.click(continueButton());
		expect(handlers.onContinue).toHaveBeenCalledWith({
			decisions: [
				{ source: a.source, policy: 'skip' },
				{ source: b.source, policy: 'keepBoth' },
			],
			counts: { replace: 0, skip: 1, keepBoth: 1, mergeFolders: 0, replaceIfNewer: 0 },
		});
	});

	it('answers every row with the bulk choice', async () => {
		const user = userEvent.setup();
		const handlers = mount([conflictFor('a.txt'), conflictFor('b.txt')]);
		await user.selectOptions(bulk(), 'Skip');
		expect(continueButton()).toBeEnabled();
		expect(within(dialog()).getByText('Covers all 2 without an answer.')).toBeInTheDocument();
		expect(choiceFor('a.txt')).toHaveDisplayValue('Same as “Apply to all remaining”: Skip');
		await user.click(continueButton());
		const sent = handlers.onContinue.mock.calls[0]![0];
		expect(sent.decisions).toHaveLength(2);
		expect(sent.applyToAll).toBeUndefined();
	});

	it('lets a row override the bulk choice', async () => {
		const user = userEvent.setup();
		const a = conflictFor('a.txt');
		const handlers = mount([a, conflictFor('b.txt')]);
		await user.selectOptions(bulk(), 'Skip');
		await user.selectOptions(choiceFor('a.txt'), 'Keep both');
		await user.click(continueButton());
		expect(handlers.onContinue.mock.calls[0]![0].decisions[0]).toEqual({
			source: a.source,
			policy: 'keepBoth',
		});
	});

	it('sends the bulk choice as the job’s own when "Apply to all conflicts like this one" is on', async () => {
		const user = userEvent.setup();
		const handlers = mount([conflictFor('a.txt'), conflictFor('b.txt')]);
		const later = within(dialog()).getByRole('checkbox', {
			name: 'Apply to all conflicts like this one',
		});
		expect(later).toBeDisabled();
		await user.selectOptions(bulk(), 'Keep both');
		await user.click(later);
		await user.click(continueButton());
		expect(handlers.onContinue.mock.calls[0]![0]).toMatchObject({
			decisions: [],
			applyToAll: 'keepBoth',
		});
	});

	it('does not let a bulk choice cover a clash it cannot settle', async () => {
		const user = userEvent.setup();
		mount([conflictFor('a.txt'), conflictFor('x', { kind: 'fileOverFolder' })]);
		await user.selectOptions(bulk(), 'Replace');
		expect(within(dialog()).getByText(/Covers 1 of the 2 without an answer/)).toBeInTheDocument();
		expect(continueButton()).toBeDisabled();
		await user.selectOptions(choiceFor('x'), 'Skip');
		expect(continueButton()).toBeEnabled();
	});

	it('goes back to unanswered when a row’s answer is cleared', async () => {
		const user = userEvent.setup();
		mount([conflictFor('a.txt')]);
		await user.selectOptions(choiceFor('a.txt'), 'Skip');
		expect(continueButton()).toBeEnabled();
		await user.selectOptions(choiceFor('a.txt'), 'Choose…');
		expect(continueButton()).toBeDisabled();
	});
});

describe('danger', () => {
	it('says in words what Replace does to a file', async () => {
		const user = userEvent.setup();
		mount([conflictFor('a.txt')]);
		await user.selectOptions(choiceFor('a.txt'), 'Replace');
		expect(choiceFor('a.txt')).toHaveAttribute('data-danger');
		expect(within(dialog()).getByText('Replaces the existing file.')).toBeInTheDocument();
	});

	it('warns that Replace on a folder replaces the whole folder', async () => {
		const user = userEvent.setup();
		mount([conflictFor('docs', { kind: 'folderOverFolder' })]);
		await user.selectOptions(choiceFor('docs'), 'Replace');
		expect(within(dialog()).getByText(/Replaces the whole folder/)).toBeInTheDocument();
	});

	it('warns when the bulk choice is Replace', async () => {
		const user = userEvent.setup();
		mount([conflictFor('docs', { kind: 'folderOverFolder' })]);
		await user.selectOptions(bulk(), 'Replace');
		expect(bulk()).toHaveAttribute('data-danger');
		expect(within(dialog()).getAllByText(/Replaces the whole folder/).length).toBeGreaterThan(0);
	});

	it('never makes the Continue button destructive', () => {
		mount([conflictFor('a.txt')]);
		expect(continueButton()).toHaveAttribute('data-variant', 'primary');
	});

	it('describes Keep both as a numbered copy, or a free name when restoring', async () => {
		const user = userEvent.setup();
		mount([conflictFor('a.txt')]);
		await user.selectOptions(choiceFor('a.txt'), 'Keep both');
		expect(
			within(dialog()).getByText('Kept beside the existing one as a numbered copy.'),
		).toBeInTheDocument();
		cleanup();
		mount([conflictFor('a.txt')], {
			job: copyJob({ kind: { kind: 'restore' }, destination: null }),
		});
		await user.selectOptions(choiceFor('a.txt'), 'Keep both');
		expect(within(dialog()).getByText('Restored under a free name.')).toBeInTheDocument();
	});
});

describe('a long list', () => {
	const many = Array.from({ length: ROW_CAP + 30 }, (_, i) =>
		conflictFor(`f${String(i).padStart(3, '0')}.txt`),
	);

	it('draws a capped number of rows and says how many more there are', () => {
		mount(many);
		expect(within(dialog()).getAllByRole('row')).toHaveLength(ROW_CAP + 1);
		expect(within(dialog()).getByText('and 30 more')).toBeInTheDocument();
	});

	it('lets the bulk choice cover the rows that are not drawn', async () => {
		const user = userEvent.setup();
		const handlers = mount(many);
		await user.selectOptions(bulk(), 'Skip');
		expect(continueButton()).toBeEnabled();
		await user.click(continueButton());
		expect(handlers.onContinue.mock.calls[0]![0].decisions).toHaveLength(many.length);
	});

	it('shows them all on request', async () => {
		const user = userEvent.setup();
		mount(many);
		await user.click(within(dialog()).getByRole('button', { name: 'Show all' }));
		expect(within(dialog()).getAllByRole('row')).toHaveLength(many.length + 1);
		expect(within(dialog()).queryByText(/and \d+ more/)).toBeNull();
	});
});

describe('focus and keys', () => {
	it('puts focus on the first control, which is not a choice that destroys anything', () => {
		mount([conflictFor('a.txt')]);
		expect(bulk()).toHaveFocus();
		expect(bulk()).toHaveValue('');
	});

	it('has no destructive button', () => {
		mount([conflictFor('a.txt')]);
		expect(dialog().querySelector('[data-variant="danger"]')).toBeNull();
	});

	it('cancels the job on Escape when nothing has been answered', async () => {
		const user = userEvent.setup();
		const handlers = mount([conflictFor('a.txt')]);
		await user.keyboard('{Escape}');
		expect(handlers.onCancelJob).toHaveBeenCalledOnce();
	});

	it('asks first on Escape once something is answered, and keeps deciding on "Keep deciding"', async () => {
		const user = userEvent.setup();
		const handlers = mount([conflictFor('a.txt')]);
		await user.selectOptions(choiceFor('a.txt'), 'Skip');
		await user.keyboard('{Escape}');
		expect(handlers.onCancelJob).not.toHaveBeenCalled();
		const confirm = screen.getByRole('dialog', { name: 'Cancel the operation?' });
		expect(within(confirm).getByRole('button', { name: 'Keep deciding' })).toHaveFocus();
		await user.keyboard('{Escape}');
		expect(screen.queryByRole('dialog', { name: 'Cancel the operation?' })).toBeNull();
		expect(handlers.onCancelJob).not.toHaveBeenCalled();
		// The answers survive the question.
		expect(choiceFor('a.txt')).toHaveValue('skip');
	});

	it('cancels after the confirmation is accepted', async () => {
		const user = userEvent.setup();
		const handlers = mount([conflictFor('a.txt')]);
		await user.selectOptions(bulk(), 'Skip');
		await user.click(within(dialog()).getByRole('button', { name: 'Cancel the operation' }));
		const confirm = screen.getByRole('dialog', { name: 'Cancel the operation?' });
		await user.click(within(confirm).getByRole('button', { name: 'Cancel the operation' }));
		expect(handlers.onCancelJob).toHaveBeenCalledOnce();
	});

	it('does not cancel on a click outside', async () => {
		const user = userEvent.setup();
		const handlers = mount([conflictFor('a.txt')]);
		await user.click(dialog());
		expect(handlers.onCancelJob).not.toHaveBeenCalled();
		expect(handlers.onLater).not.toHaveBeenCalled();
		expect(dialog()).toBeInTheDocument();
	});

	it('leaves the job waiting on "Decide later"', async () => {
		const user = userEvent.setup();
		const handlers = mount([conflictFor('a.txt')]);
		await user.click(within(dialog()).getByRole('button', { name: 'Decide later' }));
		expect(handlers.onLater).toHaveBeenCalledOnce();
		expect(handlers.onCancelJob).not.toHaveBeenCalled();
	});

	it('is reachable by keyboard alone: Tab moves through the bulk bar, the rows and the buttons', async () => {
		const user = userEvent.setup();
		mount([conflictFor('a.txt')]);
		await user.tab();
		// The "apply to all conflicts" box is off until a bulk choice exists, so it is skipped.
		expect(choiceFor('a.txt')).toHaveFocus();
		await user.keyboard('{ArrowDown}');
		await user.tab();
		expect(within(dialog()).getByRole('button', { name: 'Decide later' })).toHaveFocus();
	});
});
