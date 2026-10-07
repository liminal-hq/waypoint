// Verifies resizing the list's columns: the divider's drag, keys and reset, its accessible name and range, and that the width is kept once for the folder
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
	createFakeFolderViewsClient,
	type FakeFolderViews,
} from '../services/fakeFolderViewsClient';
import { createFakeSettingsClient } from '../services/fakeSettingsClient';
import { FakeVfsClient, fileLocation, makeEntry } from '../services/fakeVfsClient';
import type { ListColumnWidths } from '../services/folderViewsClient';
import { DEFAULT_SETTINGS } from '../services/settingsClient';
import { SettingsProvider } from '../settings/SettingsContext';
import { FOLDER, stubLayout } from '../test/browseHarness';
import { COLUMN_LIMITS, noWidths } from './columnWidths';
import { FolderViewsProvider } from './FolderViewsContext';
import { ListView } from './ListView';
import { VfsClientProvider } from './VfsClientContext';

const NAME_WIDTH = 400;
const OWN_WIDTHS: Record<string, number> = { size: 88, modified: 168, kind: 96 };

let restoreLayout: () => void;
let rectSpy: { mockRestore(): void };
let nameWidth = NAME_WIDTH;
beforeEach(() => {
	restoreLayout = stubLayout(280, 900);
	nameWidth = NAME_WIDTH;
	// happy-dom lays nothing out: a header cell is as wide as its column variable says, and Name has a set width.
	rectSpy = vi.spyOn(Element.prototype, 'getBoundingClientRect').mockImplementation(function (
		this: Element,
	) {
		const column = this.getAttribute('data-column');
		let width = 0;
		if (column === 'name') width = nameWidth;
		else if (column && this.parentElement?.getAttribute('role') === 'group') {
			const view = this.closest<HTMLElement>('[data-layout]');
			const set = view?.style.getPropertyValue(`--wp-col-${column}`);
			width = set ? Number.parseFloat(set) : (OWN_WIDTHS[column] ?? 100);
		}
		return { x: 0, y: 0, top: 0, left: 0, right: width, bottom: 28, width, height: 28 } as DOMRect;
	});
});
afterEach(() => {
	cleanup();
	rectSpy.mockRestore();
	restoreLayout();
	document.documentElement.removeAttribute('dir');
	vi.useRealTimers();
});

const OTHER = fileLocation('/home/other');

function mount(widths: Partial<ListColumnWidths> = {}, elsewhere?: Partial<ListColumnWidths>) {
	const vfs = new FakeVfsClient();
	vfs.setFolder(FOLDER, [makeEntry(1, 'a.txt'), makeEntry(2, 'b.txt')]);
	vfs.setFolder(OTHER, [makeEntry(3, 'c.txt'), makeEntry(4, 'd.txt')]);
	const seed = (set: Partial<ListColumnWidths>) =>
		Object.keys(set).length > 0 ? { columnWidths: { ...noWidths(), ...set } } : {};
	const folderViews = createFakeFolderViewsClient({
		...(Object.keys(widths).length > 0 ? { [FOLDER.uri]: seed(widths) } : {}),
		...(elsewhere ? { [OTHER.uri]: seed(elsewhere) } : {}),
	});
	const tree = (location: typeof FOLDER) => (
		<FolderViewsProvider client={folderViews}>
			<VfsClientProvider client={vfs}>
				<ListView location={location} />
			</VfsClientProvider>
		</FolderViewsProvider>
	);
	const { rerender } = render(tree(FOLDER));
	return { vfs, folderViews, goTo: (location: typeof FOLDER) => rerender(tree(location)) };
}

const handle = (column: string) =>
	screen.getByRole('separator', { name: `Resize the ${column} column` });
const view = () => screen.getByRole('listbox').closest<HTMLElement>('[data-layout]')!;
const variable = (name: string) => view().style.getPropertyValue(name);
const lastWidths = (folderViews: FakeFolderViews) =>
	folderViews.remembered.at(-1)?.patch.columnWidths;
const widthsOf = (folderViews: FakeFolderViews, uri: string) =>
	folderViews.view(uri)?.columnWidths ?? null;

