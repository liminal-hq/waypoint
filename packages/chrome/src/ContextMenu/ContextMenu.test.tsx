// Tests for the context menu keyboard model, submenus, focus and placement
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { WindowControls } from '../TitleBar/windowControls';
import { WindowChromeProvider } from '../WindowChromeProvider/WindowChromeProvider';
import { ContextMenu, SUBMENU_HOVER_DELAY_MS } from './ContextMenu';
import type { MenuItem } from './types';

const items: MenuItem[] = [
	{ type: 'section', label: 'Edit' },
	{ type: 'action', id: 'copy', label: 'Copy', shortcut: 'Ctrl+C' },
	{ type: 'action', id: 'cut', label: 'Cut' },
	{ type: 'separator' },
	{ type: 'checkbox', id: 'wrap', label: 'Word wrap', checked: true },
	{
		type: 'submenu',
		id: 'share',
		label: 'Share',
		items: [
			{ type: 'action', id: 'mail', label: 'Email' },
			{ type: 'action', id: 'link', label: 'Link' },
		],
	},
	{ type: 'action', id: 'off', label: 'Disabled', disabled: true },
	{ type: 'action', id: 'delete', label: 'Delete', danger: true },
];

function setup(overrides: Partial<Parameters<typeof ContextMenu>[0]> = {}) {
	const onSelect = vi.fn();
	const onClose = vi.fn();
	const utils = render(
		<ContextMenu
			items={items}
			position={{ x: 10, y: 10 }}
			onSelect={onSelect}
			onClose={onClose}
			ariaLabel="Test menu"
			{...overrides}
		/>,
	);
	return { onSelect, onClose, ...utils };
}

const item = (name: string) => screen.getByText(name).closest('[role^="menuitem"]') as HTMLElement;

describe('ContextMenu', () => {
	it('exposes menu roles and checkbox state', () => {
		setup();
		expect(screen.getByRole('menu', { name: 'Test menu' })).toBeInTheDocument();
		expect(screen.getAllByRole('separator')).toHaveLength(1);
		expect(screen.getByRole('menuitemcheckbox', { name: 'Word wrap' })).toHaveAttribute(
			'aria-checked',
			'true',
		);
		expect(item('Share')).toHaveAttribute('aria-haspopup', 'menu');
		expect(item('Disabled')).toHaveAttribute('aria-disabled', 'true');
	});

	it('focuses the surface on open and the first item when opened by keyboard', () => {
		const { unmount } = setup();
		expect(screen.getByRole('menu')).toHaveFocus();
		unmount();
		setup({ openedWithKeyboard: true });
		expect(item('Copy')).toHaveFocus();
	});

	it('moves with arrows, wrapping and skipping disabled items', async () => {
		const user = userEvent.setup();
		setup();
		await user.keyboard('{ArrowDown}');
		expect(item('Copy')).toHaveFocus();
		await user.keyboard('{ArrowDown}');
		expect(item('Cut')).toHaveFocus();
		await user.keyboard('{ArrowUp}{ArrowUp}');
		expect(item('Delete')).toHaveFocus();
		await user.keyboard('{ArrowUp}');
		expect(item('Share')).toHaveFocus();
		await user.keyboard('{ArrowDown}{ArrowDown}');
		expect(item('Copy')).toHaveFocus();
	});

	it('jumps with Home and End', async () => {
		const user = userEvent.setup();
		setup({ openedWithKeyboard: true });
		await user.keyboard('{End}');
		expect(item('Delete')).toHaveFocus();
		await user.keyboard('{Home}');
		expect(item('Copy')).toHaveFocus();
	});

	it('activates with Enter and closes', async () => {
		const user = userEvent.setup();
		const { onSelect, onClose } = setup({ openedWithKeyboard: true });
		await user.keyboard('{ArrowDown}{Enter}');
		expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: 'cut' }));
		expect(onClose).toHaveBeenCalled();
	});

	it('activates on click but ignores disabled items', async () => {
		const user = userEvent.setup();
		const { onSelect } = setup();
		await user.click(item('Disabled'));
		expect(onSelect).not.toHaveBeenCalled();
		await user.click(item('Word wrap'));
		expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: 'wrap' }));
	});

	it('closes on Escape', async () => {
		const user = userEvent.setup();
		const { onClose } = setup({ openedWithKeyboard: true });
		await user.keyboard('{Escape}');
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('closes on an outside pointer press', async () => {
		const user = userEvent.setup();
		const { onClose } = setup();
		await user.click(document.body);
		expect(onClose).toHaveBeenCalled();
	});

	it('jumps to items by typing, cycling on repeats', async () => {
		const user = userEvent.setup();
		setup({ openedWithKeyboard: true });
		await user.keyboard('w');
		expect(item('Word wrap')).toHaveFocus();
		await user.keyboard('d');
		// "wd" matches nothing, so the letter restarts and finds Delete.
		expect(item('Delete')).toHaveFocus();
		await user.keyboard('c');
		expect(item('Copy')).toHaveFocus();
		await user.keyboard('c');
		expect(item('Cut')).toHaveFocus();
	});

	describe('submenus', () => {
		it('opens on ArrowRight with focus on its first item, and ArrowLeft returns', async () => {
			const user = userEvent.setup();
			setup({ openedWithKeyboard: true });
			await user.keyboard('s{ArrowRight}');
			expect(item('Email')).toHaveFocus();
			expect(item('Share')).toHaveAttribute('aria-expanded', 'true');
			await user.keyboard('{ArrowLeft}');
			expect(screen.queryByRole('menuitem', { name: 'Email' })).not.toBeInTheDocument();
			expect(item('Share')).toHaveFocus();
		});

		it('closes only the submenu on Escape', async () => {
			const user = userEvent.setup();
			const { onClose } = setup({ openedWithKeyboard: true });
			await user.keyboard('s{ArrowRight}{Escape}');
			expect(onClose).not.toHaveBeenCalled();
			expect(item('Share')).toHaveFocus();
		});

		it('selects from the submenu and closes everything', async () => {
			const user = userEvent.setup();
			const { onSelect, onClose } = setup({ openedWithKeyboard: true });
			await user.keyboard('s{ArrowRight}{ArrowDown}{Enter}');
			expect(onSelect).toHaveBeenCalledWith(expect.objectContaining({ id: 'link' }));
			expect(onClose).toHaveBeenCalled();
		});

		describe('hover', () => {
			beforeEach(() => {
				vi.useFakeTimers();
			});
			afterEach(() => {
				vi.useRealTimers();
			});

			it('opens after 150ms and not before', () => {
				setup();
				const share = item('Share');
				fireEvent.mouseEnter(share);
				act(() => {
					vi.advanceTimersByTime(SUBMENU_HOVER_DELAY_MS - 1);
				});
				expect(screen.queryByRole('menuitem', { name: 'Email' })).not.toBeInTheDocument();
				act(() => {
					vi.advanceTimersByTime(1);
				});
				expect(screen.getByRole('menuitem', { name: 'Email' })).toBeInTheDocument();
			});
		});
	});

	it('restores focus to the trigger on close', () => {
		const trigger = document.createElement('button');
		document.body.append(trigger);
		trigger.focus();
		const { unmount } = setup();
		expect(trigger).not.toHaveFocus();
		unmount();
		expect(trigger).toHaveFocus();
		trigger.remove();
	});

	it('restores focus to an explicit returnFocusTo element', () => {
		const other = document.createElement('button');
		document.body.append(other);
		const { unmount } = setup({ returnFocusTo: other });
		unmount();
		expect(other).toHaveFocus();
		other.remove();
	});

	it('keeps the menu inside the viewport', () => {
		const spy = vi
			.spyOn(HTMLElement.prototype, 'getBoundingClientRect')
			.mockReturnValue({ width: 200, height: 100 } as DOMRect);
		setup({ position: { x: window.innerWidth - 5, y: window.innerHeight - 5 } });
		const menu = screen.getByRole('menu');
		expect(menu.style.getPropertyValue('--wp-menu-x')).toBe(`${window.innerWidth - 204}px`);
		expect(menu.style.getPropertyValue('--wp-menu-y')).toBe(`${window.innerHeight - 104}px`);
		spy.mockRestore();
	});
});

