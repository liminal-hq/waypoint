// Verifies resizing the list's columns: the divider's drag, keys and reset, its accessible name and range, and that the width is kept once
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { createFakeSettingsClient, type FakeSettings } from '../services/fakeSettingsClient';
import { FakeVfsClient, makeEntry } from '../services/fakeVfsClient';
import { DEFAULT_SETTINGS, type ListColumnWidths } from '../services/settingsClient';
import { SettingsProvider } from '../settings/SettingsContext';
import { FOLDER, stubLayout } from '../test/browseHarness';
import { COLUMN_LIMITS, noWidths } from './columnWidths';
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

function mount(widths: Partial<ListColumnWidths> = {}) {
	const vfs = new FakeVfsClient();
	vfs.setFolder(FOLDER, [makeEntry(1, 'a.txt'), makeEntry(2, 'b.txt')]);
	const settings: FakeSettings = createFakeSettingsClient({
		...DEFAULT_SETTINGS,
		ui: { ...DEFAULT_SETTINGS.ui, columnWidths: { ...noWidths(), ...widths } },
	});
	render(
		<SettingsProvider client={settings}>
			<VfsClientProvider client={vfs}>
				<ListView location={FOLDER} />
			</VfsClientProvider>
		</SettingsProvider>,
	);
	return { vfs, settings };
}

const handle = (column: string) =>
	screen.getByRole('separator', { name: `Resize the ${column} column` });
const view = () => screen.getByRole('listbox').closest<HTMLElement>('[data-layout]')!;
const variable = (name: string) => view().style.getPropertyValue(name);
const lastWidths = (settings: FakeSettings) => settings.uiCalls.at(-1)?.columnWidths;

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
		const { settings } = await ready();
		const size = handle('Size');
		fireEvent.pointerDown(size, { ...pointer(), clientX: 500 });
		fireEvent.pointerMove(size, { ...pointer(), clientX: 470 });
		expect(variable('--wp-col-size')).toBe('118px');
		fireEvent.pointerMove(size, { ...pointer(), clientX: 460 });
		expect(variable('--wp-col-size')).toBe('128px');
		expect(settings.uiCalls).toEqual([]);

		fireEvent.pointerUp(size, { ...pointer(), clientX: 460 });
		await waitFor(() => expect(settings.uiCalls).toHaveLength(1));
		expect(lastWidths(settings)).toEqual({ ...noWidths(), size: 128 });
		await waitFor(() => expect(settings.current().settings.ui.columnWidths.size).toBe(128));
		expect(variable('--wp-col-size')).toBe('128px');
	});

	it('narrows the column as the pointer moves toward the end edge', async () => {
		const { settings } = await ready();
		const modified = handle('Modified');
		fireEvent.pointerDown(modified, { ...pointer(), clientX: 300 });
		fireEvent.pointerMove(modified, { ...pointer(), clientX: 340 });
		fireEvent.pointerUp(modified, { ...pointer(), clientX: 340 });
		await waitFor(() => expect(lastWidths(settings)).toEqual({ ...noWidths(), modified: 128 }));
	});

	it('keeps to the column’s minimum and maximum', async () => {
		const { settings } = await ready();
		const size = handle('Size');
		fireEvent.pointerDown(size, { ...pointer(), clientX: 500 });
		fireEvent.pointerMove(size, { ...pointer(), clientX: 900 });
		expect(variable('--wp-col-size')).toBe(`${COLUMN_LIMITS.size.min}px`);
		fireEvent.pointerMove(size, { ...pointer(), clientX: 0 });
		expect(variable('--wp-col-size')).toBe(`${COLUMN_LIMITS.size.max}px`);
		fireEvent.pointerUp(size, { ...pointer(), clientX: 0 });
		await waitFor(() => expect(lastWidths(settings)?.size).toBe(COLUMN_LIMITS.size.max));
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
			const { settings } = await ready();
			const size = handle('Size');
			fireEvent.pointerDown(size, { ...pointer(), clientX: 300 });
			fireEvent.pointerMove(size, { ...pointer(), clientX: 330 });
			expect(variable('--wp-col-size')).toBe('118px');
			fireEvent.pointerUp(size, { ...pointer(), clientX: 330 });
			await waitFor(() => expect(lastWidths(settings)?.size).toBe(118));
		} finally {
			spy.mockRestore();
		}
	});

	it('is dropped by Escape, keeping nothing', async () => {
		const { settings } = await ready();
		const size = handle('Size');
		fireEvent.pointerDown(size, { ...pointer(), clientX: 500 });
		fireEvent.pointerMove(size, { ...pointer(), clientX: 450 });
		expect(variable('--wp-col-size')).toBe('138px');
		fireEvent.keyDown(window, { key: 'Escape' });
		expect(variable('--wp-col-size')).toBe('');
		fireEvent.pointerUp(size, { ...pointer(), clientX: 450 });
		expect(settings.uiCalls).toEqual([]);
	});

	it('is dropped when the pointer is cancelled', async () => {
		const { settings } = await ready();
		const size = handle('Size');
		fireEvent.pointerDown(size, { ...pointer(), clientX: 500 });
		fireEvent.pointerMove(size, { ...pointer(), clientX: 450 });
		fireEvent.pointerCancel(size, pointer());
		expect(variable('--wp-col-size')).toBe('');
		expect(settings.uiCalls).toEqual([]);
	});

	it('keeps nothing for a press with no movement, and starts no sort', async () => {
		const { vfs, settings } = await ready();
		const setSort = vi.spyOn(vfs, 'setSort');
		const size = handle('Size');
		fireEvent.pointerDown(size, { ...pointer(), clientX: 500 });
		fireEvent.pointerUp(size, { ...pointer(), clientX: 500 });
		fireEvent.click(size);
		expect(settings.uiCalls).toEqual([]);
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
		const { settings } = await ready();
		const size = handle('Size');
		fireEvent.pointerDown(size, { pointerId: 1, button: 2, clientX: 500 });
		fireEvent.pointerMove(size, { pointerId: 1, button: 2, clientX: 450 });
		expect(variable('--wp-col-size')).toBe('');
		expect(settings.uiCalls).toEqual([]);
	});
});

