// Verifies navigation end to end: history, up, breadcrumbs, the editable path and the error states
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { stubLayout } from '../test/browseHarness';
import { DOCS, HOME, MUSIC, renderWorkspace } from '../test/workspaceHarness';

let restoreLayout: () => void;
beforeEach(() => {
	restoreLayout = stubLayout(280);
});
afterEach(() => {
	cleanup();
	restoreLayout();
});

const option = (name: string) => screen.findByRole('option', { name: new RegExp(`^${name}`) });
const back = () => screen.getByRole('button', { name: 'Back' });
const forward = () => screen.getByRole('button', { name: 'Forward' });
const up = () => screen.getByRole('button', { name: 'Up' });

async function openFolder(name: string) {
	fireEvent.doubleClick(await option(name));
}

async function currentUri(tabs: Awaited<ReturnType<typeof renderWorkspace>>['tabs']) {
	const snapshot = await tabs.getSnapshot();
	return snapshot.tabs.find((tab) => tab.id === snapshot.active)!.location.uri;
}

describe('the toolbar', () => {
	it('starts with Back and Forward disabled and Up enabled below the root', async () => {
		await renderWorkspace();
		await option('docs');
		expect(back()).toBeDisabled();
		expect(forward()).toBeDisabled();
		await waitFor(() => expect(up()).toBeEnabled());
	});

	it('opens a folder on double-click and goes back and forward through history', async () => {
		const { tabs } = await renderWorkspace();
		await openFolder('docs');
		await option('report.pdf');
		expect(await currentUri(tabs)).toBe(DOCS.uri);
		expect(back()).toBeEnabled();

		fireEvent.click(back());
		await option('notes.txt');
		expect(forward()).toBeEnabled();
		fireEvent.click(forward());
		await option('report.pdf');
	});

	it('goes up to the parent and disables Up at the root', async () => {
		const { client } = await renderWorkspace();
		await option('docs');
		await waitFor(() => expect(up()).toBeEnabled());
		fireEvent.click(up());
		await option('test');
		fireEvent.click(up());
		await option('home');
		await waitFor(() => expect(up()).toBeDisabled());
		expect((await client.describeLocation({ display: '/', uri: 'file:///' })).parent).toBeNull();
	});

	it('follows Alt+Left, Alt+Right and Alt+Up from anywhere', async () => {
		await renderWorkspace();
		await openFolder('docs');
		await option('report.pdf');
		fireEvent.keyDown(window, { key: 'ArrowLeft', altKey: true });
		await option('notes.txt');
		fireEvent.keyDown(window, { key: 'ArrowRight', altKey: true });
		await option('report.pdf');
		fireEvent.keyDown(window, { key: 'ArrowUp', altKey: true });
		await option('docs');
	});
});

describe('the mouse back and forward buttons', () => {
	it('step through history from anywhere in the window', async () => {
		await renderWorkspace();
		await openFolder('docs');
		await option('report.pdf');
		fireEvent.mouseUp(window, { button: 3 });
		await option('notes.txt');
		fireEvent.mouseUp(window, { button: 4 });
		await option('report.pdf');
	});

	it('cancel the press and the click the webview would turn into its own history navigation', async () => {
		await renderWorkspace();
		for (const type of ['mousedown', 'auxclick']) {
			for (const button of [3, 4]) {
				const event = new MouseEvent(type, { button, cancelable: true, bubbles: true });
				expect(fireEvent(window, event)).toBe(false);
			}
		}
	});

	it('leave the primary, middle and right buttons alone', async () => {
		await renderWorkspace();
		await openFolder('docs');
		await option('report.pdf');
		for (const button of [0, 1, 2]) {
			expect(fireEvent.mouseUp(window, { button, cancelable: true })).toBe(true);
		}
		await option('report.pdf');
	});
});

describe('history menus', () => {
	it('lists the folders behind on right-click and jumps to the one chosen', async () => {
		const { tabs } = await renderWorkspace();
		await openFolder('docs');
		await option('report.pdf');
		fireEvent.click(up());
		await option('docs');
		await openFolder('music');
		await option('song.mp3');
		// History behind, nearest first: /home/test, /home/test/docs, /home/test.
		fireEvent.contextMenu(back());
		const menu = await screen.findByRole('menu', { name: 'Folders behind' });
		const items = within(menu).getAllByRole('menuitem');
		expect(items.map((item) => item.textContent)).toEqual([
			'/home/test',
			'/home/test/docs',
			'/home/test',
		]);
		fireEvent.click(items[1]!);
		await waitFor(async () => expect(await currentUri(tabs)).toBe(DOCS.uri));
		await option('report.pdf');
		expect(screen.queryByRole('menu')).toBeNull();
	});

	it('opens on the menu key and on a long press, and not when there is nothing behind', async () => {
		await renderWorkspace();
		await option('docs');
		fireEvent.contextMenu(back());
		expect(screen.queryByRole('menu')).toBeNull();

		await openFolder('docs');
		await option('report.pdf');
		fireEvent.keyDown(back(), { key: 'ContextMenu' });
		expect(await screen.findByRole('menu', { name: 'Folders behind' })).toBeInTheDocument();
		fireEvent.keyDown(document.activeElement ?? document.body, { key: 'Escape' });
		await waitFor(() => expect(screen.queryByRole('menu')).toBeNull());

		vi.useFakeTimers();
		fireEvent.pointerDown(back(), { button: 0 });
		act(() => {
			vi.advanceTimersByTime(600);
		});
		vi.useRealTimers();
		expect(await screen.findByRole('menu', { name: 'Folders behind' })).toBeInTheDocument();
	});
});

