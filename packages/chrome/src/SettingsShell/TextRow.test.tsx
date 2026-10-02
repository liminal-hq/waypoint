// Tests for the text row: it applies a typed value once, on Enter or when left, and never an unchanged one
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { TextRow } from './TextRow';

afterEach(cleanup);

describe('TextRow', () => {
	it('is labelled by its row and shows the value in force', () => {
		render(<TextRow label="Shortcut" value="Ctrl+Alt+W" onCommit={() => {}} />);
		expect(screen.getByRole('textbox', { name: 'Shortcut' })).toHaveValue('Ctrl+Alt+W');
	});

	it('applies the typed text on Enter, once', async () => {
		const onCommit = vi.fn();
		render(<TextRow label="Shortcut" value="Ctrl+Alt+W" onCommit={onCommit} />);
		const field = screen.getByRole('textbox', { name: 'Shortcut' });
		await userEvent.clear(field);
		await userEvent.type(field, 'Ctrl+Shift+K{Enter}');
		expect(onCommit).toHaveBeenCalledTimes(1);
		expect(onCommit).toHaveBeenCalledWith('Ctrl+Shift+K');
		// Leaving the field after Enter has nothing more to apply.
		await userEvent.tab();
		expect(onCommit).toHaveBeenCalledTimes(1);
	});

	it('applies the typed text when the field is left', async () => {
		const onCommit = vi.fn();
		render(<TextRow label="Shortcut" value="" onCommit={onCommit} placeholder="Ctrl+Alt+W" />);
		await userEvent.type(screen.getByRole('textbox', { name: 'Shortcut' }), 'Alt+Q');
		await userEvent.tab();
		expect(onCommit).toHaveBeenCalledWith('Alt+Q');
	});

	it('applies nothing when the text is unchanged, and Escape drops the draft', async () => {
		const onCommit = vi.fn();
		render(<TextRow label="Shortcut" value="Ctrl+Alt+W" onCommit={onCommit} />);
		const field = screen.getByRole('textbox', { name: 'Shortcut' });
		await userEvent.click(field);
		await userEvent.tab();
		await userEvent.type(field, 'x{Escape}');
		await userEvent.tab();
		expect(onCommit).not.toHaveBeenCalled();
		expect(field).toHaveValue('Ctrl+Alt+W');
	});

	it('marks the field invalid and described when the row has an error', () => {
		render(
			<TextRow label="Shortcut" value="x" onCommit={() => {}} error="write it like Ctrl+Alt+W" />,
		);
		const field = screen.getByRole('textbox', { name: 'Shortcut' });
		expect(field).toHaveAttribute('aria-invalid', 'true');
		expect(field).toHaveAccessibleDescription('write it like Ctrl+Alt+W');
	});
});
