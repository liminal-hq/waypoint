// Verifies the toolbar's panel toggles: their state, their command path, their tooltips and the overflow menu
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { stubLayout } from '../test/browseHarness';
import { renderWorkspace } from '../test/workspaceHarness';
import { COLLAPSE_BELOW, PANEL_TOGGLES, shouldCollapse } from './panelToggles';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

const toggle = (name: string) =>
	within(screen.getByRole('group', { name: 'Panels' })).getByRole('button', { name });

describe('the panel toggles', () => {
	it('offers the sidebar and split view, and not the Shelf (the status bar has its button) or the panels that do not exist yet', async () => {
		await renderWorkspace();
		const group = screen.getByRole('group', { name: 'Panels' });
		expect(
			within(group)
				.getAllByRole('button')
				.map((b) => b.getAttribute('aria-label')),
		).toEqual(['Sidebar', 'Split View']);
		expect(PANEL_TOGGLES).toEqual(['sidebar', 'splitView']);
		expect(screen.queryByRole('button', { name: /inspector|terminal/i })).toBeNull();
	});

	it('shows the command state in aria-pressed and the label and key in the tooltip', async () => {
		await renderWorkspace();
		expect(toggle('Sidebar')).toHaveAttribute('aria-pressed', 'false');
		expect(toggle('Sidebar')).toHaveAttribute('title', 'Sidebar (F9)');
		expect(toggle('Split View')).toHaveAttribute('title', 'Split View (F3)');
	});

	it('runs the registry command and follows the state it changes, from the button and the key', async () => {
		await renderWorkspace();
		fireEvent.click(toggle('Sidebar'));
		await waitFor(() => expect(toggle('Sidebar')).toHaveAttribute('aria-pressed', 'true'));
		expect(screen.getByRole('navigation', { name: /places|sidebar/i })).toBeTruthy();
		// The key goes through the same command, so the button follows it.
		fireEvent.keyDown(window, { key: 'F9' });
		await waitFor(() => expect(toggle('Sidebar')).toHaveAttribute('aria-pressed', 'false'));
	});

	it('pairs the tab and unpairs it', async () => {
		await renderWorkspace();
		fireEvent.click(toggle('Split View'));
		await waitFor(() => expect(toggle('Split View')).toHaveAttribute('aria-pressed', 'true'));
		fireEvent.click(toggle('Split View'));
		await waitFor(() => expect(toggle('Split View')).toHaveAttribute('aria-pressed', 'false'));
	});
});

describe('the overflow menu', () => {
	it('decides by the toolbar width, and shows everything where nothing is laid out', () => {
		expect(shouldCollapse(0)).toBe(false);
		expect(shouldCollapse(COLLAPSE_BELOW)).toBe(false);
		expect(shouldCollapse(COLLAPSE_BELOW - 1)).toBe(true);
	});

	it('replaces the buttons with a More button that lists the same commands with their state', async () => {
		restoreLayout();
		restoreLayout = stubLayout(280, 400);
		await renderWorkspace();
		expect(
			screen.queryByRole('group', { name: 'Panels' })?.querySelector('[aria-pressed]'),
		).toBeNull();
		const more = screen.getByRole('button', { name: 'More panels' });
		fireEvent.click(more);
		const menu = await screen.findByRole('menu', { name: 'Panels' });
		const sidebar = within(menu).getByRole('menuitemcheckbox', { name: /Sidebar/ });
		expect(sidebar).toHaveAttribute('aria-checked', 'false');
		expect(within(menu).getByRole('menuitemcheckbox', { name: /Split View/ })).toBeTruthy();

		await act(async () => fireEvent.click(sidebar));
		fireEvent.click(screen.getByRole('button', { name: 'More panels' }));
		const again = await screen.findByRole('menu', { name: 'Panels' });
		expect(within(again).getByRole('menuitemcheckbox', { name: /Sidebar/ })).toHaveAttribute(
			'aria-checked',
			'true',
		);
	});
});