async function ready() {
	const mounted = mount();
	await screen.findByRole('listbox');
	await waitFor(() => expect(screen.getAllByRole('option')).toHaveLength(2));
	return mounted;
}

const pointer = (pointerId = 1) => ({ pointerId, button: 0 });

describe('the dividers', () => {
	it('are separators named for their column, on every column but Name', async () => {
		await ready();
		const separators = screen.getAllByRole('separator');
		expect(separators.map((separator) => separator.getAttribute('aria-label'))).toEqual([
			'Resize the Size column',
			'Resize the Modified column',
			'Resize the Kind column',
		]);
		const size = handle('Size');
		expect(size).toHaveAttribute('aria-orientation', 'vertical');
		expect(size).toHaveAttribute('tabindex', '0');
		expect(size).toHaveAttribute('aria-valuenow', '88');
		expect(size).toHaveAttribute('aria-valuemin', String(COLUMN_LIMITS.size.min));
		expect(size).toHaveAttribute('aria-valuemax', String(COLUMN_LIMITS.size.max));
		expect(size).toHaveAttribute('aria-valuetext', '88 pixels wide');
	});

	it('sit beside the sort button, not inside it', async () => {
		await ready();
		expect(handle('Size').closest('button')).toBeNull();
		expect(screen.getByRole('button', { name: /^Size/ })).not.toContainElement(handle('Size'));
	});

	it('apply a remembered width to the header and every row through one variable', async () => {
		mount({ size: 140 });
		await screen.findByRole('listbox');
		await waitFor(() => expect(variable('--wp-col-size')).toBe('140px'));
		expect(variable('--wp-col-modified')).toBe('');
		await waitFor(() => expect(handle('Size')).toHaveAttribute('aria-valuenow', '140'));
	});

	it('bring a stored width to the column’s limits', async () => {
		mount({ size: COLUMN_LIMITS.size.max + 400 });
		await screen.findByRole('listbox');
		await waitFor(() => expect(variable('--wp-col-size')).toBe(`${COLUMN_LIMITS.size.max}px`));
	});
});