describe('the keys on a divider', () => {
	it('widen toward the start edge and narrow toward the end, and keep the width once the keys rest', async () => {
		const { settings } = await ready();
		const size = handle('Size');
		act(() => size.focus());
		fireEvent.keyDown(size, { key: 'ArrowLeft' });
		fireEvent.keyDown(size, { key: 'ArrowLeft' });
		fireEvent.keyDown(size, { key: 'ArrowLeft', shiftKey: true });
		expect(variable('--wp-col-size')).toBe('136px');
		expect(size).toHaveAttribute('aria-valuenow', '136');
		fireEvent.keyDown(size, { key: 'ArrowRight' });
		expect(variable('--wp-col-size')).toBe('128px');
		expect(settings.uiCalls).toEqual([]);

		await waitFor(() => expect(settings.uiCalls).toHaveLength(1), { timeout: 2000 });
		expect(lastWidths(settings)).toEqual({ ...noWidths(), size: 128 });
	});

	it('keep the width at once when the divider loses the focus', async () => {
		const { settings } = await ready();
		const size = handle('Size');
		act(() => size.focus());
		fireEvent.keyDown(size, { key: 'ArrowLeft' });
		expect(settings.uiCalls).toEqual([]);
		fireEvent.blur(size);
		await waitFor(() => expect(lastWidths(settings)).toEqual({ ...noWidths(), size: 96 }));
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
		const { settings } = mount({ size: 150 });
		await screen.findByRole('listbox');
		await waitFor(() => expect(variable('--wp-col-size')).toBe('150px'));
		fireEvent.keyDown(handle('Size'), { key: 'Backspace' });
		await waitFor(() => expect(lastWidths(settings)).toEqual(noWidths()));
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
		const { settings } = mount({ size: 150, kind: 130 });
		await screen.findByRole('listbox');
		await waitFor(() => expect(variable('--wp-col-size')).toBe('150px'));
		fireEvent.doubleClick(handle('Size'));
		await waitFor(() => expect(lastWidths(settings)).toEqual({ ...noWidths(), kind: 130 }));
		await waitFor(() => expect(variable('--wp-col-size')).toBe(''));
		expect(variable('--wp-col-kind')).toBe('130px');
	});

	it('is offered by the header menu once a column is resized, and puts every column back', async () => {
		const { settings } = mount({ size: 150, kind: 130 });
		await screen.findByRole('listbox');
		await waitFor(() => expect(variable('--wp-col-size')).toBe('150px'));
		fireEvent.contextMenu(screen.getByRole('group', { name: 'Sort the list' }));
		const item = await screen.findByRole('menuitem', { name: 'Reset Column Widths' });
		expect(item).not.toHaveAttribute('aria-disabled', 'true');
		await userEvent.click(item);
		await waitFor(() => expect(lastWidths(settings)).toEqual(noWidths()));
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
		const { settings } = await ready();
		expect(variable('--wp-col-size')).toBe('');
		const current = settings.current().settings;
		act(() => {
			settings.change({
				...current,
				ui: { ...current.ui, columnWidths: { ...noWidths(), modified: 200 } },
			});
		});
		await waitFor(() => expect(variable('--wp-col-modified')).toBe('200px'));
	});
});
