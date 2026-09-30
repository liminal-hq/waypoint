// Tests for the title bar: slots, drag regions, window menu and controls
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { AppMenuButton } from './AppMenuButton';
import { TitleBar, type TitleBarProps } from './TitleBar';
import type { WindowControls } from './windowControls';

function fakeControls(overrides: Partial<WindowControls> = {}) {
	let listener: ((maximised: boolean) => void) | undefined;
	const controls: WindowControls = {
		minimize: vi.fn(),
		toggleMaximize: vi.fn(),
		close: vi.fn(),
		startDragging: vi.fn(),
		setAlwaysOnTop: vi.fn(),
		isMaximized: vi.fn(async () => false),
		onMaximizedChange: vi.fn((l) => {
			listener = l;
			return () => {
				listener = undefined;
			};
		}),
		...overrides,
	};
	return { controls, emitMaximised: (value: boolean) => act(() => listener?.(value)) };
}

function renderBar(controls: WindowControls, props: Partial<TitleBarProps> = {}) {
	return render(
		<TitleBar
			windowControls={controls}
			start={<AppMenuButton label="Demo" items={[]} onSelect={() => {}} />}
			center={<span data-testid="title">Title</span>}
			end={<button type="button">Action</button>}
			showAlwaysOnTop
			{...props}
		/>,
	);
}

const bar = () => document.querySelector('[data-controls-style][data-maximised]') as HTMLElement;

