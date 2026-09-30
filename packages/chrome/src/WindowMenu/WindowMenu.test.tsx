// Verifies the window menu renders an icon on every item
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { WindowControls } from '../TitleBar/windowControls';
import { WindowChromeProvider } from '../WindowChromeProvider/WindowChromeProvider';
import { WindowMenu } from './WindowMenu';

const controls: WindowControls = {
	minimize: vi.fn(),
	toggleMaximize: vi.fn(),
	close: vi.fn(),
	startDragging: vi.fn(),
	setAlwaysOnTop: vi.fn(),
	isMaximized: () => false,
	onMaximizedChange: () => () => {},
};

describe('WindowMenu', () => {
	it('shows an icon beside every item', () => {
		render(
			<WindowChromeProvider controls={controls}>
				<WindowMenu
					position={{ x: 10, y: 10 }}
					alwaysOnTop={false}
					onAlwaysOnTopChange={() => {}}
					onClose={() => {}}
				/>
			</WindowChromeProvider>,
		);
		const items = [...screen.getAllByRole('menuitem'), ...screen.getAllByRole('menuitemcheckbox')];
		expect(items.length).toBe(5);
		for (const item of items) {
			expect(item.querySelector('svg')).not.toBeNull();
		}
	});

	it('offers More options only when the host can show the compositor menu', () => {
		const withoutHost = render(
			<WindowChromeProvider controls={controls}>
				<WindowMenu
					position={{ x: 10, y: 10 }}
					alwaysOnTop={false}
					onAlwaysOnTopChange={() => {}}
					onClose={() => {}}
				/>
			</WindowChromeProvider>,
		);
		expect(screen.queryByRole('menuitem', { name: 'More options…' })).toBeNull();
		withoutHost.unmount();
	});

	it('asks the host for the compositor menu at the menu position', async () => {
		const showSystemMenu = vi.fn().mockResolvedValue(true);
		render(
			<WindowChromeProvider controls={{ ...controls, showSystemMenu }}>
				<WindowMenu
					position={{ x: 120, y: 48 }}
					alwaysOnTop={false}
					onAlwaysOnTopChange={() => {}}
					onClose={() => {}}
				/>
			</WindowChromeProvider>,
		);
		await userEvent.click(screen.getByRole('menuitem', { name: 'More options…' }));
		expect(showSystemMenu).toHaveBeenCalledWith({ x: 120, y: 48 });
	});
});
