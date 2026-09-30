// Verifies the window frame follows the maximised state and passes content through
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

import { act, render, screen, waitFor } from '@testing-library/react';
import type { WindowControls } from '../TitleBar/windowControls';
import { describe, expect, it, vi } from 'vitest';
import { WindowChromeProvider } from '../WindowChromeProvider/WindowChromeProvider';
import { WindowFrame } from './WindowFrame';

function controls(initiallyMaximised: boolean): WindowControls & { emit: (v: boolean) => void } {
	let listener: (value: boolean) => void = () => {};
	return {
		minimize: vi.fn(),
		toggleMaximize: vi.fn(),
		close: vi.fn(),
		setAlwaysOnTop: vi.fn(),
		isMaximized: vi.fn().mockResolvedValue(initiallyMaximised),
		onMaximizedChange: (l: (value: boolean) => void) => {
			listener = l;
			return () => {};
		},
		emit: (value: boolean) => listener(value),
	};
}

describe('WindowFrame', () => {
	it('renders its children with the caller class and starts rounded', () => {
		render(
			<WindowChromeProvider controls={controls(false)}>
				<WindowFrame className="extra">
					<p>inside</p>
				</WindowFrame>
			</WindowChromeProvider>,
		);
		const surface = screen.getByText('inside').parentElement!;
		const frame = surface.parentElement!;
		expect(surface.classList.contains('extra')).toBe(true);
		expect(frame.getAttribute('data-maximised')).toBe('false');
	});

	it('goes square when the window is already maximised', async () => {
		render(
			<WindowChromeProvider controls={controls(true)}>
				<WindowFrame>
					<p>inside</p>
				</WindowFrame>
			</WindowChromeProvider>,
		);
		const frame = screen.getByText('inside').parentElement!.parentElement!;
		await waitFor(() => expect(frame.getAttribute('data-maximised')).toBe('true'));
	});

	it('follows maximise and restore', async () => {
		const c = controls(false);
		render(
			<WindowChromeProvider controls={c}>
				<WindowFrame>
					<p>inside</p>
				</WindowFrame>
			</WindowChromeProvider>,
		);
		const frame = screen.getByText('inside').parentElement!.parentElement!;
		// Let the initial read settle so it cannot land after the event below.
		await act(async () => {});
		act(() => c.emit(true));
		expect(frame.getAttribute('data-maximised')).toBe('true');
		act(() => c.emit(false));
		expect(frame.getAttribute('data-maximised')).toBe('false');
	});

	it('reports focus and follows focus changes', async () => {
		let listener: (focused: boolean) => void = () => {};
		const c = {
			...controls(false),
			isFocused: vi.fn().mockResolvedValue(false),
			onFocusChange: (l: (focused: boolean) => void) => {
				listener = l;
				return () => {};
			},
		};
		render(
			<WindowChromeProvider controls={c}>
				<WindowFrame>
					<p>inside</p>
				</WindowFrame>
			</WindowChromeProvider>,
		);
		const frame = screen.getByText('inside').parentElement!.parentElement!;
		await waitFor(() => expect(frame.getAttribute('data-focused')).toBe('false'));
		act(() => listener(true));
		expect(frame.getAttribute('data-focused')).toBe('true');
		act(() => listener(false));
		expect(frame.getAttribute('data-focused')).toBe('false');
	});

	it('throws a clear error without a provider', () => {
		const spy = vi.spyOn(console, 'error').mockImplementation(() => {});
		expect(() =>
			render(
				<WindowFrame>
					<p>inside</p>
				</WindowFrame>,
			),
		).toThrow(/WindowChromeProvider/);
		spy.mockRestore();
	});

	it('treats a host that cannot report focus as focused', () => {
		render(
			<WindowChromeProvider controls={controls(false)}>
				<WindowFrame>
					<p>inside</p>
				</WindowFrame>
			</WindowChromeProvider>,
		);
		const frame = screen.getByText('inside').parentElement!.parentElement!;
		expect(frame.getAttribute('data-focused')).toBe('true');
	});
});