describe('dragging a divider', () => {
	it('shows the width as the pointer moves toward the start edge, and keeps it once, on release', async () => {
		const { folderViews } = await ready();
		const size = handle('Size');
		fireEvent.pointerDown(size, { ...pointer(), clientX: 500 });
		fireEvent.pointerMove(size, { ...pointer(), clientX: 470 });
		expect(variable('--wp-col-size')).toBe('118px');
		fireEvent.pointerMove(size, { ...pointer(), clientX: 460 });
		expect(variable('--wp-col-size')).toBe('128px');
		expect(folderViews.remembered).toEqual([]);

		fireEvent.pointerUp(size, { ...pointer(), clientX: 460 });
		await waitFor(() => expect(folderViews.remembered).toHaveLength(1));
		expect(lastWidths(folderViews)).toEqual({ ...noWidths(), size: 128 });
		expect(folderViews.remembered[0]?.key).toBe(FOLDER.uri);
		await waitFor(() => expect(widthsOf(folderViews, FOLDER.uri)?.size).toBe(128));
		expect(variable('--wp-col-size')).toBe('128px');
	});

	it('narrows the column as the pointer moves toward the end edge', async () => {
		const { folderViews } = await ready();
		const modified = handle('Modified');
		fireEvent.pointerDown(modified, { ...pointer(), clientX: 300 });
		fireEvent.pointerMove(modified, { ...pointer(), clientX: 340 });
		fireEvent.pointerUp(modified, { ...pointer(), clientX: 340 });
		await waitFor(() => expect(lastWidths(folderViews)).toEqual({ ...noWidths(), modified: 128 }));
	});

	it('keeps to the column’s minimum and maximum', async () => {
		const { folderViews } = await ready();
		const size = handle('Size');
		fireEvent.pointerDown(size, { ...pointer(), clientX: 500 });
		fireEvent.pointerMove(size, { ...pointer(), clientX: 900 });
		expect(variable('--wp-col-size')).toBe(`${COLUMN_LIMITS.size.min}px`);
		fireEvent.pointerMove(size, { ...pointer(), clientX: 0 });
		expect(variable('--wp-col-size')).toBe(`${COLUMN_LIMITS.size.max}px`);
		fireEvent.pointerUp(size, { ...pointer(), clientX: 0 });
		await waitFor(() => expect(lastWidths(folderViews)?.size).toBe(COLUMN_LIMITS.size.max));
	});

	it('stops before Name would be squeezed below its minimum', async () => {
		nameWidth = 130;
		await ready();
		const size = handle('Size');
		fireEvent.pointerDown(size, { ...pointer(), clientX: 500 });
		// Name has 10 pixels to give above its 120.
		fireEvent.pointerMove(size, { ...pointer(), clientX: 100 });
		expect(variable('--wp-col-size')).toBe('98px');
	});

	it('mirrors in a right-to-left view: the divider is on the right and moving right widens', async () => {
		document.documentElement.setAttribute('dir', 'rtl');
		const style = window.getComputedStyle.bind(window);
		const spy = vi.spyOn(window, 'getComputedStyle').mockImplementation((element: Element) => {
			const computed = style(element);
			return new Proxy(computed, {
				get: (target, property) =>
					property === 'direction' ? 'rtl' : Reflect.get(target, property, target),
			});
		});
		try {
			const { folderViews } = await ready();
			const size = handle('Size');
			fireEvent.pointerDown(size, { ...pointer(), clientX: 300 });
			fireEvent.pointerMove(size, { ...pointer(), clientX: 330 });
			expect(variable('--wp-col-size')).toBe('118px');
			fireEvent.pointerUp(size, { ...pointer(), clientX: 330 });
			await waitFor(() => expect(lastWidths(folderViews)?.size).toBe(118));
		} finally {
			spy.mockRestore();
		}
	});

	it('is dropped by Escape, keeping nothing', async () => {
		const { folderViews } = await ready();
		const size = handle('Size');
		fireEvent.pointerDown(size, { ...pointer(), clientX: 500 });
		fireEvent.pointerMove(size, { ...pointer(), clientX: 450 });
		expect(variable('--wp-col-size')).toBe('138px');
		fireEvent.keyDown(window, { key: 'Escape' });
		expect(variable('--wp-col-size')).toBe('');
		fireEvent.pointerUp(size, { ...pointer(), clientX: 450 });
		expect(folderViews.remembered).toEqual([]);
	});

	it('is dropped when the pointer is cancelled', async () => {
		const { folderViews } = await ready();
		const size = handle('Size');
		fireEvent.pointerDown(size, { ...pointer(), clientX: 500 });
		fireEvent.pointerMove(size, { ...pointer(), clientX: 450 });
		fireEvent.pointerCancel(size, pointer());
		expect(variable('--wp-col-size')).toBe('');
		expect(folderViews.remembered).toEqual([]);
	});

	it('keeps nothing for a press with no movement, and starts no sort', async () => {
		const { vfs, folderViews } = await ready();
		const setSort = vi.spyOn(vfs, 'setSort');
		const size = handle('Size');
		fireEvent.pointerDown(size, { ...pointer(), clientX: 500 });
		fireEvent.pointerUp(size, { ...pointer(), clientX: 500 });
		fireEvent.click(size);
		expect(folderViews.remembered).toEqual([]);
		expect(setSort).not.toHaveBeenCalled();
	});

	it('ends a drag without sorting, and the label still sorts', async () => {
		const { vfs } = await ready();
		const setSort = vi.spyOn(vfs, 'setSort');
		const size = handle('Size');
		fireEvent.pointerDown(size, { ...pointer(), clientX: 500 });
		fireEvent.pointerMove(size, { ...pointer(), clientX: 450 });
		fireEvent.pointerUp(size, { ...pointer(), clientX: 450 });
		fireEvent.click(size);
		expect(setSort).not.toHaveBeenCalled();
		fireEvent.click(screen.getByRole('button', { name: /^Size/ }));
		await waitFor(() => expect(setSort).toHaveBeenCalledTimes(1));
	});

	it('ignores a press with another button', async () => {
		const { folderViews } = await ready();
		const size = handle('Size');
		fireEvent.pointerDown(size, { pointerId: 1, button: 2, clientX: 500 });
		fireEvent.pointerMove(size, { pointerId: 1, button: 2, clientX: 450 });
		expect(variable('--wp-col-size')).toBe('');
		expect(folderViews.remembered).toEqual([]);
	});
});

