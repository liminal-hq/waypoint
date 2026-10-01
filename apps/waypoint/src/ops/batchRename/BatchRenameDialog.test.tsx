// Tests for the batch rename dialog against a fake API: the rule stack, the live preview, problems, Apply and the keyboard
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import type { Sources } from '@liminal-hq/waypoint-protocol/generated/Sources';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ROW_CAP } from './batchRenameModel';
import { BatchRenameDialog } from './BatchRenameDialog';
import { FakeBatchRenameApi, previewOf } from './fakeBatchRenameApi';

const sources: Sources = {
	kind: 'locations',
	locations: [{ display: '/a', uri: 'file:///a' }],
};

function setup(
	options: {
		api?: FakeBatchRenameApi;
		debounceMs?: number;
		onClose?: ReturnType<typeof vi.fn>;
		announce?: ReturnType<typeof vi.fn>;
	} = {},
) {
	const api = options.api ?? new FakeBatchRenameApi();
	const onClose = options.onClose ?? vi.fn();
	const announce = options.announce ?? vi.fn();
	const user = userEvent.setup();
	const utils = render(
		<BatchRenameDialog
			open
			selection={{ sources, count: 2 }}
			api={api}
			onClose={onClose}
			announce={announce}
			debounceMs={options.debounceMs ?? 0}
			utcOffsetMinutes={-300}
		/>,
	);
	return { api, onClose, announce, user, ...utils };
}

const apply = () => screen.getByRole('button', { name: 'Apply' });

afterEach(() => {
	cleanup();
	vi.useRealTimers();
	document.documentElement.removeAttribute('style');
});

describe('opening', () => {
	it('is a dialog named for the batch rename with the count in its description', async () => {
		setup();
		const dialog = await screen.findByRole('dialog', { name: 'Batch rename' });
		expect(dialog).toHaveAccessibleDescription(/Rename 2 items/);
	});

	it('renders nothing while closed', () => {
		render(
			<BatchRenameDialog
				open={false}
				selection={{ sources }}
				api={new FakeBatchRenameApi()}
				onClose={() => {}}
			/>,
		);
		expect(document.querySelector('dialog')).toBeNull();
	});

	it('focuses the first control, which is not Apply', async () => {
		setup();
		const first = await screen.findByRole('combobox', { name: 'Rule type' });
		expect(first).toHaveFocus();
		expect(apply()).not.toHaveFocus();
	});

	it('starts with one find and replace rule', async () => {
		setup();
		await screen.findByRole('dialog');
		expect(screen.getByRole('heading', { name: 'Rule 1' })).toBeInTheDocument();
		expect(screen.getByLabelText('Find')).toHaveValue('');
	});
});