describe('the path bar', () => {
	it('shows breadcrumbs that navigate', async () => {
		const { tabs } = await renderWorkspace();
		await option('docs');
		const crumbs = await screen.findByRole('navigation', { name: 'Location' });
		expect(
			within(crumbs)
				.getAllByRole('button')
				.map((b) => b.textContent),
		).toEqual(['/', 'home', 'test']);
		expect(within(crumbs).getByRole('button', { name: 'test' })).toHaveAttribute(
			'aria-current',
			'page',
		);
		fireEvent.click(within(crumbs).getByRole('button', { name: 'home' }));
		await option('test');
		expect(await currentUri(tabs)).toBe('file:///home');
	});

	it('edits as text on a click in the empty space and on Ctrl+L, and goes on Enter', async () => {
		const user = userEvent.setup();
		const { tabs } = await renderWorkspace();
		await option('docs');
		await screen.findByRole('navigation', { name: 'Location' });

		await user.click(screen.getByRole('button', { name: 'Edit location' }));
		const field = screen.getByRole('textbox', { name: /Type a location/ });
		expect(field).toHaveValue('/home/test');
		expect(field).toHaveFocus();

		await user.clear(field);
		await user.type(field, '~/../test/music{Enter}');
		await option('song.mp3');
		expect(await currentUri(tabs)).toBe(MUSIC.uri);
		expect(screen.queryByRole('textbox')).toBeNull();

		fireEvent.keyDown(window, { key: 'l', ctrlKey: true });
		expect(await screen.findByRole('textbox', { name: /Type a location/ })).toHaveFocus();
	});

	it('moves focus to the list after a typed path is accepted, so the arrow keys work at once', async () => {
		const user = userEvent.setup();
		await renderWorkspace();
		await option('docs');
		fireEvent.keyDown(window, { key: 'l', ctrlKey: true });
		const field = await screen.findByRole('textbox');
		await user.clear(field);
		await user.type(field, '~/../test/music{Enter}');
		await option('song.mp3');
		await waitFor(() => expect(screen.getByRole('listbox')).toHaveFocus());
	});

	it('cancels on Escape and on leaving the field, without navigating', async () => {
		const user = userEvent.setup();
		const { tabs } = await renderWorkspace();
		await option('docs');
		fireEvent.keyDown(window, { key: 'l', ctrlKey: true });
		const field = await screen.findByRole('textbox');
		await user.type(field, 'x{Escape}');
		expect(screen.queryByRole('textbox')).toBeNull();
		expect(await currentUri(tabs)).toBe(HOME.uri);

		fireEvent.keyDown(window, { key: 'g', ctrlKey: true, shiftKey: true });
		await screen.findByRole('textbox');
		fireEvent.blur(screen.getByRole('textbox'));
		await waitFor(() => expect(screen.queryByRole('textbox')).toBeNull());
	});

	it('shows an inline message for text that is not a location and stays in the field', async () => {
		const user = userEvent.setup();
		const { tabs } = await renderWorkspace();
		await option('docs');
		fireEvent.keyDown(window, { key: 'l', ctrlKey: true });
		const field = await screen.findByRole('textbox');
		await user.clear(field);
		await user.type(field, 'sftp://host/path{Enter}');
		const alert = await screen.findByRole('alert');
		expect(alert).toHaveTextContent('cannot open sftp locations');
		expect(field).toHaveAttribute('aria-invalid', 'true');
		expect(await currentUri(tabs)).toBe(HOME.uri);
		// Typing again clears the message.
		await user.type(field, 'x');
		expect(screen.queryByRole('alert')).toBeNull();
	});
});

describe('locations that cannot be opened', () => {
	it('shows the missing-folder state, keeps the history and lets Back return', async () => {
		const user = userEvent.setup();
		const { tabs } = await renderWorkspace();
		await openFolder('docs');
		await option('report.pdf');

		fireEvent.keyDown(window, { key: 'l', ctrlKey: true });
		const field = await screen.findByRole('textbox');
		await user.clear(field);
		await user.type(field, '/home/test/nowhere{Enter}');
		expect(await screen.findByRole('alert')).toHaveAttribute('data-error', 'notFound');
		expect(screen.queryByRole('listbox')).toBeNull();
		const snapshot = await tabs.getSnapshot();
		expect(snapshot.tabs[0]!.back.map((l) => l.display)).toEqual(['/home/test', '/home/test/docs']);

		fireEvent.click(back());
		await option('report.pdf');
	});

	it('shows permission denied and not-a-folder as their own states', async () => {
		const { client } = await renderWorkspace();
		const user = userEvent.setup();
		client.failOpening(MUSIC, { kind: 'permissionDenied', location: MUSIC });
		await openFolder('music');
		expect(await screen.findByRole('alert')).toHaveAttribute('data-error', 'permissionDenied');
		fireEvent.click(back());
		await option('music');

		const notes = { display: '/home/test/notes.txt', uri: 'file:///home/test/notes.txt' };
		client.failOpening(notes, { kind: 'notADirectory', location: notes });
		fireEvent.keyDown(window, { key: 'l', ctrlKey: true });
		const field = await screen.findByRole('textbox');
		await user.clear(field);
		await user.type(field, 'notes.txt{Enter}');
		expect(await screen.findByRole('alert')).toHaveAttribute('data-error', 'notADirectory');
	});
});
