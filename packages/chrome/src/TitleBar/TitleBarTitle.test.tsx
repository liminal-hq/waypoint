// Tests that the title text is a drag region carrying its children, or following the window title
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { render, screen } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { WindowChromeProvider } from '../WindowChromeProvider/WindowChromeProvider';
import { useWindowTitle } from '../WindowTitle/useWindowTitle';
import type { WindowControls } from './windowControls';
import { TitleBarTitle } from './TitleBarTitle';

describe('TitleBarTitle', () => {
	it('renders its children in a drag region', () => {
		render(<TitleBarTitle className="extra">Documents</TitleBarTitle>);
		const title = screen.getByText('Documents');
		expect(title.tagName).toBe('SPAN');
		expect(title).toHaveAttribute('data-tauri-drag-region');
		expect(title).toHaveClass('extra');
	});
});

function Setter({ title }: { title: string }) {
	useWindowTitle(title);
	return null;
}

const controls: WindowControls = {
	minimize: () => {},
	toggleMaximize: () => {},
	close: () => {},
	setAlwaysOnTop: () => {},
	isMaximized: () => false,
	onMaximizedChange: () => () => {},
};

describe('TitleBarTitle following the window title', () => {
	it('shows the fallback until a title is set, then the title', () => {
		const { rerender } = render(
			<WindowChromeProvider controls={controls}>
				<TitleBarTitle fallback="Fallback" />
			</WindowChromeProvider>,
		);
		expect(screen.getByText('Fallback')).toBeInTheDocument();
		rerender(
			<WindowChromeProvider controls={controls}>
				<TitleBarTitle fallback="Fallback" />
				<Setter title="Set" />
			</WindowChromeProvider>,
		);
		expect(screen.getByText('Set')).toBeInTheDocument();
		expect(screen.queryByText('Fallback')).toBeNull();
	});

	it('shows its children as they are, whatever title is set', () => {
		render(
			<WindowChromeProvider controls={controls} formatTitle={(title) => `X ${title}`}>
				<TitleBarTitle fallback="Fallback">Fixed</TitleBarTitle>
				<Setter title="Set" />
			</WindowChromeProvider>,
		);
		expect(screen.getByText('Fixed')).toBeInTheDocument();
	});

	it('shows the fallback without a provider', () => {
		render(<TitleBarTitle fallback="Alone" />);
		expect(screen.getByText('Alone')).toBeInTheDocument();
	});
});