describe('the keys on a divider', () => {
	it('widen toward the start edge and narrow toward the end, and keep the width once the keys rest', async () => {
		const { folderViews } = await ready();
		const size = handle('Size');
		act(() => size.focus());
		fireEvent.keyDown(size, { key: 'ArrowLeft' });
		fireEvent.keyDown(size, { key: 'ArrowLeft' });
		fireEvent.keyDown(size, { key: 'ArrowLeft', shiftKey: true });
		expect(variable('--wp-col-size')).toBe('136px');
		expect(size).toHaveAttribute('aria-valuenow', '136');
		fireEvent.keyDown(size, { key: 'ArrowRight' });
		expect(variable('--wp-col-size')).toBe('128px');
		expect(folderViews.remembered).toEqual([]);

		await waitFor(() => expect(folderViews.remembered).toHaveLength(1), { timeout: 2000 });
		expect(lastWidths(folderViews)).toEqual({ ...noWidths(), size: 128 });
	});

	it('keep the width at once when the divider loses the focus', async () => {
		const { folderViews } = await ready();
		const size = handle('Size');
		act(() => size.focus());
		fireEvent.keyDown(size, { key: 'ArrowLeft' });
		expect(folderViews.remembered).toEqual([]);
		fireEvent.blur(size);
		await waitFor(() => expect(lastWidths(folderViews)).toEqual({ ...noWidths(), size: 96 }));
	});

	it('go to the narrowest and widest with Home and End', async () => {
		await ready();
		const size = handle('Size');
		fireEvent.keyDown(size, { key: 'Home' });
		expect(variable('--wp-col-size')).toBe(`${COLUMN_LIMITS.size.min}px`);
		fireEvent.keyDown(size, { key: 'End' });
		expect(variable('--wp-col-size')).toBe(`${COLUMN_LIMITS.size.max}px`);
	});

	it('restore the column’s own width with Backspace', async () => {
		const { folderViews } = mount({ size: 150 });
		await screen.findByRole('listbox');
		await waitFor(() => expect(variable('--wp-col-size')).toBe('150px'));
		fireEvent.keyDown(handle('Size'), { key: 'Backspace' });
		await waitFor(() => expect(lastWidths(folderViews)).toEqual(noWidths()));
		await waitFor(() => expect(variable('--wp-col-size')).toBe(''));
	});

	it('leave other keys, such as Tab and the arrows up and down, alone', async () => {
		await ready();
		const size = handle('Size');
		expect(fireEvent.keyDown(size, { key: 'Tab' })).toBe(true);
		expect(fireEvent.keyDown(size, { key: 'ArrowUp' })).toBe(true);
		expect(variable('--wp-col-size')).toBe('');
	});
});

describe('resetting', () => {
	it('puts a column back to its own width on a double-click', async () => {
		const { folderViews } = mount({ size: 150, kind: 130 });
		await screen.findByRole('listbox');
		await waitFor(() => expect(variable('--wp-col-size')).toBe('150px'));
		fireEvent.doubleClick(handle('Size'));
		await waitFor(() => expect(lastWidths(folderViews)).toEqual({ ...noWidths(), kind: 130 }));
		await waitFor(() => expect(variable('--wp-col-size')).toBe(''));
		expect(variable('--wp-col-kind')).toBe('130px');
	});

	it('is offered by the header menu once a column is resized, and puts every column back', async () => {
		const { folderViews } = mount({ size: 150, kind: 130 });
		await screen.findByRole('listbox');
		await waitFor(() => expect(variable('--wp-col-size')).toBe('150px'));
		fireEvent.contextMenu(screen.getByRole('group', { name: 'Sort the list' }));
		const item = await screen.findByRole('menuitem', { name: 'Reset Column Widths' });
		expect(item).not.toHaveAttribute('aria-disabled', 'true');
		await userEvent.click(item);
		await waitFor(() => expect(lastWidths(folderViews)).toEqual(noWidths()));
		await waitFor(() => expect(variable('--wp-col-kind')).toBe(''));
	});

	it('is disabled in the header menu while every column has its own width', async () => {
		await ready();
		fireEvent.contextMenu(screen.getByRole('group', { name: 'Sort the list' }));
		const item = await screen.findByRole('menuitem', { name: 'Reset Column Widths' });
		expect(item).toHaveAttribute('aria-disabled', 'true');
	});
});