describe('TitleBar', () => {
	it('marks the bar and passive slots as drag regions, but not buttons', () => {
		const { controls } = fakeControls();
		renderBar(controls);
		expect(bar()).toHaveAttribute('data-tauri-drag-region');
		expect(screen.getByTestId('title').parentElement).toHaveAttribute('data-tauri-drag-region');
		for (const button of screen.getAllByRole('button')) {
			expect(button).not.toHaveAttribute('data-tauri-drag-region');
		}
		expect(screen.getByRole('group', { name: 'Window controls' })).not.toHaveAttribute(
			'data-tauri-drag-region',
		);
	});

	it('maximises on double-click of empty space', async () => {
		const user = userEvent.setup();
		const { controls } = fakeControls();
		renderBar(controls);
		await user.dblClick(screen.getByTestId('title'));
		expect(controls.toggleMaximize).toHaveBeenCalledTimes(1);
	});

	it('leaves double-click to the host when it handles it natively', async () => {
		const user = userEvent.setup();
		const { controls } = fakeControls({ handlesDoubleClickNatively: true });
		renderBar(controls);
		await user.dblClick(screen.getByTestId('title'));
		expect(controls.toggleMaximize).not.toHaveBeenCalled();
	});

	it('does not maximise on double-click of a button', async () => {
		const user = userEvent.setup();
		const { controls } = fakeControls();
		renderBar(controls);
		await user.dblClick(screen.getByRole('button', { name: 'Action' }));
		expect(controls.toggleMaximize).not.toHaveBeenCalled();
	});

	it('toggles the maximised class and restore label with the adapter state', async () => {
		const { controls, emitMaximised } = fakeControls({ isMaximized: vi.fn(async () => true) });
		renderBar(controls);
		await waitFor(() => expect(bar()).toHaveClass('maximised'));
		expect(screen.getByRole('button', { name: 'Restore' })).toBeInTheDocument();
		emitMaximised(false);
		expect(bar()).not.toHaveClass('maximised');
		expect(screen.getByRole('button', { name: 'Maximise' })).toBeInTheDocument();
	});

	it('invokes the adapter from the window buttons', async () => {
		const user = userEvent.setup();
		const { controls } = fakeControls();
		renderBar(controls);
		await user.click(screen.getByRole('button', { name: 'Minimise' }));
		await user.click(screen.getByRole('button', { name: 'Maximise' }));
		await user.click(screen.getByRole('button', { name: 'Always on Top' }));
		await user.click(screen.getByRole('button', { name: 'Close' }));
		expect(controls.minimize).toHaveBeenCalled();
		expect(controls.toggleMaximize).toHaveBeenCalled();
		expect(controls.setAlwaysOnTop).toHaveBeenCalledWith(true);
		expect(controls.close).toHaveBeenCalled();
		expect(screen.getByRole('button', { name: 'Always on Top' })).toHaveAttribute(
			'aria-pressed',
			'true',
		);
	});

	it('hides the Always on Top button unless enabled', () => {
		const { controls } = fakeControls();
		renderBar(controls, { showAlwaysOnTop: false });
		expect(screen.queryByRole('button', { name: 'Always on Top' })).not.toBeInTheDocument();
	});

	it('orders buttons by side and exposes the style', () => {
		const { controls } = fakeControls();
		renderBar(controls, { controlsSide: 'start', controlsStyle: 'win11', showAlwaysOnTop: false });
		const group = screen.getByRole('group', { name: 'Window controls' });
		expect(group).toHaveAttribute('data-controls-style', 'win11');
		expect(
			Array.from(group.querySelectorAll('button')).map((b) => b.getAttribute('aria-label')),
		).toEqual(['Close', 'Minimise', 'Maximise']);
		expect(bar().firstElementChild).toBe(group);
	});

	describe('window menu', () => {
		it('opens on right-click of empty space and runs actions', async () => {
			const user = userEvent.setup();
			const { controls } = fakeControls();
			renderBar(controls);
			await user.pointer({ keys: '[MouseRight]', target: screen.getByTestId('title') });
			const menu = screen.getByRole('menu', { name: 'Window menu' });
			expect(menu).toBeInTheDocument();
			await user.click(screen.getByRole('menuitemcheckbox', { name: 'Always on Top' }));
			expect(controls.setAlwaysOnTop).toHaveBeenCalledWith(true);
			expect(screen.queryByRole('menu')).not.toBeInTheDocument();
		});

		it('shows Restore when maximised and checks Always on Top from the adapter', async () => {
			const user = userEvent.setup();
			const { controls } = fakeControls({
				isMaximized: vi.fn(async () => true),
				isAlwaysOnTop: vi.fn(async () => true),
			});
			renderBar(controls);
			await waitFor(() => expect(bar()).toHaveClass('maximised'));
			await user.pointer({ keys: '[MouseRight]', target: bar() });
			expect(screen.getByRole('menuitem', { name: 'Restore' })).toBeInTheDocument();
			expect(screen.getByRole('menuitemcheckbox', { name: 'Always on Top' })).toHaveAttribute(
				'aria-checked',
				'true',
			);
		});

		it('does not open on the app button or the window buttons', async () => {
			const user = userEvent.setup();
			const { controls } = fakeControls();
			renderBar(controls);
			await user.pointer({
				keys: '[MouseRight]',
				target: screen.getByRole('button', { name: 'Demo' }),
			});
			expect(screen.queryByRole('menu')).not.toBeInTheDocument();
			await user.pointer({
				keys: '[MouseRight]',
				target: screen.getByRole('button', { name: 'Close' }),
			});
			expect(screen.queryByRole('menu')).not.toBeInTheDocument();
		});

		it('routes Close and Move through the adapter', async () => {
			const user = userEvent.setup();
			const { controls } = fakeControls();
			renderBar(controls);
			await user.pointer({ keys: '[MouseRight]', target: bar() });
			await user.click(screen.getByRole('menuitem', { name: 'Move' }));
			expect(controls.startDragging).toHaveBeenCalled();
			await user.pointer({ keys: '[MouseRight]', target: bar() });
			await user.click(screen.getByRole('menuitem', { name: 'Close' }));
			expect(controls.close).toHaveBeenCalled();
		});
	});
});

describe('AppMenuButton', () => {
	const items = [{ type: 'action' as const, id: 'about', label: 'About' }];

	it('opens on F10 with focus on the first item, then returns focus to the button', async () => {
		const user = userEvent.setup();
		render(<AppMenuButton label="Demo" items={items} onSelect={() => {}} />);
		await user.keyboard('{F10}');
		expect(screen.getByRole('menuitem', { name: 'About' })).toHaveFocus();
		await user.keyboard('{Escape}');
		expect(screen.getByRole('button', { name: 'Demo' })).toHaveFocus();
	});

	it('opens on a lone Alt press but not on Alt combined with another key', async () => {
		const user = userEvent.setup();
		render(<AppMenuButton label="Demo" items={items} onSelect={() => {}} />);
		await user.keyboard('{Alt>}x{/Alt}');
		expect(screen.queryByRole('menu')).not.toBeInTheDocument();
		await user.keyboard('{Alt}');
		expect(screen.getByRole('menu', { name: 'Demo' })).toBeInTheDocument();
	});
});
