// Verifies the placeholder screen renders the shared title bar and content
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
import { WindowChromeProvider } from '@liminal-hq/waypoint-chrome/WindowChromeProvider/WindowChromeProvider';
import { tauriWindowControls } from '@liminal-hq/waypoint-chrome/TitleBar/tauriWindowControls';
import { PlaceholderScreen } from './PlaceholderScreen';

vi.mock('@tauri-apps/api/window', () => ({
	getCurrentWindow: () => ({
		minimize: vi.fn(),
		toggleMaximize: vi.fn(),
		close: vi.fn(),
		startDragging: vi.fn(),
		setAlwaysOnTop: vi.fn(),
		isAlwaysOnTop: vi.fn().mockResolvedValue(false),
		isMaximized: vi.fn().mockResolvedValue(false),
		onResized: vi.fn().mockResolvedValue(() => {}),
		isFocused: vi.fn().mockResolvedValue(true),
		onFocusChanged: vi.fn().mockResolvedValue(() => {}),
	}),
}));

function renderScreen() {
	return render(
		<WindowChromeProvider controls={tauriWindowControls}>
			<PlaceholderScreen title="Main window" description="Browser goes here" />
		</WindowChromeProvider>,
	);
}

describe('PlaceholderScreen', () => {
	it('renders the description and the window controls', () => {
		renderScreen();
		expect(screen.getByText('Browser goes here')).toBeTruthy();
		expect(screen.getByRole('button', { name: /close/i })).toBeTruthy();
	});

	it('marks the static title text as a drag region', () => {
		renderScreen();
		const titles = screen.getAllByText('Main window');
		expect(titles.some((el) => el.hasAttribute('data-tauri-drag-region'))).toBe(true);
	});
});
