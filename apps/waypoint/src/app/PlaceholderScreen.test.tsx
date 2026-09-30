// Verifies the placeholder screen renders the shared title bar and content
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { render, screen } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
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
	}),
}));

describe('PlaceholderScreen', () => {
	it('renders the description and the window controls', () => {
		render(<PlaceholderScreen title="Main window" description="Browser goes here" />);
		expect(screen.getByText('Browser goes here')).toBeTruthy();
		expect(screen.getByRole('button', { name: /close/i })).toBeTruthy();
	});
});
