// Verifies the confirmation dialog: a safe default focus, Esc and Cancel answering no, and the list of names
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it } from 'vitest';
import { useConfirm } from './ConfirmHost';
import type { ConfirmSpec } from './fileCommands';

afterEach(cleanup);

const danger: ConfirmSpec = {
	title: 'Delete permanently?',
	message: 'This permanently deletes 3 items. It cannot be undone.',
	items: ['a.txt', 'b.txt', 'c.txt'],
	note: 'Total size: 3 kB',
	confirmLabel: 'Delete Permanently',
	danger: true,
};

function mount() {
	const asked: Array<Promise<boolean>> = [];
	function Host() {
		const { confirm, dialog } = useConfirm();
		return (
			<>
				<button onClick={() => asked.push(confirm(danger))}>ask</button>
				<button onClick={() => asked.push(confirm({ ...danger, danger: false, items: [] }))}>
					ask plainly
				</button>
				{dialog}
			</>
		);
	}
	render(<Host />);
	return asked;
}

describe('useConfirm', () => {
	it('lists the names and the note, and starts with focus on Cancel for a destructive question', async () => {
		const user = userEvent.setup();
		mount();
		await user.click(screen.getByText('ask'));
		const dialog = await screen.findByRole('dialog', { name: 'Delete permanently?' });
		expect(dialog).toHaveAccessibleDescription(/cannot be undone/);
		expect(
			within(dialog)
				.getAllByRole('listitem')
				.map((li) => li.textContent),
		).toEqual(['a.txt', 'b.txt', 'c.txt']);
		expect(within(dialog).getByText('Total size: 3 kB')).toBeInTheDocument();
		expect(within(dialog).getByRole('button', { name: 'Cancel' })).toHaveFocus();
	});

	it('answers false on Escape and on Cancel, so Enter on the default cannot delete', async () => {
		const user = userEvent.setup();
		const asked = mount();
		await user.click(screen.getByText('ask'));
		await screen.findByRole('dialog');
		await user.keyboard('{Enter}'); // Enter on the focused Cancel
		expect(await asked[0]).toBe(false);
		await user.click(screen.getByText('ask'));
		await screen.findByRole('dialog');
		await user.keyboard('{Escape}');
		expect(await asked[1]).toBe(false);
		expect(screen.queryByRole('dialog')).toBeNull();
	});

	it('answers true only on the confirm button', async () => {
		const user = userEvent.setup();
		const asked = mount();
		await user.click(screen.getByText('ask'));
		await user.click(await screen.findByRole('button', { name: 'Delete Permanently' }));
		expect(await asked[0]).toBe(true);
	});

	it('answers a question still open false when another is asked', async () => {
		const user = userEvent.setup();
		const asked = mount();
		await user.click(screen.getByText('ask'));
		await screen.findByRole('dialog');
		await act(async () => {
			screen.getByText('ask plainly').click();
		});
		expect(await asked[0]).toBe(false);
	});
});