describe('the live preview', () => {
	it('asks for the preview of the stack once it is open, as a batch rename request', async () => {
		const { api } = setup();
		await waitFor(() => expect(api.previews).toHaveLength(1));
		const request = api.previews[0]!;
		expect(request.kind).toEqual({ kind: 'batchRename' });
		expect(request.sources).toBe(sources);
		expect(request.rename?.utcOffsetMinutes).toBe(-300);
		expect(request.rename?.rules).toHaveLength(1);
		expect(request.rename?.rules[0]).toMatchObject({ kind: 'findReplace', find: '' });
	});

	it('shows each entry before and after, and counts what will be renamed', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = () =>
			previewOf([
				{ from: 'a.txt', to: 'b.txt' },
				{ from: 'c.txt', to: 'c.txt', problems: [{ kind: 'unchangedSkip' }] },
			]);
		setup({ api });
		const table = await screen.findByRole('table', { name: 'New names' });
		const rows = within(table).getAllByRole('row');
		expect(rows).toHaveLength(3);
		expect(within(rows[1]!).getByText('a.txt')).toBeInTheDocument();
		expect(within(rows[1]!).getByText('b.txt')).toBeInTheDocument();
		expect(within(rows[2]!).getByText('No change')).toBeInTheDocument();
		expect(screen.getByRole('status')).toHaveTextContent('1 item will be renamed');
	});

	it('waits for typing to pause, then asks once for the whole stack', async () => {
		const api = new FakeBatchRenameApi();
		const { user } = setup({ api, debounceMs: 150 });
		await waitFor(() => expect(api.previews).toHaveLength(1));
		await user.type(screen.getByLabelText('Find'), 'abc');
		expect(api.previews).toHaveLength(1);
		await waitFor(() => expect(api.previews).toHaveLength(2));
		await new Promise((resolve) => setTimeout(resolve, 300));
		expect(api.previews).toHaveLength(2);
		expect(api.previews[1]!.rename?.rules[0]).toMatchObject({ find: 'abc' });
	});

	it('drops the answer to a stack that has changed since', async () => {
		const api = new FakeBatchRenameApi();
		let release: (() => void) | undefined;
		const first = new Promise<void>((resolve) => (release = resolve));
		let calls = 0;
		api.respond = async () => {
			calls += 1;
			if (calls === 1) {
				await first;
				return previewOf([{ from: 'stale', to: 'stale2' }]);
			}
			return previewOf([{ from: 'fresh', to: 'fresh2' }]);
		};
		const { user } = setup({ api });
		await waitFor(() => expect(calls).toBe(1));
		await user.type(screen.getByLabelText('Find'), 'x');
		await screen.findByText('fresh2');
		await act(async () => release?.());
		expect(screen.queryByText('stale2')).toBeNull();
		expect(screen.getByText('fresh2')).toBeInTheDocument();
	});

	it('says so when the preview cannot be made, and keeps Apply off', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = () => Promise.reject({ kind: 'ops', message: 'the folder is gone' });
		setup({ api });
		expect(
			await screen.findByText('The new names could not be worked out: the folder is gone'),
		).toHaveAttribute('role', 'status');
		expect(apply()).toBeDisabled();
	});

	it('draws at most the cap of rows and says how many were left out', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = () =>
			previewOf(Array.from({ length: ROW_CAP + 5 }, (_, i) => ({ from: `f${i}`, to: `g${i}` })));
		setup({ api });
		await screen.findByRole('table');
		expect(screen.getAllByRole('row')).toHaveLength(ROW_CAP + 1);
		expect(screen.getByText(`Showing the first ${ROW_CAP} of ${ROW_CAP + 5} items`)).toBeVisible();
		expect(screen.getByRole('status')).toHaveTextContent(`${ROW_CAP + 5} items will be renamed`);
	});
});

describe('problems', () => {
	const clash = () =>
		previewOf([
			{ from: 'a.txt', to: 'x.txt', problems: [{ kind: 'duplicateTarget', with: 1 }] },
			{ from: 'b.txt', to: 'x.txt', problems: [{ kind: 'duplicateTarget', with: 0 }] },
			{ from: 'c.txt', to: 'keep.txt', problems: [{ kind: 'existsInFolder' }] },
			{
				from: 'd.txt',
				to: 'a/b',
				problems: [{ kind: 'invalid', reason: 'a name cannot contain `/`' }],
			},
			{ from: 'e.txt', to: 'f.txt' },
		]);

	it('marks each row that has one, in words as well as by colour', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = clash;
		setup({ api });
		const table = await screen.findByRole('table');
		const rows = within(table).getAllByRole('row').slice(1);
		expect(rows.map((row) => row.getAttribute('data-problem'))).toEqual([
			'true',
			'true',
			'true',
			'true',
			null,
		]);
		expect(within(rows[0]!).getByText('Problem: The same name as “x.txt”')).toBeVisible();
		expect(
			within(rows[2]!).getByText('Problem: A file or folder with this name is already here'),
		).toBeVisible();
		expect(within(rows[3]!).getByText('Problem: a name cannot contain `/`')).toBeVisible();
		expect(within(rows[4]!).queryByText(/Problem/)).toBeNull();
	});

	it('sums them in the summary line, which is a live region', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = clash;
		setup({ api });
		const summary = await screen.findByText('4 problems');
		expect(summary).toHaveAttribute('role', 'status');
		expect(summary).toHaveAttribute('aria-live', 'polite');
	});

	it('shows a rule that cannot work under that rule', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = () =>
			previewOf([{ from: 'a', to: 'a' }], {
				ruleErrors: [{ rule: 0, reason: 'the pattern is not valid' }],
			});
		setup({ api });
		const alert = await screen.findByText(/Rule 1: the pattern is not valid/);
		expect(alert).toHaveTextContent('Problem:');
		expect(screen.getByRole('status')).toHaveTextContent('1 problem');
		expect(apply()).toBeDisabled();
	});
});