describe('following another window', () => {
	it('shows a width another window kept, at once', async () => {
		const { folderViews } = await ready();
		expect(variable('--wp-col-size')).toBe('');
		act(() => {
			folderViews.change(FOLDER.uri, { columnWidths: { ...noWidths(), modified: 200 } });
		});
		await waitFor(() => expect(variable('--wp-col-modified')).toBe('200px'));
	});

	it('ignores a width another window kept for another folder', async () => {
		const { folderViews } = await ready();
		act(() => {
			folderViews.change(OTHER.uri, { columnWidths: { ...noWidths(), modified: 200 } });
		});
		await act(async () => {});
		expect(variable('--wp-col-modified')).toBe('');
	});

	it('goes back to the columns’ own widths when another window resets the folder', async () => {
		const { folderViews } = mount({ size: 150 });
		await waitFor(() => expect(variable('--wp-col-size')).toBe('150px'));
		await act(async () => {
			await folderViews.reset(FOLDER.uri);
		});
		await waitFor(() => expect(variable('--wp-col-size')).toBe(''));
	});
});

describe('each folder has its own widths', () => {
	it('keeps a width for the folder it was resized in and not for another', async () => {
		const { folderViews, goTo } = await ready();
		fireEvent.keyDown(handle('Size'), { key: 'End' });
		fireEvent.blur(handle('Size'));
		await waitFor(() => expect(widthsOf(folderViews, FOLDER.uri)?.size).toBe(240));
		expect(folderViews.view(OTHER.uri)).toBeUndefined();

		goTo(OTHER);
		await waitFor(() => expect(screen.getAllByRole('option')[0]).toHaveTextContent('c.txt'));
		expect(variable('--wp-col-size')).toBe('');
		expect(handle('Size')).toHaveAttribute('aria-valuenow', '88');
	});

	it('shows each folder’s own widths when moving between two that were resized', async () => {
		const { goTo } = mount({ size: 140 }, { size: 200, kind: 130 });
		await waitFor(() => expect(variable('--wp-col-size')).toBe('140px'));
		expect(variable('--wp-col-kind')).toBe('');

		goTo(OTHER);
		await waitFor(() => expect(screen.getAllByRole('option')[0]).toHaveTextContent('c.txt'));
		await waitFor(() => expect(variable('--wp-col-size')).toBe('200px'));
		expect(variable('--wp-col-kind')).toBe('130px');

		goTo(FOLDER);
		await waitFor(() => expect(screen.getAllByRole('option')[0]).toHaveTextContent('a.txt'));
		await waitFor(() => expect(variable('--wp-col-size')).toBe('140px'));
		expect(variable('--wp-col-kind')).toBe('');
	});

	it('keeps a width the keys had not yet kept when the folder changes, for the folder it was set in', async () => {
		const { folderViews, goTo } = await ready();
		const size = handle('Size');
		act(() => size.focus());
		fireEvent.keyDown(size, { key: 'ArrowLeft' });
		expect(folderViews.remembered).toEqual([]);
		goTo(OTHER);
		await waitFor(() => expect(folderViews.remembered).toHaveLength(1));
		expect(folderViews.remembered[0]).toMatchObject({ key: FOLDER.uri });
		expect(widthsOf(folderViews, FOLDER.uri)?.size).toBe(96);
		expect(folderViews.view(OTHER.uri)).toBeUndefined();
	});

	it('resets only the folder it is shown in', async () => {
		const { folderViews } = mount({ size: 150 }, { size: 200 });
		await waitFor(() => expect(variable('--wp-col-size')).toBe('150px'));
		fireEvent.contextMenu(screen.getByRole('group', { name: 'Sort the list' }));
		await userEvent.click(await screen.findByRole('menuitem', { name: 'Reset Column Widths' }));
		await waitFor(() => expect(variable('--wp-col-size')).toBe(''));
		expect(folderViews.remembered.at(-1)?.key).toBe(FOLDER.uri);
		expect(folderViews.view(FOLDER.uri)).toBeUndefined();
		expect(widthsOf(folderViews, OTHER.uri)?.size).toBe(200);
	});

	it('falls back to the defaults once a folder’s last width is cleared, and drops the record', async () => {
		const { folderViews } = mount({ size: 150 });
		await waitFor(() => expect(variable('--wp-col-size')).toBe('150px'));
		fireEvent.doubleClick(handle('Size'));
		await waitFor(() => expect(variable('--wp-col-size')).toBe(''));
		expect(lastWidths(folderViews)).toEqual(noWidths());
		expect(folderViews.view(FOLDER.uri)).toBeUndefined();
		expect(handle('Size')).toHaveAttribute('aria-valuenow', '88');
	});

	it('leaves the folder’s other remembered choices when its widths are cleared', async () => {
		const folderViews = createFakeFolderViewsClient({
			[FOLDER.uri]: { mode: 'list', columnWidths: { ...noWidths(), size: 150 } },
		});
		const vfs = new FakeVfsClient();
		vfs.setFolder(FOLDER, [makeEntry(1, 'a.txt')]);
		render(
			<FolderViewsProvider client={folderViews}>
				<VfsClientProvider client={vfs}>
					<ListView location={FOLDER} />
				</VfsClientProvider>
			</FolderViewsProvider>,
		);
		await waitFor(() => expect(variable('--wp-col-size')).toBe('150px'));
		fireEvent.keyDown(handle('Size'), { key: 'Delete' });
		await waitFor(() => expect(variable('--wp-col-size')).toBe(''));
		expect(folderViews.view(FOLDER.uri)).toMatchObject({ mode: 'list', columnWidths: null });
	});
});

