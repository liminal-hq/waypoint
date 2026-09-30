// Tests for the title bar: slots, drag regions, window menu and controls
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { AppMenuButton } from './AppMenuButton';
import { WindowChromeProvider } from '../WindowChromeProvider/WindowChromeProvider';
import { DEFAULT_TITLEBAR_ACTIONS, type ButtonLayout, type TitlebarActions } from './buttonLayout';
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
		<WindowChromeProvider controls={controls}>
			<TitleBar
				start={<AppMenuButton label="Demo" items={[]} onSelect={() => {}} />}
				center={<span data-testid="title">Title</span>}
				end={<button type="button">Action</button>}
				showAlwaysOnTop
				{...props}
			/>
		</WindowChromeProvider>,
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

	describe('Always on Top follows the window manager', () => {
		const pin = () => screen.getByRole('button', { name: 'Always on Top' });

		function watchedControls(initial: boolean) {
			let listener: ((value: boolean) => void) | undefined;
			const unsubscribe = vi.fn();
			const { controls } = fakeControls({
				isAlwaysOnTop: vi.fn(async () => initial),
				onAlwaysOnTopChange: vi.fn((l) => {
					listener = l;
					return unsubscribe;
				}),
			});
			return { controls, unsubscribe, emit: (value: boolean) => act(() => listener?.(value)) };
		}

		it('updates the pin when the window manager changes the state', async () => {
			const { controls, emit } = watchedControls(false);
			renderBar(controls);
			await waitFor(() => expect(pin()).toHaveAttribute('aria-pressed', 'false'));
			emit(true);
			expect(pin()).toHaveAttribute('aria-pressed', 'true');
			emit(false);
			expect(pin()).toHaveAttribute('aria-pressed', 'false');
		});

		it('reads the current state only after subscribing', async () => {
			const order: string[] = [];
			const { controls } = fakeControls({
				isAlwaysOnTop: vi.fn(async () => {
					order.push('read');
					return true;
				}),
				onAlwaysOnTopChange: vi.fn(() => {
					order.push('subscribe');
					return () => {};
				}),
			});
			renderBar(controls);
			await waitFor(() => expect(pin()).toHaveAttribute('aria-pressed', 'true'));
			expect(order).toEqual(['subscribe', 'read']);
		});

		it('keeps a change that arrives while the initial read is in flight', async () => {
			let resolveRead: (value: boolean) => void = () => {};
			let listener: ((value: boolean) => void) | undefined;
			const { controls } = fakeControls({
				isAlwaysOnTop: vi.fn(() => new Promise<boolean>((resolve) => (resolveRead = resolve))),
				onAlwaysOnTopChange: vi.fn((l) => {
					listener = l;
					return () => {};
				}),
			});
			renderBar(controls);
			await waitFor(() => expect(controls.isAlwaysOnTop).toHaveBeenCalled());
			act(() => listener?.(true));
			await act(async () => resolveRead(false));
			expect(pin()).toHaveAttribute('aria-pressed', 'true');
		});

		it('stops listening when unmounted', () => {
			const { controls, unsubscribe } = watchedControls(false);
			renderBar(controls).unmount();
			expect(unsubscribe).toHaveBeenCalledTimes(1);
		});
	});

	it('hides the Always on Top button unless enabled', () => {
		const { controls } = fakeControls();
		renderBar(controls, { showAlwaysOnTop: false });
		expect(screen.queryByRole('button', { name: 'Always on Top' })).not.toBeInTheDocument();
	});

	it('exposes the controls style on each group', () => {
		const { controls } = fakeControls();
		renderBar(controls, { controlsStyle: 'win11', showAlwaysOnTop: false });
		const group = screen.getByRole('group', { name: 'Window controls' });
		expect(group).toHaveAttribute('data-controls-style', 'win11');
		expect(group).toHaveAttribute('data-controls-side', 'end');
	});

	it('throws a clear error without a provider', () => {
		const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
		expect(() => render(<TitleBar />)).toThrow(/WindowChromeProvider/);
		spy.mockRestore();
	});

	describe('structure', () => {
		const groups = () => {
			const el = bar();
			return {
				all: Array.from(el.children) as HTMLElement[],
				start: el.querySelector('[data-group="start"]') as HTMLElement,
				centre: el.querySelector('[data-group="centre"]') as HTMLElement,
				end: el.querySelector('[data-group="end"]') as HTMLElement,
			};
		};

		it('lays out start group, centre and end group in order', () => {
			const { controls } = fakeControls();
			renderBar(controls, { buttonLayout: { start: ['close'], end: ['minimise', 'maximise'] } });
			const g = groups();
			expect(g.all).toEqual([g.start, g.centre, g.end]);
			expect(g.centre).toContainElement(screen.getByTestId('title'));
		});

		it('puts start-side buttons before the start slot and end-side buttons after the end slot', () => {
			const { controls } = fakeControls();
			renderBar(controls, {
				buttonLayout: { start: ['close'], end: ['minimise'] },
				showAlwaysOnTop: false,
			});
			const g = groups();
			const startLabels = Array.from(g.start.querySelectorAll('button')).map(
				(b) => b.getAttribute('aria-label') ?? b.textContent,
			);
			expect(startLabels).toEqual(['Close', 'Demo']);
			const endLabels = Array.from(g.end.querySelectorAll('button')).map(
				(b) => b.getAttribute('aria-label') ?? b.textContent,
			);
			expect(endLabels).toEqual(['Action', 'Minimise']);
			expect(g.start.firstElementChild).toHaveAttribute('data-wp-controls');
			expect(g.end.lastElementChild).toHaveAttribute('data-wp-controls');
		});

		it('keeps the three groups when everything is empty', () => {
			const { controls } = fakeControls();
			render(
				<WindowChromeProvider controls={controls}>
					<TitleBar buttonLayout={{ start: [], end: [] }} />
				</WindowChromeProvider>,
			);
			expect(groups().all).toHaveLength(3);
		});

		it('defaults the title alignment to centre and accepts start', () => {
			const { controls } = fakeControls();
			const first = renderBar(controls);
			expect(bar()).toHaveAttribute('data-title-align', 'center');
			first.unmount();
			renderBar(controls, { titleAlign: 'start' });
			expect(bar()).toHaveAttribute('data-title-align', 'start');
		});
	});

	describe('button layout', () => {
		const labelsOf = (group: HTMLElement) =>
			Array.from(group.querySelectorAll('button')).map((b) => b.getAttribute('aria-label'));
		const setup = (layout: ButtonLayout, props: Partial<TitleBarProps> = {}) => {
			const { controls } = fakeControls();
			renderBar(controls, { buttonLayout: layout, showAlwaysOnTop: false, ...props });
			const sideGroups = Array.from(document.querySelectorAll<HTMLElement>('[data-wp-controls]'));
			return {
				start: sideGroups.find((g) => g.dataset.controlsSide === 'start'),
				end: sideGroups.find((g) => g.dataset.controlsSide === 'end'),
				count: sideGroups.length,
			};
		};

		it('renders the default layout on the end side only', () => {
			const { controls } = fakeControls();
			renderBar(controls, { showAlwaysOnTop: false });
			const { start, end } = {
				start: document.querySelector('[data-controls-side="start"]'),
				end: document.querySelector<HTMLElement>('[data-controls-side="end"]')!,
			};
			expect(start).toBeNull();
			expect(labelsOf(end)).toEqual(['Minimise', 'Maximise', 'Close']);
		});

		it('renders a GNOME appmenu:minimize,maximize,close layout on the end side', () => {
			const { start, end, count } = setup({
				start: ['appMenu'],
				end: ['minimise', 'maximise', 'close'],
			});
			expect(count).toBe(1);
			expect(start).toBeUndefined();
			expect(labelsOf(end!)).toEqual(['Minimise', 'Maximise', 'Close']);
		});

		it('renders a close-only layout', () => {
			const { end } = setup({ start: [], end: ['close'] });
			expect(labelsOf(end!)).toEqual(['Close']);
		});

		it('renders buttons on both sides', () => {
			const { start, end } = setup({ start: ['close'], end: ['minimise', 'maximise'] });
			expect(labelsOf(start!)).toEqual(['Close']);
			expect(labelsOf(end!)).toEqual(['Minimise', 'Maximise']);
		});

		it('keeps the order it is given', () => {
			const { end } = setup({ start: [], end: ['close', 'minimise'] });
			expect(labelsOf(end!)).toEqual(['Close', 'Minimise']);
		});

		it('skips tokens the chrome does not support', () => {
			const { start, end } = setup({
				start: ['appMenu', 'windowMenu', 'shade'],
				end: ['help', 'stick', 'keepBelow', 'minimise', 'close'],
			});
			expect(start).toBeUndefined();
			expect(labelsOf(end!)).toEqual(['Minimise', 'Close']);
		});

		it('renders no controls for an empty layout', () => {
			const { count } = setup({ start: [], end: [] });
			expect(count).toBe(0);
			expect(screen.queryByRole('group', { name: 'Window controls' })).not.toBeInTheDocument();
		});

		it('places the pin before the first window button on the end side', () => {
			const { start, end } = setup(
				{ start: ['close'], end: ['appMenu', 'minimise', 'maximise'] },
				{ showAlwaysOnTop: true },
			);
			expect(labelsOf(start!)).toEqual(['Close']);
			expect(labelsOf(end!)).toEqual(['Always on Top', 'Minimise', 'Maximise']);
		});

		it('places the pin where the layout puts keepAbove', () => {
			const { start, end } = setup(
				{ start: ['keepAbove', 'close'], end: ['minimise', 'maximise'] },
				{ showAlwaysOnTop: true },
			);
			expect(labelsOf(start!)).toEqual(['Always on Top', 'Close']);
			expect(labelsOf(end!)).toEqual(['Minimise', 'Maximise']);
		});

		it('flips maximise to restore on both sides of state', async () => {
			const { controls } = fakeControls({ isMaximized: vi.fn(async () => true) });
			renderBar(controls, { buttonLayout: { start: ['maximise'], end: [] } });
			expect(await screen.findByRole('button', { name: 'Restore' })).toBeInTheDocument();
		});
	});

	describe('titlebar actions', () => {
		const actions = (over: Partial<TitlebarActions>): TitlebarActions => ({
			...DEFAULT_TITLEBAR_ACTIONS,
			...over,
		});

		it('runs minimise on double-click', () => {
			const { controls } = fakeControls();
			renderBar(controls, { titlebarActions: actions({ doubleClick: 'minimise' }) });
			fireEvent.doubleClick(screen.getByTestId('title'));
			expect(controls.minimize).toHaveBeenCalledTimes(1);
			expect(controls.toggleMaximize).not.toHaveBeenCalled();
		});

		it('opens the menu on double-click when asked', () => {
			const { controls } = fakeControls();
			renderBar(controls, { titlebarActions: actions({ doubleClick: 'menu' }) });
			fireEvent.doubleClick(screen.getByTestId('title'));
			expect(screen.getByRole('menu', { name: 'Window menu' })).toBeInTheDocument();
		});

		it.each(['toggleShade', 'lower', 'none'] as const)('ignores double-click %s', (action) => {
			const { controls } = fakeControls();
			renderBar(controls, { titlebarActions: actions({ doubleClick: action }) });
			fireEvent.doubleClick(screen.getByTestId('title'));
			expect(controls.minimize).not.toHaveBeenCalled();
			expect(controls.toggleMaximize).not.toHaveBeenCalled();
			expect(screen.queryByRole('menu')).not.toBeInTheDocument();
		});

		it('still runs a non-maximise double-click action when the host handles maximise natively', () => {
			const { controls } = fakeControls({ handlesDoubleClickNatively: true });
			renderBar(controls, { titlebarActions: actions({ doubleClick: 'minimise' }) });
			fireEvent.doubleClick(screen.getByTestId('title'));
			expect(controls.minimize).toHaveBeenCalledTimes(1);
		});

		describe('native double-click maximise', () => {
			// Stands in for Tauri's drag script, which listens on `document` and maximises on the
			// second press of a double click.
			function watchDocument() {
				const seen: string[] = [];
				const listener = (event: Event) =>
					seen.push(`${event.type}:${(event as MouseEvent).detail}`);
				document.addEventListener('mousedown', listener);
				document.addEventListener('mouseup', listener);
				return {
					seen,
					stop: () => {
						document.removeEventListener('mousedown', listener);
						document.removeEventListener('mouseup', listener);
					},
				};
			}

			function doubleClickPresses(target: Element) {
				for (const detail of [1, 2]) {
					fireEvent.mouseDown(target, { button: 0, detail });
					fireEvent.mouseUp(target, { button: 0, detail });
				}
			}

			it.each(['minimise', 'menu', 'none', 'toggleShade', 'lower'] as const)(
				'keeps the second press from reaching the host when the action is %s',
				(action) => {
					const { controls } = fakeControls({ handlesDoubleClickNatively: true });
					renderBar(controls, { titlebarActions: actions({ doubleClick: action }) });
					const document_ = watchDocument();
					doubleClickPresses(screen.getByTestId('title'));
					document_.stop();
					expect(document_.seen).toEqual(['mousedown:1', 'mouseup:1']);
				},
			);

			it('lets the host see the second press when the action is toggleMaximise', () => {
				const { controls } = fakeControls({ handlesDoubleClickNatively: true });
				renderBar(controls, { titlebarActions: actions({ doubleClick: 'toggleMaximise' }) });
				const document_ = watchDocument();
				doubleClickPresses(screen.getByTestId('title'));
				document_.stop();
				expect(document_.seen).toEqual(['mousedown:1', 'mouseup:1', 'mousedown:2', 'mouseup:2']);
			});

			it('leaves presses alone when the host does not handle maximise natively', () => {
				const { controls } = fakeControls({ handlesDoubleClickNatively: false });
				renderBar(controls, { titlebarActions: actions({ doubleClick: 'minimise' }) });
				const document_ = watchDocument();
				doubleClickPresses(screen.getByTestId('title'));
				document_.stop();
				expect(document_.seen).toHaveLength(4);
			});

			it('does not suppress presses on window buttons', () => {
				const { controls } = fakeControls({ handlesDoubleClickNatively: true });
				renderBar(controls, { titlebarActions: actions({ doubleClick: 'none' }) });
				const document_ = watchDocument();
				doubleClickPresses(screen.getByRole('button', { name: 'Minimise' }));
				document_.stop();
				expect(document_.seen).toHaveLength(4);
			});
		});

		it('ignores middle-click by default', () => {
			const { controls } = fakeControls();
			renderBar(controls);
			fireEvent(
				screen.getByTestId('title'),
				new MouseEvent('auxclick', { bubbles: true, button: 1 }),
			);
			expect(controls.minimize).not.toHaveBeenCalled();
			expect(controls.toggleMaximize).not.toHaveBeenCalled();
			expect(screen.queryByRole('menu')).not.toBeInTheDocument();
		});

		it('maps middle-click to minimise, maximise and menu', () => {
			const middle = (target: Element) =>
				fireEvent(target, new MouseEvent('auxclick', { bubbles: true, button: 1 }));
			const { controls } = fakeControls();
			const first = renderBar(controls, { titlebarActions: actions({ middleClick: 'minimise' }) });
			middle(screen.getByTestId('title'));
			expect(controls.minimize).toHaveBeenCalledTimes(1);
			first.unmount();

			const second = renderBar(controls, {
				titlebarActions: actions({ middleClick: 'toggleMaximise' }),
			});
			middle(screen.getByTestId('title'));
			expect(controls.toggleMaximize).toHaveBeenCalledTimes(1);
			second.unmount();

			renderBar(controls, { titlebarActions: actions({ middleClick: 'menu' }) });
			middle(screen.getByTestId('title'));
			expect(screen.getByRole('menu', { name: 'Window menu' })).toBeInTheDocument();
		});

		it('ignores other mouse buttons on auxclick and interactive targets', () => {
			const { controls } = fakeControls();
			renderBar(controls, { titlebarActions: actions({ middleClick: 'minimise' }) });
			fireEvent(
				screen.getByTestId('title'),
				new MouseEvent('auxclick', { bubbles: true, button: 2 }),
			);
			fireEvent(
				screen.getByRole('button', { name: 'Action' }),
				new MouseEvent('auxclick', { bubbles: true, button: 1 }),
			);
			expect(controls.minimize).not.toHaveBeenCalled();
		});

		it('opens the menu on right-click and prevents the native one', () => {
			const { controls } = fakeControls();
			renderBar(controls);
			const notPrevented = fireEvent.contextMenu(screen.getByTestId('title'));
			expect(notPrevented).toBe(false);
			expect(screen.getByRole('menu', { name: 'Window menu' })).toBeInTheDocument();
		});

		it.each(['none', 'minimise', 'toggleMaximise', 'lower'] as const)(
			'does nothing and keeps the native menu for right-click %s',
			(action) => {
				const { controls } = fakeControls();
				renderBar(controls, { titlebarActions: actions({ rightClick: action }) });
				const notPrevented = fireEvent.contextMenu(screen.getByTestId('title'));
				expect(notPrevented).toBe(true);
				expect(screen.queryByRole('menu')).not.toBeInTheDocument();
				expect(controls.minimize).not.toHaveBeenCalled();
				expect(controls.toggleMaximize).not.toHaveBeenCalled();
			},
		);
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

	it('is focused by default and quiets itself while the window is unfocused', async () => {
		const { controls: plain } = fakeControls();
		const first = renderBar(plain);
		expect(bar().getAttribute('data-focused')).toBe('true');
		first.unmount();

		let emit: (focused: boolean) => void = () => {};
		const { controls } = fakeControls({
			isFocused: vi.fn(async () => false),
			onFocusChange: vi.fn((l) => {
				emit = l;
				return () => {};
			}),
		});
		renderBar(controls);
		await waitFor(() => expect(bar().getAttribute('data-focused')).toBe('false'));
		act(() => emit(true));
		expect(bar().getAttribute('data-focused')).toBe('true');
	});

	describe('Always on Top pin placement', () => {
		const labelsIn = (side: 'start' | 'end') =>
			[...document.querySelectorAll(`[data-wp-controls][data-controls-side="${side}"] button`)].map(
				(button) => button.getAttribute('aria-label'),
			);

		it('sits before the ordered controls on the end side, keeping Close outermost', () => {
			const { controls } = fakeControls();
			renderBar(controls, {
				buttonLayout: { start: [], end: ['minimise', 'maximise', 'close'] },
			});
			expect(labelsIn('end')).toEqual(['Always on Top', 'Minimise', 'Maximise', 'Close']);
		});

		it('stays off the start side, so Close stays outermost on a start-side layout', () => {
			const { controls } = fakeControls();
			renderBar(controls, {
				buttonLayout: { start: ['close', 'minimise', 'maximise'], end: [] },
			});
			expect(labelsIn('start')).toEqual(['Close', 'Minimise', 'Maximise']);
			expect(labelsIn('end')).toEqual(['Always on Top']);
		});

		it('never reorders the layout the user configured', () => {
			const { controls } = fakeControls();
			renderBar(controls, { buttonLayout: { start: ['close'], end: ['minimise', 'maximise'] } });
			expect(labelsIn('start')).toEqual(['Close']);
			expect(labelsIn('end')).toEqual(['Always on Top', 'Minimise', 'Maximise']);
		});
	});

	describe('titlebar actions the chrome cannot perform', () => {
		const unperformed = [
			'toggleShade',
			'lower',
			'toggleMaximiseHorizontally',
			'toggleMaximiseVertically',
		] as const;

		it.each(unperformed)('treats %s as no action for every gesture', (action) => {
			const { controls } = fakeControls();
			renderBar(controls, {
				titlebarActions: { doubleClick: action, middleClick: action, rightClick: action },
			});
			const target = document.querySelector('[data-group="centre"]') as HTMLElement;

			fireEvent.doubleClick(target);
			fireEvent(target, new MouseEvent('auxclick', { bubbles: true, button: 1 }));
			const notPrevented = fireEvent.contextMenu(target);

			expect(controls.toggleMaximize).not.toHaveBeenCalled();
			expect(controls.minimize).not.toHaveBeenCalled();
			expect(screen.queryByRole('menu')).toBeNull();
			// A gesture that does nothing must not swallow the platform's own behaviour.
			expect(notPrevented).toBe(true);
		});

		it('still performs the actions it supports', () => {
			const { controls } = fakeControls({ handlesDoubleClickNatively: false });
			renderBar(controls, {
				titlebarActions: {
					doubleClick: 'toggleMaximise',
					middleClick: 'minimise',
					rightClick: 'menu',
				},
			});
			const target = document.querySelector('[data-group="centre"]') as HTMLElement;
			fireEvent.doubleClick(target);
			fireEvent(target, new MouseEvent('auxclick', { bubbles: true, button: 1 }));
			expect(controls.toggleMaximize).toHaveBeenCalledTimes(1);
			expect(controls.minimize).toHaveBeenCalledTimes(1);
		});
	});
});