describe('Apply', () => {
	const renames = () => previewOf([{ from: 'a.txt', to: 'b.txt' }]);

	it('is disabled until the preview is back, then while nothing changes', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = () => previewOf([{ from: 'a', to: 'a' }]);
		setup({ api });
		expect(apply()).toBeDisabled();
		await screen.findByText('No names change');
		expect(apply()).toBeDisabled();
	});

	it('is disabled while a problem stands', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = () => previewOf([{ from: 'a', to: 'b', problems: [{ kind: 'existsInFolder' }] }]);
		setup({ api });
		await screen.findByText('1 problem');
		expect(apply()).toBeDisabled();
	});

	it('is enabled once something changes and nothing is wrong', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = renames;
		setup({ api });
		await waitFor(() => expect(apply()).toBeEnabled());
	});

	it('goes off again while the next preview is being asked for', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = renames;
		const { user } = setup({ api, debounceMs: 50 });
		await waitFor(() => expect(apply()).toBeEnabled());
		api.respond = () => new Promise(() => {});
		await user.type(screen.getByLabelText('Find'), 'x');
		expect(apply()).toBeDisabled();
	});

	it('queues the job with the time the preview used, announces it and closes', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = () => previewOf([{ from: 'a.txt', to: 'b.txt' }], { nowMs: 42 });
		const { user, onClose, announce } = setup({ api });
		await waitFor(() => expect(apply()).toBeEnabled());
		await user.click(apply());
		await waitFor(() => expect(onClose).toHaveBeenCalledWith('applied'));
		expect(api.applied).toHaveLength(1);
		expect(api.applied[0]!.rename?.nowMs).toBe(42);
		expect(api.applied[0]!.rename?.rules).toEqual(api.previews[0]!.rename?.rules);
		expect(announce).toHaveBeenCalledWith('Renaming 1 item');
	});

	it('stays open and says why when the job cannot be queued', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = renames;
		api.applyResult = Object.assign(new Error('the queue is closed'), { kind: 'queue' });
		const { user, onClose } = setup({ api });
		await waitFor(() => expect(apply()).toBeEnabled());
		await user.click(apply());
		expect(await screen.findByRole('alert')).toHaveTextContent(
			'The rename could not be started: the queue is closed',
		);
		expect(onClose).not.toHaveBeenCalled();
		expect(apply()).toBeEnabled();
	});
});

describe('the rule stack', () => {
	it('adds a rule of the chosen type at the end', async () => {
		const { user, api } = setup();
		await screen.findByRole('dialog');
		await user.selectOptions(screen.getByRole('combobox', { name: 'Type of rule to add' }), 'case');
		await user.click(screen.getByRole('button', { name: 'Add rule' }));
		expect(screen.getByRole('heading', { name: 'Rule 2' })).toBeInTheDocument();
		await waitFor(() => expect(api.previews.at(-1)!.rename?.rules).toHaveLength(2));
		expect(api.previews.at(-1)!.rename?.rules[1]).toMatchObject({ kind: 'case' });
	});

	it('changes a rule to another type with that type’s defaults', async () => {
		const { user, api } = setup();
		await screen.findByRole('dialog');
		await user.selectOptions(screen.getByRole('combobox', { name: 'Rule type' }), 'counter');
		expect(screen.getByLabelText('Start at')).toHaveValue(1);
		await waitFor(() =>
			expect(api.previews.at(-1)!.rename?.rules[0]).toMatchObject({ kind: 'counter', width: 2 }),
		);
	});

	it('removes a rule, which renumbers the rest', async () => {
		const { user, api } = setup();
		await screen.findByRole('dialog');
		await user.click(screen.getByRole('button', { name: 'Add rule' }));
		await user.click(screen.getByRole('button', { name: 'Remove rule 1' }));
		expect(screen.getAllByRole('heading', { level: 4 })).toHaveLength(1);
		expect(screen.getByRole('heading', { name: 'Rule 1' })).toBeInTheDocument();
		await waitFor(() => expect(api.previews.at(-1)!.rename?.rules).toHaveLength(1));
	});

	it('reorders rules with the move buttons, and the order is the order sent', async () => {
		const { user, api } = setup();
		await screen.findByRole('dialog');
		await user.selectOptions(
			screen.getByRole('combobox', { name: 'Type of rule to add' }),
			'trimWhitespace',
		);
		await user.click(screen.getByRole('button', { name: 'Add rule' }));
		expect(screen.getByRole('button', { name: 'Move rule 1 up' })).toBeDisabled();
		expect(screen.getByRole('button', { name: 'Move rule 2 down' })).toBeDisabled();
		await user.click(screen.getByRole('button', { name: 'Move rule 2 up' }));
		await waitFor(() =>
			expect(api.previews.at(-1)!.rename?.rules.map((r) => r.kind)).toEqual([
				'trimWhitespace',
				'findReplace',
			]),
		);
	});

	it('keeps what was typed in a rule when another is moved past it', async () => {
		const { user } = setup();
		await screen.findByRole('dialog');
		await user.type(screen.getByLabelText('Find'), 'keep me');
		await user.click(screen.getByRole('button', { name: 'Add rule' }));
		await user.click(screen.getByRole('button', { name: 'Move rule 2 up' }));
		expect(screen.getByDisplayValue('keep me')).toBeInTheDocument();
	});
});

