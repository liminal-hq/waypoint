// Verifies the window menu renders an icon on every item
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { render, screen } from '@testing-library/react';
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
});
