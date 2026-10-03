// Tests for the Help dialogs: each opens from its key or command, shows the registry's keys, moves on to its siblings and closes
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandBridgeProvider, createCommandBridge } from '../commands/commandBridge';
import { runCommand } from '../commands/registry';
import { t, tf } from '../i18n/messages';
import { FakeAppInfoClient } from '../services/fakeAppInfoClient';
import { factsFor } from '../test/commandFacts';
import { HelpHost } from './HelpHost';
import { TOUR_LENGTH } from './helpModel';

afterEach(() => {
	cleanup();
	vi.unstubAllGlobals();
});

function setup(client = new FakeAppInfoClient('1.2.3')) {
	const bridge = createCommandBridge({ facts: factsFor({ selected: 1, focused: true }) });
	render(
		<CommandBridgeProvider value={bridge}>
			<button>Pane</button>
			<input aria-label="Field" />
			<HelpHost appInfo={client} />
		</CommandBridgeProvider>,
	);
	const pane = screen.getByRole('button', { name: 'Pane' });
	pane.focus();
	return { bridge, client, pane };
}

const press = (key: string, target: Element | Window = window) =>
	fireEvent.keyDown(target, { key, bubbles: true, cancelable: true });

const dialog = (name: string) => screen.findByRole('dialog', { name });

describe('Help', () => {
	it('opens on F1 with the registry’s keys in a list', async () => {
		setup();
		press('F1');
		const help = await dialog(t('help.title'));
		expect(within(help).getByText(t('help.subtitle'))).toBeInTheDocument();
		const rows = within(
			within(help).getByRole('list', { name: t('help.rows.label') }),
		).getAllByRole('listitem');
		expect(rows.length).toBeGreaterThanOrEqual(5);
		const keys = rows.map((row) => row.querySelector('kbd')?.textContent);
		expect(keys).toContain('Ctrl+Shift+P');
		expect(keys).toContain('F3');
		expect(keys).toContain('Ctrl+Z');
		expect(keys).toContain('?');
		for (const row of rows) expect(row.querySelector('kbd')).not.toBeNull();
	});

	it('opens from the command, and the command is how the menu and palette open it', async () => {
		const { bridge } = setup();
		const { actions, facts } = bridge.store.getState();
		act(() => void runCommand('help', actions, facts));
		expect(await dialog(t('help.title'))).toBeInTheDocument();
	});

	it('keeps its picture, caption included, out of the accessibility tree', async () => {
		setup();
		press('F1');
		const help = await dialog(t('help.title'));
		const caption = within(help).getByText(t('help.art.caption'));
		expect(caption.closest('[aria-hidden="true"]')).not.toBeNull();
		expect(within(help).queryByRole('img')).toBeNull();
	});

	it('stops the picture’s motion under reduce motion', async () => {
		vi.stubGlobal('matchMedia', () => ({
			matches: true,
			addEventListener: () => {},
			removeEventListener: () => {},
		}));
		setup();
		press('F1');
		const help = await dialog(t('help.title'));
		expect(help.querySelector('[data-reduced-motion]')).not.toBeNull();
	});

	it('animates the picture otherwise', async () => {
		setup();
		press('F1');
		const help = await dialog(t('help.title'));
		expect(help.querySelector('[data-reduced-motion]')).toBeNull();
	});

	it('closes on Esc and returns focus to where it was', async () => {
		const { pane } = setup();
		press('F1');
		await dialog(t('help.title'));
		press('Escape', document.body);
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
		expect(pane).toHaveFocus();
	});

	it('has a Close button that is where focus starts', async () => {
		setup();
		press('F1');
		const help = await dialog(t('help.title'));
		const close = within(help).getByRole('button', { name: t('help.close') });
		expect(close).toHaveFocus();
		await userEvent.click(close);
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});

	it('leads to the shortcut list, the tour and About', async () => {
		setup();
		const user = userEvent.setup();
		press('F1');
		await user.click(
			within(await dialog(t('help.title'))).getByRole('button', {
				name: t('cmd.keyboardShortcuts'),
			}),
		);
		expect(await dialog(t('shortcuts.title'))).toBeInTheDocument();
		press('F1');
		await user.click(
			within(await dialog(t('help.title'))).getByRole('button', { name: t('cmd.tour') }),
		);
		expect(await dialog(t('tour.title.welcome'))).toBeInTheDocument();
		press('F1');
		await user.click(
			within(await dialog(t('help.title'))).getByRole('button', { name: t('help.about') }),
		);
		expect(await dialog(t('about.title'))).toBeInTheDocument();
	});

	it('hands focus back to the opener when the last dialog it led to closes', async () => {
		const { pane } = setup();
		const user = userEvent.setup();
		press('F1');
		await user.click(
			within(await dialog(t('help.title'))).getByRole('button', { name: t('help.about') }),
		);
		await dialog(t('about.title'));
		press('Escape', document.body);
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
		expect(pane).toHaveFocus();
	});
});

