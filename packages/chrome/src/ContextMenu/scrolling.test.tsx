// Verifies a menu larger than the viewport scrolls inside itself and keeps the focused item visible
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ContextMenu } from './ContextMenu';
import type { MenuItem } from './types';

const many: MenuItem[] = Array.from({ length: 40 }, (_, index) => ({
	type: 'action',
	id: `item-${index}`,
	label: `Item ${index}`,
}));

describe('a menu larger than the viewport', () => {
	const original = Element.prototype.scrollIntoView;
	afterEach(() => {
		Element.prototype.scrollIntoView = original;
	});

	it('is constrained to the viewport and scrolls in its own container', () => {
		// The panel's rule lives in the stylesheet; Vitest empties CSS, so read the real file.
		const css = readFileSync(join(import.meta.dirname, 'ContextMenu.module.css'), 'utf8');
		const menuRule = css.slice(css.indexOf('.menu {'), css.indexOf('}', css.indexOf('.menu {')));
		expect(menuRule).toMatch(/max-height:\s*calc\(100vh/);
		expect(menuRule).toMatch(/max-width:\s*calc\(100vw/);
		expect(menuRule).toMatch(/overflow:\s*auto/);
	});

	it('scrolls the focused item into view as the keyboard moves through it', async () => {
		const scrollIntoView = vi.fn();
		Element.prototype.scrollIntoView = scrollIntoView;
		render(
			<ContextMenu
				items={many}
				position={{ x: 10, y: 10 }}
				onSelect={() => {}}
				onClose={() => {}}
				openedWithKeyboard
			/>,
		);
		const user = userEvent.setup();
		await user.keyboard('{ArrowDown}{ArrowDown}');
		expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Item 2' }));
		expect(scrollIntoView).toHaveBeenCalledWith({ block: 'nearest' });
		expect(scrollIntoView.mock.calls.length).toBeGreaterThanOrEqual(3);
	});

	it('keeps every item reachable by the keyboard, to the end', async () => {
		Element.prototype.scrollIntoView = vi.fn();
		render(
			<ContextMenu
				items={many}
				position={{ x: 10, y: 10 }}
				onSelect={() => {}}
				onClose={() => {}}
				openedWithKeyboard
			/>,
		);
		const user = userEvent.setup();
		await user.keyboard('{End}');
		expect(document.activeElement).toBe(screen.getByRole('menuitem', { name: 'Item 39' }));
	});
});
