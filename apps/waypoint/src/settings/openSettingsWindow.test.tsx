// Verifies the Settings window opens through its command, reports a failure, and Ctrl+, is its shortcut
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { cleanup, fireEvent, render } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

const invoke = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({ invoke: (...args: unknown[]) => invoke(...args) }));
const showNotice = vi.fn();
vi.mock('../app/notices', () => ({ showNotice: (...args: unknown[]) => showNotice(...args) }));

import { isSettingsShortcut, openSettingsWindow, useSettingsShortcut } from './openSettingsWindow';

afterEach(() => {
	cleanup();
	invoke.mockReset();
	showNotice.mockReset();
});

function Host({ open }: { open: () => void }) {
	useSettingsShortcut(open);
	return <input aria-label="field" />;
}

describe('openSettingsWindow', () => {
	it('can ask for one page, such as Experimental', () => {
		invoke.mockResolvedValue(undefined);
		openSettingsWindow('experimental');
		expect(invoke).toHaveBeenCalledWith('open_settings_window', { section: 'experimental' });
	});

	it('asks the app to open the single Settings window', () => {
		invoke.mockResolvedValue(undefined);
		openSettingsWindow();
		expect(invoke).toHaveBeenCalledWith('open_settings_window');
	});

	it('tells the person when it cannot', async () => {
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		invoke.mockRejectedValue(new Error('no window'));
		openSettingsWindow();
		await vi.waitFor(() =>
			expect(showNotice).toHaveBeenCalledWith('Could not open the Settings window.'),
		);
		warn.mockRestore();
	});
});

describe('the Settings shortcut', () => {
	const key = (init: Partial<KeyboardEventInit>) => ({
		key: ',',
		ctrlKey: false,
		metaKey: false,
		altKey: false,
		shiftKey: false,
		isComposing: false,
		...init,
	});

	it('is Ctrl+, or Cmd+, and nothing else', () => {
		expect(isSettingsShortcut(key({ ctrlKey: true }))).toBe(true);
		expect(isSettingsShortcut(key({ metaKey: true }))).toBe(true);
		expect(isSettingsShortcut(key({}))).toBe(false);
		expect(isSettingsShortcut(key({ ctrlKey: true, shiftKey: true }))).toBe(false);
		expect(isSettingsShortcut(key({ ctrlKey: true, altKey: true }))).toBe(false);
		expect(isSettingsShortcut(key({ ctrlKey: true, key: '.' }))).toBe(false);
		expect(isSettingsShortcut(key({ ctrlKey: true, isComposing: true }))).toBe(false);
	});

	it('opens the window from anywhere on the page, including a text field, and stops when unmounted', () => {
		const open = vi.fn();
		const { getByLabelText, unmount } = render(<Host open={open} />);
		fireEvent.keyDown(getByLabelText('field'), { key: ',', ctrlKey: true });
		expect(open).toHaveBeenCalledTimes(1);
		fireEvent.keyDown(document.body, { key: ',' });
		expect(open).toHaveBeenCalledTimes(1);
		unmount();
		fireEvent.keyDown(document.body, { key: ',', ctrlKey: true });
		expect(open).toHaveBeenCalledTimes(1);
	});

	it('leaves a key that something else already handled alone', () => {
		const open = vi.fn();
		render(<Host open={open} />);
		const event = new KeyboardEvent('keydown', { key: ',', ctrlKey: true, cancelable: true });
		event.preventDefault();
		window.dispatchEvent(event);
		expect(open).not.toHaveBeenCalled();
	});
});