describe('the keys', () => {
	it('? opens the shortcut list, but not while typing in a field', async () => {
		setup();
		const field = screen.getByLabelText('Field');
		field.focus();
		press('?', field);
		expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
		press('?');
		expect(await dialog(t('shortcuts.title'))).toBeInTheDocument();
	});

	it('F1 works in a field', async () => {
		setup();
		const field = screen.getByLabelText('Field');
		field.focus();
		press('F1', field);
		expect(await dialog(t('help.title'))).toBeInTheDocument();
	});

	it('leaves another dialog alone', async () => {
		setup();
		const other = document.createElement('dialog');
		other.setAttribute('open', '');
		document.body.append(other);
		press('F1');
		expect(screen.queryByRole('dialog', { name: t('help.title') })).not.toBeInTheDocument();
		other.remove();
	});

	it('switches pages when one is already open', async () => {
		setup();
		press('F1');
		await dialog(t('help.title'));
		press('?');
		expect(await dialog(t('shortcuts.title'))).toBeInTheDocument();
		expect(screen.queryByRole('dialog', { name: t('help.title') })).not.toBeInTheDocument();
	});
});

describe('Keyboard Shortcuts', () => {
	it('lists the registry’s commands by group as keyboard chips, and Done closes it', async () => {
		const { bridge } = setup();
		act(() => bridge.store.getState().actions.openHelp('shortcuts'));
		const shortcuts = await dialog(t('shortcuts.title'));
		const lists = within(shortcuts).getAllByRole('list');
		expect(lists.length).toBeGreaterThan(3);
		const items = within(shortcuts).getAllByRole('listitem');
		expect(items.length).toBeGreaterThan(20);
		for (const item of items) expect(item.querySelector('kbd')?.textContent).toBeTruthy();
		const file = within(shortcuts).getByRole('list', {
			name: tf('shortcuts.list.label', { group: t('palette.group.file') }),
		});
		expect(within(file).getByText(t('cmd.newTab').replace(/…$/, ''))).toBeInTheDocument();
		await userEvent.click(within(shortcuts).getByRole('button', { name: t('shortcuts.done') }));
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});

	it('leaves out a command the window hides', async () => {
		const { bridge } = setup();
		act(() => bridge.store.getState().actions.openHelp('shortcuts'));
		const shortcuts = await dialog(t('shortcuts.title'));
		// Restore is the Trash's own command, so a folder's list does not have it.
		expect(within(shortcuts).queryByText(t('trash.restore'))).toBeNull();
	});
});

describe('the tour', () => {
	it('walks the five steps with Next and Back and finishes with Get Started', async () => {
		const { bridge } = setup();
		const user = userEvent.setup();
		act(() => bridge.store.getState().actions.openHelp('tour'));
		const first = await dialog(t('tour.title.welcome'));
		expect(within(first).getByRole('status')).toHaveTextContent(
			tf('tour.progress', { step: 1, total: TOUR_LENGTH }),
		);
		expect(within(first).queryByRole('button', { name: t('tour.back') })).toBeNull();
		const next = within(first).getByRole('button', { name: t('tour.next') });
		expect(next).toHaveFocus();

		await user.click(next);
		const second = await dialog(t('tour.title.tabs'));
		expect(within(second).getByRole('status')).toHaveTextContent('Step 2 of 5');
		await user.click(within(second).getByRole('button', { name: t('tour.back') }));
		expect(await dialog(t('tour.title.welcome'))).toBeInTheDocument();

		for (let step = 1; step < TOUR_LENGTH; step++) {
			await user.click(screen.getByRole('button', { name: t('tour.next') }));
		}
		const last = await dialog(t('tour.title.desktop'));
		expect(within(last).queryByRole('button', { name: t('tour.next') })).toBeNull();
		await user.click(within(last).getByRole('button', { name: t('tour.done') }));
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});

	it('is left with Skip or Esc', async () => {
		const { bridge } = setup();
		const user = userEvent.setup();
		act(() => bridge.store.getState().actions.openHelp('tour'));
		await user.click(
			within(await dialog(t('tour.title.welcome'))).getByRole('button', { name: t('tour.skip') }),
		);
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
		act(() => bridge.store.getState().actions.openHelp('tour'));
		await dialog(t('tour.title.welcome'));
		press('Escape', document.body);
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});
});

describe('About', () => {
	it('shows the version the client reports, the licence and what it is built with', async () => {
		const { bridge, client } = setup();
		act(() => bridge.store.getState().actions.openHelp('about'));
		const about = await dialog(t('about.title'));
		expect(await within(about).findByText(tf('about.version', { version: '1.2.3' }))).toBeVisible();
		expect(client.reads).toBe(1);
		expect(within(about).getByText('Apache-2.0 OR MIT')).toBeInTheDocument();
		expect(within(about).getByText(t('about.builtWith.value'))).toBeInTheDocument();
		expect(within(about).queryByText(/update/i)).toBeNull();
		await userEvent.click(within(about).getByRole('button', { name: t('about.close') }));
		await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});

	it('says the version could not be read when the client fails', async () => {
		const client = new FakeAppInfoClient();
		client.failReads();
		vi.spyOn(console, 'warn').mockImplementation(() => {});
		const { bridge } = setup(client);
		act(() => bridge.store.getState().actions.openHelp('about'));
		const about = await dialog(t('about.title'));
		expect(await within(about).findByText(t('about.versionUnavailable'))).toBeInTheDocument();
	});
});