describe('the extension guard', () => {
	it('shows a note when a rule would change an extension, and not otherwise', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = () => previewOf([{ from: 'a.txt', to: 'a.md', extensionChanged: true }]);
		const { rerender } = setup({ api });
		expect(await screen.findByText(/A rule changes a file extension/)).toBeVisible();
		expect(screen.getByText('The extension changes')).toBeVisible();
		rerender(<></>);
		const quiet = new FakeBatchRenameApi();
		quiet.respond = () => previewOf([{ from: 'a.txt', to: 'b.txt' }]);
		setup({ api: quiet });
		await screen.findByText('1 item will be renamed');
		expect(screen.queryByText(/A rule changes a file extension/)).toBeNull();
	});
});

describe('the keyboard', () => {
	it('closes with Escape, as a cancel', async () => {
		const { user, onClose } = setup();
		await screen.findByRole('dialog');
		await user.keyboard('{Escape}');
		expect(onClose).toHaveBeenCalledWith('cancelled');
	});

	it('closes with Cancel without queueing anything', async () => {
		const { user, onClose, api } = setup();
		await screen.findByRole('dialog');
		await user.click(screen.getByRole('button', { name: 'Cancel' }));
		expect(onClose).toHaveBeenCalledWith('cancelled');
		expect(api.applied).toHaveLength(0);
	});

	it('reaches every control with Tab, in reading order, ending on the buttons', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = () => previewOf([{ from: 'a', to: 'b' }]);
		const { user } = setup({ api });
		await waitFor(() => expect(apply()).toBeEnabled());
		const order: string[] = [];
		const name = (element: Element) =>
			element.getAttribute('aria-label') ??
			(element as HTMLInputElement).labels?.[0]?.textContent ??
			element.textContent ??
			'';
		order.push(name(document.activeElement!));
		for (let step = 0; step < 14; step += 1) {
			await user.tab();
			order.push(name(document.activeElement!));
		}
		expect(order.slice(0, 7)).toEqual([
			'Rule type',
			'Remove rule 1',
			'Find',
			'Replace with',
			'Apply to',
			'Regular expression',
			'Match case',
		]);
		expect(order).toContain('Add rule');
		expect(order).toContain('Cancel');
		expect(order.indexOf('Apply')).toBeGreaterThan(order.indexOf('Add rule'));
	});

	it('can be driven without a pointer from add to apply', async () => {
		const api = new FakeBatchRenameApi();
		api.respond = (request) =>
			previewOf([
				{
					from: 'a',
					to: request.rename?.rules.some((r) => r.kind === 'case') ? 'A' : 'a',
				},
			]);
		const { user, onClose } = setup({ api });
		await screen.findByRole('dialog');
		screen.getByRole('combobox', { name: 'Type of rule to add' }).focus();
		await user.keyboard('{ArrowDown}');
		await user.tab();
		await user.keyboard('{Enter}');
		expect(screen.getByRole('heading', { name: 'Rule 2' })).toBeInTheDocument();
		await user.selectOptions(screen.getAllByRole('combobox', { name: 'Rule type' })[1]!, 'case');
		await waitFor(() => expect(apply()).toBeEnabled());
		apply().focus();
		await user.keyboard('{Enter}');
		await waitFor(() => expect(onClose).toHaveBeenCalledWith('applied'));
	});
});