describe('where folders do not remember', () => {
	it('shows a width for the listing it was set in, and keeps nothing, when remembering is off', async () => {
		const folderViews = createFakeFolderViewsClient();
		const settings = createFakeSettingsClient({
			...DEFAULT_SETTINGS,
			general: { ...DEFAULT_SETTINGS.general, rememberFolderViews: false },
		});
		const vfs = new FakeVfsClient();
		vfs.setFolder(FOLDER, [makeEntry(1, 'a.txt')]);
		render(
			<SettingsProvider client={settings}>
				<FolderViewsProvider client={folderViews}>
					<VfsClientProvider client={vfs}>
						<ListView location={FOLDER} />
					</VfsClientProvider>
				</FolderViewsProvider>
			</SettingsProvider>,
		);
		await waitFor(() => expect(screen.getAllByRole('option')).toHaveLength(1));
		fireEvent.keyDown(handle('Size'), { key: 'End' });
		fireEvent.blur(handle('Size'));
		expect(variable('--wp-col-size')).toBe(`${COLUMN_LIMITS.size.max}px`);
		await act(async () => {});
		expect(folderViews.remembered).toEqual([]);
		expect(variable('--wp-col-size')).toBe(`${COLUMN_LIMITS.size.max}px`);
	});

	it('shows a width for the listing it was set in when there is no service', async () => {
		const vfs = new FakeVfsClient();
		vfs.setFolder(FOLDER, [makeEntry(1, 'a.txt')]);
		render(
			<VfsClientProvider client={vfs}>
				<ListView location={FOLDER} />
			</VfsClientProvider>,
		);
		await waitFor(() => expect(screen.getAllByRole('option')).toHaveLength(1));
		fireEvent.keyDown(handle('Size'), { key: 'End' });
		fireEvent.blur(handle('Size'));
		expect(variable('--wp-col-size')).toBe(`${COLUMN_LIMITS.size.max}px`);
	});

	it('shows the default width again when keeping the width fails', async () => {
		const { folderViews } = await ready();
		folderViews.failNext();
		fireEvent.keyDown(handle('Size'), { key: 'End' });
		fireEvent.blur(handle('Size'));
		await waitFor(() => expect(folderViews.remembered).toHaveLength(1));
		await waitFor(() => expect(variable('--wp-col-size')).toBe(''));
	});
});