describe('ContextMenu dismissal on focus loss', () => {
	function providerControls(initiallyFocused: boolean) {
		let emit: (focused: boolean) => void = () => {};
		const controls: WindowControls = {
			minimize: vi.fn(),
			toggleMaximize: vi.fn(),
			close: vi.fn(),
			setAlwaysOnTop: vi.fn(),
			isMaximized: () => false,
			onMaximizedChange: () => () => {},
			isFocused: () => initiallyFocused,
			onFocusChange: (listener) => {
				emit = listener;
				return () => {};
			},
		};
		return { controls, emit: (focused: boolean) => act(() => emit(focused)) };
	}

	function withProvider(controls: WindowControls, onClose: () => void) {
		return render(
			<WindowChromeProvider controls={controls}>
				<ContextMenu
					items={items}
					position={{ x: 1, y: 1 }}
					onSelect={() => {}}
					onClose={onClose}
				/>
			</WindowChromeProvider>,
		);
	}

	it('closes on a focused to unfocused transition', async () => {
		const { controls, emit } = providerControls(true);
		const onClose = vi.fn();
		withProvider(controls, onClose);
		await act(async () => {});
		expect(onClose).not.toHaveBeenCalled();
		emit(false);
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('stays open when opened while the window is already unfocused', async () => {
		const { controls, emit } = providerControls(false);
		const onClose = vi.fn();
		withProvider(controls, onClose);
		await act(async () => {});
		expect(onClose).not.toHaveBeenCalled();
		emit(false);
		expect(onClose).not.toHaveBeenCalled();
	});

	it('ignores DOM blur events while a provider is present', async () => {
		const { controls } = providerControls(true);
		const onClose = vi.fn();
		withProvider(controls, onClose);
		await act(async () => {});
		fireEvent.blur(window);
		expect(onClose).not.toHaveBeenCalled();
	});

	it('still dismisses on pointerdown outside and resize with a provider', async () => {
		const { controls } = providerControls(true);
		const onClose = vi.fn();
		withProvider(controls, onClose);
		await act(async () => {});
		fireEvent.pointerDown(document.body);
		fireEvent(window, new Event('resize'));
		expect(onClose).toHaveBeenCalledTimes(2);
	});

	it('works without a provider and falls back to the DOM blur event', () => {
		const { onClose } = setup();
		expect(screen.getByRole('menu')).toBeInTheDocument();
		fireEvent.blur(window);
		expect(onClose).toHaveBeenCalledTimes(1);
	});

	it('places itself through custom properties, not inline layout styles', () => {
		render(
			<ContextMenu
				items={[{ type: 'action', id: 'a', label: 'One' }]}
				position={{ x: 40, y: 30 }}
				onSelect={() => {}}
				onClose={() => {}}
			/>,
		);
		const menu = screen.getByRole('menu');
		expect(menu.style.getPropertyValue('--wp-menu-x')).not.toBe('');
		expect(menu.style.getPropertyValue('--wp-menu-y')).not.toBe('');
		expect(menu.style.left).toBe('');
		expect(menu.style.top).toBe('');
	});
});
